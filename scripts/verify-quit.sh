#!/usr/bin/env bash
# Checks that dismissing the window stores the viewing and closes its session —
# for both the close box and ⌘Q.
#
#   scripts/verify-quit.sh                  # both dismissals
#   scripts/verify-quit.sh cmd-q            # one of them
#   SCREENROOM_MEDIA=/path/to/media scripts/verify-quit.sh
#
# Why this is a script and not a sentence in a document: ⌘Q on macOS is a menu bar
# command, not an event the application receives, so "it saves the position" is a
# claim about how the platform's menu was wired, and the only way to keep believing
# it is to dismiss a real window with a real key press and read what the library
# holds afterwards.
#
# A run is only meaningful if playback is actually happening. That is confirmed
# through the database rather than assumed from the keystrokes, and a keystroke
# that failed to land is retried — driving a window that sits on another Space is
# unreliable, and a green result produced by nothing playing would be a lie.
#
# Requires Accessibility and Screen Recording permission for the invoking terminal,
# the same as the two helpers this uses.
#
# This is an application test on this Mac. It is not a device test on any other
# platform, and it says nothing about Windows or a TV.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

process="screenroom-desktop"
binary="target/debug/screenroom-desktop"
media="${SCREENROOM_MEDIA:-$root/.artifacts/media}"
work="${SCREENROOM_VERIFY_DIR:-${TMPDIR:-/tmp}/screenroom-verify}"
database="$work/library.db"
mode="${1:-both}"

case "$mode" in
  both | cmd-q | close-box) ;;
  *)
    echo "usage: verify-quit.sh [both|cmd-q|close-box]" >&2
    exit 2
    ;;
esac

for tool in sqlite3 osascript; do
  command -v "$tool" >/dev/null || {
    echo "$tool is required" >&2
    exit 2
  }
done
[ -d "$media" ] || {
  echo "no media folder at $media" >&2
  exit 2
}

query() { sqlite3 "$database" "$1" 2>/dev/null; }
active_sessions() { query "SELECT COUNT(*) FROM sessions WHERE status='active';"; }
stored_position() { query "SELECT COALESCE(position_ms, -1) FROM progress;"; }

echo "==> building"
cargo build -q -p screenroom-desktop || exit 1

echo "==> preparing a scratch library from $media"
rm -rf "$work"
mkdir -p "$work"
cargo run -q -p screenroom-core --example prepare_library -- "$database" "$media" >/dev/null || exit 1

# The window server, and the accessibility API with it, hides a window that lives
# on another Space. Raising it through the capture helper is what brings it back,
# so the helper (which raises and then captures) is retried until it finds a window.
await_window() {
  local attempt
  for attempt in $(seq 1 10); do
    if ./scripts/capture-window.sh "$work/window.png" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

# Drives the library to a playing file, and only returns once the database agrees
# that a session is open.
start_playing() {
  local attempt
  for attempt in $(seq 1 8); do
    if [ "$(active_sessions)" -gt 0 ]; then
      return 0
    fi
    ./scripts/drive-app.sh escape >/dev/null 2>&1
    sleep 0.6
    ./scripts/drive-app.sh library >/dev/null 2>&1
    sleep 1.0
    ./scripts/drive-app.sh right >/dev/null 2>&1
    sleep 1.0
    ./scripts/drive-app.sh enter >/dev/null 2>&1
    sleep 1.8
    ./scripts/drive-app.sh enter >/dev/null 2>&1
    sleep 2.4
  done
  return 1
}

wait_for_exit() {
  local attempt pid
  pid="$(pgrep -f "$binary" | head -1)"
  for attempt in $(seq 1 15); do
    if [ -z "$(pgrep -f "$binary")" ]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

dismiss() {
  case "$1" in
    cmd-q)
      ./scripts/drive-app.sh quit >/dev/null 2>&1
      ;;
    close-box)
      osascript >/dev/null 2>&1 <<APPLESCRIPT
tell application "System Events" to tell process "$process"
  set frontmost to true
  delay 0.5
  click button 1 of window 1
end tell
APPLESCRIPT
      ;;
  esac
}

# One dismissal, from a clean library, against a confirmed live viewing.
check() {
  local how="$1" status=0
  rm -f "$database"
  cargo run -q -p screenroom-core --example prepare_library -- "$database" "$media" >/dev/null || return 1

  SCREENROOM_LIBRARY="$database" "$binary" >"$work/app.log" 2>&1 &
  local app=$!
  sleep 5

  if ! await_window; then
    echo "FAIL [$how]: the app never showed a window" >&2
    kill "$app" 2>/dev/null
    return 1
  fi
  if ! start_playing; then
    echo "FAIL [$how]: playback never started, so there is nothing to store" >&2
    kill "$app" 2>/dev/null
    return 1
  fi

  sleep 4
  local before_active before_position after_active after_position
  before_active="$(active_sessions)"
  before_position="$(stored_position)"
  echo "  before  [$how]: $before_active open session(s), $before_position ms stored"

  dismiss "$how"
  if ! wait_for_exit; then
    echo "FAIL [$how]: the app was still running after the dismissal" >&2
    kill "$app" 2>/dev/null
    return 1
  fi

  after_active="$(active_sessions)"
  after_position="$(stored_position)"
  echo "  after   [$how]: $after_active open session(s), $after_position ms stored"

  if [ "$before_active" != "1" ]; then
    echo "FAIL [$how]: the viewing was not open before the dismissal" >&2
    status=1
  fi
  if [ "$after_active" != "0" ]; then
    echo "FAIL [$how]: the session was left open" >&2
    status=1
  fi
  if [ "$after_position" -le 0 ]; then
    echo "FAIL [$how]: no position was stored" >&2
    status=1
  fi
  if [ "$status" -eq 0 ]; then
    echo "PASS [$how]: the final position was stored and the session closed"
  fi
  return "$status"
}

failures=0
for how in cmd-q close-box; do
  if [ "$mode" != "both" ] && [ "$mode" != "$how" ]; then
    continue
  fi
  echo "==> dismissing with $how"
  check "$how" || failures=$((failures + 1))
done

pkill -f "$binary" 2>/dev/null
if [ "$failures" -ne 0 ]; then
  echo "$failures dismissal(s) failed" >&2
  exit 1
fi
echo "all dismissals stored the viewing and closed its session"
