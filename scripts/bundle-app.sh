#!/usr/bin/env bash
# Builds an app bundle in OUT (default target/bundle): the release binary,
# the built-in app as it ships (scripts/stage-app.sh: Wasm providers
# prebuilt, no sources), and the icon. On macOS it is NAME.app, with an
# Info.plist, signed ad hoc so it runs on this Mac; on Linux a folder laid
# out like a prefix (bin/, share/moonframe/, share/applications/ with a
# .desktop file, share/icons/) to copy under /usr/local or ~/.local.
#
# With no PROFILE it is Moonframe.app, the starter: the user's mods,
# providers and stylesheet live in ~/.config/moonframe, never here.
#
# With PROFILE it is that profile's app, standalone: its app package and the
# packages it requires are staged in Contents/Resources/bundle, it takes the
# app's title, id (dev.moonframe.app.NAME) and icon (its manifest's `icon`,
# else icon.png), and it runs as a release, without the Components menu or
# Inspect Element. What its users add goes in
# ~/.config/moonframe/profiles/NAME. MOONFRAME_USER names another user
# folder to find the profile in.
#
# MOONFRAME_BINARY bundles a prebuilt executable (the release's universal
# one) instead of building for this Mac.
set -euo pipefail

out=${1:-target/bundle}
profile=${2:-}
exe=moonframe
title=Moonframe
id=dev.moonframe.app
icon=assets/icon.png
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
binary=${MOONFRAME_BINARY:-}

if [ -z "$binary" ]; then
    cargo build --quiet --release --locked -p moonframe-app
    binary="target/release/$exe"
fi

app=
packages=()
if [ -n "$profile" ]; then
    while IFS=$'\t' read -r kind value; do
        case $kind in
            name) id="$id.$value" ;;
            title) title=$value ;;
            app) app=$value ;;
            icon) icon=$value ;;
            package) packages+=("$value") ;;
        esac
    done < <("$binary" ${MOONFRAME_USER:+--user "$MOONFRAME_USER"} app paths "$profile")
    [ -n "$app" ] || { echo "no app in the profile $profile" >&2; exit 1; }
    case $title in
        "" | .* | */* | *'\'* | *'<'* | *'>'* | *'&'* | *:*)
            echo "the app's title \`$title\` cannot name a bundle" >&2
            exit 1
            ;;
    esac
fi

# Stages the app, and the profile's app with its packages, under $1.
stage() {
    scripts/stage-app.sh "$1/app" >/dev/null
    if [ -n "$app" ]; then
        scripts/stage-app.sh "$1/bundle/app" "$app" >/dev/null
        for package in "${packages[@]}"; do
            staged="$1/bundle/packages/$(basename "$package")"
            # Two packages of one name would land in one folder, the second
            # quietly over the first.
            if [ -e "$staged" ]; then
                echo "two required packages are named \`$(basename "$package")\`; a bundle needs distinct names" >&2
                exit 1
            fi
            scripts/stage-app.sh "$staged" "$package" >/dev/null
        done
        # What git keeps for itself is no part of the app.
        find "$1/bundle" -name .git -prune -exec rm -rf {} +
    fi
}

if [ "$(uname)" != Darwin ]; then
    # Linux: a prefix. Each app's binary has a name of its own, and finds
    # its files in share/<that name>. The window system matches the
    # window's app id to the .desktop file of that name, which names the
    # icon.
    bin=$exe${profile:+-$profile}
    bundle="$out/$title"
    rm -rf "$bundle"
    mkdir -p "$bundle/bin" "$bundle/share/$bin" "$bundle/share/applications" \
        "$bundle/share/icons/hicolor/512x512/apps"
    cp "$binary" "$bundle/bin/$bin"
    stage "$bundle/share/$bin"
    cp "$icon" "$bundle/share/icons/hicolor/512x512/apps/$id.png"
    cat > "$bundle/share/applications/$id.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=$title
Exec=$bin
Icon=$id
StartupWMClass=$id
Categories=Development;
DESKTOP
    echo "$bundle"
    exit 0
fi

bundle="$out/$title.app"
rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$binary" "$bundle/Contents/MacOS/$exe"
stage "$bundle/Contents/Resources"

# The icon, at every size Finder and the Dock ask for.
iconset=$(mktemp -d)/AppIcon.iconset
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z $size $size "$icon" --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    [ $double -le 512 ] && sips -z $double $double "$icon" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
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
    <key>CFBundleIdentifier</key><string>$id</string>
    <key>CFBundleExecutable</key><string>$exe</string>
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
