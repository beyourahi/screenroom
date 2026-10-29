#!/usr/bin/env bash
# Verifies the packaged macOS app without opening it.
#
#   scripts/verify-bundle.sh                    # checks dist/Screenroom.app
#   scripts/verify-bundle.sh /path/to/App.app   # checks another bundle
#
# Why this exists: `open`ing the app is the only way to see it, and a window on the
# desktop belongs to whoever is using the machine. Everything here is a file
# inspection or a process that exits by itself, so it takes no focus and never
# interrupts the owner (see "Testing without taking over the desktop" in AGENTS.md).
#
# What it proves, and each item is a real check rather than a restatement:
#
# * the bundle has the structure macOS actually requires;
# * `Info.plist` parses and names the executable it ships;
# * the bundled libmpv is a real file, not a symlink into Homebrew that will break
#   when the prefix changes;
# * every absolute dependency of that bundled libmpv resolves on this machine;
# * and — the one that matters — libmpv *from inside the bundle* loads and can
#   create a core. `capability` prefers system paths, so on a development machine it
#   would happily report Homebrew's copy; naming the bundled path is what makes this
#   evidence about the bundle rather than about the machine.
#
# What it cannot prove: that a frame reaches the screen. That needs a window.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

app="${1:-$root/dist/Screenroom.app}"
executable_name="Screenroom"

failures=0
fail() {
  echo "  FAIL: $*" >&2
  failures=$((failures + 1))
}
ok() { echo "  ok:   $*"; }

if [ ! -d "$app" ]; then
  echo "no bundle at $app" >&2
  echo "build one with: scripts/package-macos.sh" >&2
  exit 2
fi

echo "==> structure"
for required in "Contents/Info.plist" "Contents/MacOS/$executable_name"; do
  if [ -e "$app/$required" ]; then ok "$required"; else fail "$required is missing"; fi
done
if [ -x "$app/Contents/MacOS/$executable_name" ]; then
  ok "the executable is executable"
else
  fail "Contents/MacOS/$executable_name is not executable"
fi

echo
echo "==> Info.plist"
if plutil -lint "$app/Contents/Info.plist" >/dev/null 2>&1; then
  ok "plist parses"
  declared="$(plutil -extract CFBundleExecutable raw "$app/Contents/Info.plist" 2>/dev/null || true)"
  if [ "$declared" = "$executable_name" ]; then
    ok "CFBundleExecutable names $declared"
  else
    fail "CFBundleExecutable is '${declared:-unset}', but the binary is $executable_name"
  fi
  identifier="$(plutil -extract CFBundleIdentifier raw "$app/Contents/Info.plist" 2>/dev/null || true)"
  if [ -n "$identifier" ]; then ok "CFBundleIdentifier is $identifier"; else fail "no CFBundleIdentifier"; fi
else
  fail "Info.plist does not parse"
fi

echo
echo "==> bundled libmpv"
ls -1 "target/release/screenroom-desktop" >/dev/null 2>&1 || true
libmpv="$app/Contents/Frameworks/libmpv.2.dylib"
if [ ! -f "$libmpv" ]; then
  echo "  note: the bundle carries no libmpv, so playback will report itself unavailable."
  echo "        Install one with: brew install mpv, then re-run scripts/package-macos.sh"
else
  # `cp -L` must have followed Homebrew's symlink; a symlink here would dangle the
  # moment the linked version is upgraded in place.
  if [ -L "$libmpv" ]; then
    fail "Contents/Frameworks/libmpv.2.dylib is a symlink, not a real file"
  else
    ok "libmpv.2.dylib is a real file ($(du -h "$libmpv" | cut -f1 | tr -d ' '))"
  fi
  if file "$libmpv" | grep -q 'Mach-O'; then
    ok "libmpv.2.dylib is a Mach-O library ($(lipo -archs "$libmpv" 2>/dev/null || echo unknown))"
  else
    fail "libmpv.2.dylib is not a Mach-O library"
  fi

  echo
  echo "==> libmpv runtime dependencies (must exist on the target machine)"
  # These are absolute install names pointing into /opt/homebrew/opt, so the bundle
  # runs on a machine with the same Homebrew mpv installed. Vendoring FFmpeg would
  # remove that requirement and is a separate licensing decision, so the situation
  # is reported rather than hidden.
  missing=0
  while IFS= read -r dependency; do
    if [ -e "$dependency" ]; then
      echo "  ok:   $dependency"
    else
      echo "  MISSING: $dependency" >&2
      missing=$((missing + 1))
    fi
  done < <(otool -L "$libmpv" | tail -n +2 | awk '{print $1}' | grep -E '^/(opt|usr/local)/' || true)
  if [ "$missing" -eq 0 ]; then
    ok "every absolute dependency resolves on this machine"
  else
    fail "$missing dependency/dependencies missing; playback would fail here"
  fi

  echo
  echo "==> the bundle's own libmpv loads (no window, no focus taken)"
  if output="$(cargo run --quiet -p screenroom-player --example capability -- "$libmpv" 2>&1)"; then
    echo "$output" | sed 's/^/  /'
    ok "the copy inside the bundle created a real libmpv core"
  else
    echo "$output" | sed 's/^/  /' >&2
    fail "the bundled libmpv did not load"
  fi
fi

echo
echo "==> signature and permissions"
if codesign --verify --deep --strict "$app" 2>/dev/null; then
  ok "the bundle's ad-hoc signature verifies, libmpv included"
else
  fail "codesign --verify --deep --strict rejects the bundle"
fi
for key in NSRemovableVolumesUsageDescription NSDocumentsFolderUsageDescription; do
  if plutil -extract "$key" raw "$app/Contents/Info.plist" >/dev/null 2>&1; then
    ok "$key explains the access prompt"
  else
    fail "no $key, so macOS asks for drive access without saying why"
  fi
done

echo
echo "==> media tools the installed app runs (posters, runtimes)"
for tool in ffmpeg ffprobe; do
  if [ -x "/opt/homebrew/bin/$tool" ] || [ -x "/usr/local/bin/$tool" ]; then
    ok "$tool is installed where a Dock launch can find it"
  else
    echo "  note: no $tool in /opt/homebrew/bin or /usr/local/bin; cards will have no posters or runtimes."
  fi
done

echo
if [ "$failures" -ne 0 ]; then
  echo "$failures check(s) failed." >&2
  exit 1
fi
cat <<'NOTE'
All checks passed.

Not proven here, by design: that a frame reaches the screen, or that the window
opens at all. Both need a visible window, which is an owner-approved session —
see docs/TEST_PLAN.md.
NOTE
