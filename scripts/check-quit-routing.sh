#!/usr/bin/env bash
# Checks that the menu bar's Quit is re-pointed at the window's close action.
#
#   scripts/check-quit-routing.sh
#
# This is the check to reach for. It launches the real application — because the
# menu bar this changes only exists inside one — with a **hidden** window, reads the
# outcome, and exits. Nothing is shown, nothing is activated, no key is sent and no
# Space moves, so it is safe to run while using the machine.
#
# It answers one question: was the Quit command re-pointed. It cannot answer
# whether a *dismissal* stores the viewing, because that needs a real window and a
# real dismissal; `scripts/verify-quit.sh` does that, and says out loud that it
# takes over the desktop.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "this check is about the macOS menu bar; nothing to check on $(uname -s)"
  exit 0
fi

work="${TMPDIR:-/tmp}/screenroom-routing-check"
rm -rf "$work"
mkdir -p "$work"

report="$work/routing.txt"
library="$work/library.db"
binary="target/debug/screenroom-desktop"

echo "==> building"
cargo build -q -p screenroom-desktop || exit 1

# The application opens a library on start. An in-memory one would do, but the
# launch takes the same path the installed app does when the file exists, and a
# scratch file keeps this from touching the library a person is actually using.
SCREENROOM_LIBRARY="$library" SCREENROOM_QUIT_CHECK="$report" \
  "$binary" >"$work/app.log" 2>&1 &
app=$!

# The window is hidden, so the only thing that can hang this is the application
# failing to start at all. Waiting on the file rather than sleeping means a fast
# answer is used immediately.
for _ in $(seq 1 40); do
  [ -s "$report" ] && break
  sleep 0.25
done

if [ ! -s "$report" ]; then
  echo "FAIL: the application did not report within 10s" >&2
  sed 's/^/    /' "$work/app.log" >&2 2>/dev/null || true
  kill "$app" 2>/dev/null
  exit 1
fi

outcome="$(tr -d '[:space:]' <"$report")"
kill "$app" 2>/dev/null
wait "$app" 2>/dev/null

echo "  reported: $outcome"
case "$outcome" in
  routed)
    echo "PASS: the menu bar's Quit performs the window's close action"
    ;;
  unsupported)
    echo "FAIL: the routing was reported unsupported on macOS" >&2
    exit 1
    ;;
  *)
    echo "FAIL: the Quit item was not routed ($outcome)" >&2
    echo "      See the note above 'route_quit' in apps/desktop/src/app.rs." >&2
    exit 1
    ;;
esac
