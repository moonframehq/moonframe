#!/usr/bin/env bash
# Builds Moonframe.app in OUT (default target/bundle): the release binary,
# the built-in app as it ships (scripts/stage-app.sh: Wasm providers
# prebuilt, no sources) in Contents/Resources/app, the icon, and an
# Info.plist, signed ad hoc so it runs on this Mac. The user's mods,
# providers and stylesheet live in ~/.config/moonframe, never here.
#
# MOONFRAME_BINARY bundles a prebuilt executable (the release's universal
# one) instead of building for this Mac.
set -euo pipefail

out=${1:-target/bundle}
name=moonframe
title=Moonframe
bundle="$out/$title.app"
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
binary=${MOONFRAME_BINARY:-}

if [ -z "$binary" ]; then
    cargo build --quiet --release --locked -p moonframe
    binary="target/release/$name"
fi

rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$binary" "$bundle/Contents/MacOS/$name"
scripts/stage-app.sh "$bundle/Contents/Resources/app" >/dev/null

# The icon, at every size Finder and the Dock ask for.
iconset=$(mktemp -d)/AppIcon.iconset
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z $size $size assets/icon.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    [ $double -le 512 ] && sips -z $double $double assets/icon.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$bundle/Contents/Resources/AppIcon.icns"
rm -rf "$(dirname "$iconset")"

cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$title</string>
    <key>CFBundleDisplayName</key><string>$title</string>
    <key>CFBundleIdentifier</key><string>dev.moonframe.app</string>
    <key>CFBundleExecutable</key><string>$name</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

cp LICENSE "$bundle/Contents/Resources/LICENSE"

codesign --force --deep --sign - "$bundle" 2>/dev/null
echo "$bundle"
