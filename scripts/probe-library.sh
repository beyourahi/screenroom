#!/usr/bin/env bash
# Probes every media file under a root with ffprobe, read-only, into JSON Lines.
#
# **Reads the media root and writes nothing there.** Output goes to the path you
# name, which should be outside the root (`.artifacts/...` in this repo). This is a
# measurement tool for real-library work, not part of the app: the app decides
# playback compatibility from the platform player, never from a container list.
#
#   scripts/probe-library.sh "/Volumes/SomeDrive/movies & shows" .artifacts/probe.jsonl
#
# Every line is one file:
#   {"path": "<relative path>", "ok": true, "format": {...}, "streams": [...]}
#   {"path": "<relative path>", "ok": false, "error": "<ffprobe stderr, first line>"}
#
# Excluded, the same way a scan is: AppleDouble `._*` companions, dot-directories,
# backup folders and anything outside a media-container extension. Concurrency is
# bounded (`PROBE_JOBS`, default 4) so a USB drive is not saturated and the machine
# stays usable; no file is ever opened for writing.
set -uo pipefail

root="${1:?usage: probe-library.sh <media-root> <output.jsonl>}"
out="${2:?usage: probe-library.sh <media-root> <output.jsonl>}"
jobs="${PROBE_JOBS:-4}"

if [ ! -d "$root" ]; then
  echo "not a directory: $root" >&2
  exit 2
fi
command -v ffprobe >/dev/null || { echo "ffprobe is not installed" >&2; exit 2; }
command -v jq >/dev/null || { echo "jq is not installed" >&2; exit 2; }

mkdir -p "$(dirname "$out")"
# Each worker writes its own file and the results are concatenated at the end:
# several processes appending to one descriptor interleaves their JSON and
# corrupts the line that happens to land in the middle.
work=$(mktemp -d -t screenroom-probe)
trap 'rm -rf "$work"' EXIT

# The extensions the library imports; probing one it would not import would make
# the probe matrix disagree with the catalogue.
find "$root" -type f \
  \( -iname '*.mkv' -o -iname '*.mp4' -o -iname '*.m4v' -o -iname '*.avi' \
     -o -iname '*.mov' -o -iname '*.webm' -o -iname '*.mpeg' -o -iname '*.mpg' \
     -o -iname '*.ts' -o -iname '*.m2ts' -o -iname '*.wmv' -o -iname '*.flv' \
     -o -iname '*.ogv' -o -iname '*.3gp' \) \
  -not -name '._*' -not -path '*/.*' \
  -not -path '*/Auto Data Backup/*' -not -path '*arkham project media*' \
  -print0 2>/dev/null |
  PROBE_WORK="$work" xargs -0 -n 1 -P "$jobs" bash -c '
    # $0 is the root, so the file xargs appended is $1; the worker directory
    # travels in the environment rather than as another positional, where it
    # would land after the filename.
    file="$1"
    relative="${file#"$0"/}"
    # `-v error` keeps warnings out of the JSON and a non-zero exit is reported
    # rather than swallowed. ffprobe has no `-nostdin` (that is an ffmpeg option);
    # passing it fails every file with "Option not found", which is exactly how
    # this harness first reported the whole library as unreadable.
    stderr=$(mktemp -t screenroom-probe)
    if json=$(ffprobe -v error -show_entries \
        "format=format_name,duration,size,bit_rate:format_tags=title:stream=index,codec_type,codec_name,profile,width,height,pix_fmt,r_frame_rate,sample_aspect_ratio,display_aspect_ratio,channels,channel_layout,sample_rate,bit_rate:stream_tags=language,title:stream_disposition=attached_pic" \
        -of json "$file" 2>"$stderr"); then
      # The document goes through stdin, never into the jq program: interpolating
      # it would paste its quotes straight into the filter and silently print
      # nothing.
      printf "%s" "$json" | jq -c --arg path "$relative" ". + {path: \$path, ok: true}"
    else
      error=$(head -1 "$stderr" 2>/dev/null | tr -d "\r")
      jq -cn --arg path "$relative" --arg error "$error" \
        "{path: \$path, ok: false, error: \$error}"
    fi >>"$PROBE_WORK/$$.jsonl"
    rm -f "$stderr"
  ' "$root"

# One line per file, sorted so two runs over the same drive diff cleanly and no
# line is a splice of two workers.
cat "$work"/*.jsonl 2>/dev/null | jq -c . | sort >"$out"
echo "probed $(wc -l <"$out" | tr -d ' ') file(s) into $out" >&2
