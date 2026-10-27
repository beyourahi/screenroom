#!/usr/bin/env bash
# Builds a locally installable Screenroom.app for macOS.
#
#   scripts/package-macos.sh                 # -> dist/Screenroom.app
#   scripts/package-macos.sh /Applications   # install into a folder afterwards
#
# What this produces and what it deliberately does not:
#
# * The bundle carries its own copy of libmpv in `Contents/Frameworks`. The app
#   looks there after the system paths (see `packaged_candidates` in
#   `crates/screenroom-player`), so an app installed from `dist` keeps working
#   after the Homebrew prefix changes or a development copy is removed.
# * libmpv's *own* dependencies — FFmpeg, libass, libplacebo and the rest — are
#   resolved by their absolute install names, which on this host point into
#   `/opt/homebrew/opt`. So this bundle runs on a machine with the same Homebrew
#   mpv installed. Making it run anywhere would mean vendoring FFmpeg, which is a
#   separate licensing decision (FFmpeg's build options decide which codecs may be
#   redistributed) and is not implied by a personal install. The script checks the
#   dependency situation and reports it rather than hiding it.
# * No code signing and no notarization: this is the approved personal
#   installation path, and neither is required to run an app you built yourself.
#   Gatekeeper still asks once on first launch; see the note the script prints.
#
# Nothing here needs Xcode beyond the command line tools already used to build.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

install_into="${1:-}"
app="$root/dist/Screenroom.app"
bundle_id="com.screenroom.desktop"
process="Screenroom"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "this script packages a macOS app; $(uname -s) is not macOS" >&2
  exit 2
fi

echo "==> building the release binary"
cargo build --release -p screenroom-desktop

binary="$root/target/release/screenroom-desktop"
[ -x "$binary" ] || { echo "missing $binary" >&2; exit 1; }

echo "==> assembling $app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Frameworks"
cp "$binary" "$app/Contents/MacOS/$process"
chmod +x "$app/Contents/MacOS/$process"

# The icon is optional: a missing one is not a packaging failure, it just leaves
# the bundle with the default application icon.
if [ -f "$root/assets/Screenroom.icns" ]; then
  cp "$root/assets/Screenroom.icns" "$app/Contents/Resources/Screenroom.icns"
fi

# ---------------------------------------------------------------- libmpv

libmpv=""
for candidate in \
  /opt/homebrew/lib/libmpv.2.dylib \
  /usr/local/lib/libmpv.2.dylib \
  /opt/homebrew/lib/libmpv.dylib \
  /usr/local/lib/libmpv.dylib
do
  if [ -f "$candidate" ]; then
    libmpv="$candidate"
    break
  fi
done

if [ -z "$libmpv" ]; then
  echo "!! no libmpv found, so the app will report playback as unavailable." >&2
  echo "   Install it with: brew install mpv" >&2
else
  # `cp -L` follows the Homebrew symlink, so the bundle holds a real file and
  # does not break when the linked version is upgraded in place.
  cp -L "$libmpv" "$app/Contents/Frameworks/libmpv.2.dylib"
  echo "==> bundled $libmpv"
  # `readlink -f` is not available on BSD userland, so the symlink is resolved
  # with `cd`/`pwd -P` on its directory instead.
  resolved="$(cd "$(dirname "$libmpv")" && pwd -P)/$(basename "$libmpv")"
  echo "    libmpv dependencies that must be present on this machine:"
  otool -L "$resolved" 2>/dev/null \
    | tail -n +2 \
    | awk '{print $1}' \
    | grep -E '^/(opt|usr/local)/' \
    | sed 's/^/      /' \
    || true
  missing=0
  while IFS= read -r dependency; do
    [ -e "$dependency" ] || { echo "      MISSING: $dependency" >&2; missing=1; }
  done < <(otool -L "$resolved" 2>/dev/null | tail -n +2 | awk '{print $1}' | grep -E '^/(opt|usr/local)/')
  if [ "$missing" -ne 0 ]; then
    echo "!! some libmpv dependencies are absent; playback will fail on this machine." >&2
    exit 1
  fi
fi

# ---------------------------------------------------------------- Info.plist

version="$(grep -m1 '^version' "$root/Cargo.toml" | sed -E 's/.*"(.*)".*/\1/')"
version="${version:-0.1.0}"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Screenroom</string>
  <key>CFBundleDisplayName</key><string>Screenroom</string>
  <key>CFBundleIdentifier</key><string>$bundle_id</string>
  <key>CFBundleExecutable</key><string>$process</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.video</string>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>CFBundleIconFile</key><string>Screenroom</string>
</dict>
</plist>
PLIST

# An unsigned bundle still launches, but macOS quarantines anything downloaded.
# Clearing the attribute here is correct for a bundle built on this machine and
# has no effect on a copy that arrives with a quarantine flag already set.
xattr -cr "$app" 2>/dev/null || true

echo "==> built $app"
plutil -lint "$app/Contents/Info.plist" >/dev/null && echo "    Info.plist is valid"
du -sh "$app" | sed 's/^/    size: /'

if [ -n "$install_into" ]; then
  mkdir -p "$install_into"
  rm -rf "$install_into/Screenroom.app"
  cp -R "$app" "$install_into/Screenroom.app"
  echo "==> installed $install_into/Screenroom.app"
fi

cat <<'NOTE'

Run it with:
  open dist/Screenroom.app

The first launch of a locally built app may show Gatekeeper's "unidentified
developer" prompt. Right-click the app and choose Open once, or open
System Settings > Privacy & Security and press "Open Anyway".

Settings (viewer profiles, folders, backup) live in the app's own data
directory, which is the same one the development build uses, so an installed
app opens the library you have already been building.
NOTE
