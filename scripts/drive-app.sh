#!/usr/bin/env bash
# Sends keys to a running Screenroom window and captures evidence.
#
# macOS delivers synthetic keys only when the target window is raised inside the
# same AppleScript block, so every helper does raise+nudge+key in one call.
#
#   scripts/drive-app.sh home          # ⌘1
#   scripts/drive-app.sh library       # ⌘2
#   scripts/drive-app.sh settings      # ⌘3
#   scripts/drive-app.sh enter|space|left|right|escape
#   scripts/drive-app.sh close|quit    # ⌘W closes the window, ⌘Q terminates
#   scripts/drive-app.sh ⌘r            # any "⌘<char>"
#   scripts/drive-app.sh capture NAME  # raise, wait, screenshot to .artifacts/evidence/NAME
#
# Requires Accessibility + Screen Recording grants for the invoking terminal.
set -uo pipefail

process="${SCREENROOM_PROCESS:-screenroom-desktop}"
action="${1:?usage: drive-app.sh <key|capture> [name]}"

# macOS occasionally reports "window 1 ... Invalid index" while the previous
# raise is still settling, so every action is retried before it is called failed.
# Activating the process is what actually delivers the key, and it is also what
# brings a window back from another Space. `AXRaise` is the icing — but it throws
# while the window is on a Space the accessibility API cannot see, and that used
# to abort the whole block before the key was sent, so it is best-effort for the
# same reason the capture helper treats it that way.
raise_block() {
  local script
  script=$(cat <<APPLESCRIPT
tell application "System Events"
  tell process "$process"
    set frontmost to true
    try
      perform action "AXRaise" of window 1
    end try
    delay 0.45
$1
  end tell
end tell
APPLESCRIPT
)
  local attempt=1
  while [ "$attempt" -le 4 ]; do
    if osascript <<<"$script" 2>/dev/null; then
      return 0
    fi
    sleep 0.5
    attempt=$((attempt + 1))
  done
  echo "failed to drive $process after 4 attempts" >&2
  return 1
}

case "$action" in
  enter) raise_block 'key code 36' ;;
  space) raise_block 'key code 49' ;;
  tab) raise_block 'key code 48' ;;
  backspace) raise_block 'key code 51' ;;
  escape) raise_block 'key code 53' ;;
  left) raise_block 'key code 123' ;;
  right) raise_block 'key code 124' ;;
  up) raise_block 'key code 126' ;;
  down) raise_block 'key code 125' ;;
  home) raise_block 'keystroke "1" using command down' ;;
  library) raise_block 'keystroke "2" using command down' ;;
  settings) raise_block 'keystroke "3" using command down' ;;
  refresh) raise_block 'keystroke "r" using command down' ;;
  close) raise_block 'keystroke "w" using command down' ;;
  quit) raise_block 'keystroke "q" using command down' ;;
  type:*)
    raise_block "keystroke \"${action#type:}\"" ;;
  capture)
    name="${2:?usage: drive-app.sh capture NAME}"
    raise_block 'delay 0.2'
    ./scripts/capture-window.sh ".artifacts/evidence/$name"
    sips -Z 1000 ".artifacts/evidence/$name" \
      --out ".artifacts/evidence/${name%.png}-small.png" >/dev/null
    echo "captured $name"
    ;;
  *)
    echo "unknown action: $action" >&2
    exit 2 ;;
esac
