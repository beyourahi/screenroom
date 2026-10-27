#!/usr/bin/env bash
# A gate for the few checks that need a visible window on this desktop.
#
# Why this exists: `cargo test --workspace` and the headless checks never show a
# window. The two scripts that do — driving a window with synthetic keys, and
# activating it to capture it — take over the Mac while they run: they activate
# Screenroom, can switch Spaces, and send keyboard events to whatever is focused.
# That is unusable during ordinary work on the same machine, so it is opt-in
# rather than something a script does because it was called.
#
# Source this, then call `screenroom_require_foreground "<what this does>"`.
#
#   SCREENROOM_ALLOW_FOREGROUND=1 scripts/drive-app.sh library
#
# Nothing here is a security boundary. It is a courtesy to the person using the
# machine, and the default answer is no.
set -uo pipefail

screenroom_require_foreground() {
  local purpose="${1:-this check}"
  if [ "${SCREENROOM_ALLOW_FOREGROUND:-0}" = "1" ]; then
    return 0
  fi
  cat >&2 <<NOTE
Refusing to take over the desktop.

  $purpose.

  That needs a visible Screenroom window on the active desktop: it activates the
  application, may switch Spaces, and sends synthetic keyboard or pointer events.
  While it runs, this Mac is not yours.

  To allow it once, run the same command with SCREENROOM_ALLOW_FOREGROUND=1.

  Most checks do not need this. In order of what they cost the machine:

    cargo test --workspace                     # no window at all
    cargo run -p screenroom-player --example capability
    scripts/check-quit-routing.sh              # hidden window, no focus taken
    scripts/verify-quit.sh                     # needs SCREENROOM_ALLOW_FOREGROUND=1

  See docs/TEST_PLAN.md, "Testing without taking over the desktop".
NOTE
  exit 3
}
