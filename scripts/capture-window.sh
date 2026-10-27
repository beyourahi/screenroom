#!/usr/bin/env bash
# Captures the Screenroom desktop window to a PNG on macOS.
#
# The capture is addressed by window number, never by a screen rectangle: a
# rectangle records whatever is on top of it, which on a multi-display desktop can
# be a different application entirely. Raising the window first is best-effort, so
# an occluded window still resolves; if it stays occluded the capture may show the
# occluding window, which is why callers look at what they captured.
#
#   scripts/capture-window.sh .artifacts/evidence/player.png
#   SCREENROOM_PROCESS=other scripts/capture-window.sh out.png
#
# The helper is compiled once into target/devtools (Git-ignored) and reused.
# Requires Screen Recording permission for the invoking terminal.
set -euo pipefail

out="${1:?usage: capture-window.sh <output.png>}"
process="${SCREENROOM_PROCESS:-screenroom-desktop}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_file="$root/scripts/capture-window.swift"
binary="$root/target/devtools/capture-window"

if [ ! -x "$binary" ] || [ "$source_file" -nt "$binary" ]; then
  mkdir -p "$(dirname "$binary")"
  xcrun swiftc -O -swift-version 5 -o "$binary" "$source_file" >/dev/null
fi

# Best-effort raise: a failure here is not fatal, because the capture is by window
# number. It only makes the captured content current and un-occluded.
osascript >/dev/null 2>&1 <<APPLESCRIPT || true
tell application "System Events"
  tell process "$process"
    set frontmost to true
    try
      perform action "AXRaise" of window 1
    end try
  end tell
end tell
APPLESCRIPT

mkdir -p "$(dirname "$out")"
listing="$("$binary" "$process" --list 2>/dev/null)" || {
  echo "no window for \"$process\" — is Screenroom running?" >&2
  exit 1
}
window_number="$(printf '%s\n' "$listing" | head -1 | cut -f1)"
if [ -z "$window_number" ]; then
  echo "could not resolve a window number for \"$process\"" >&2
  exit 1
fi

# `-l <number>` captures that window's own pixels, so no other application can
# appear in an evidence file even if the window moves or the region overlaps one.
screencapture -x -o -l"$window_number" "$out"
printf 'captured window %s of %s -> %s\n' "$window_number" "$process" "$out"
