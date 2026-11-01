#!/usr/bin/env bash
# Packages the macOS release in OUT_DIR from one executable per
# architecture: a universal Moonframe.app (scripts/bundle-app.sh, signed ad
# hoc), Moonframe.app.tar for the signing job, and the disk image users
# download, Moonframe-VERSION-universal-apple-darwin.dmg. Developer ID
# signing and notarization happen later, in scripts/release/sign-macos.sh.
set -euo pipefail
[[ $# -ge 2 ]] || { echo "usage: package-macos.sh OUT_DIR BINARY..." >&2; exit 2; }
[[ $(uname -s) == Darwin ]] || { echo "macOS required" >&2; exit 1; }
out=$1
shift
version=$(scripts/release/version.sh)
mkdir -p "$out"
out=$(cd "$out" && pwd)

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
lipo -create "$@" -output "$work/moonframe"
lipo -info "$work/moonframe"

MOONFRAME_BINARY="$work/moonframe" scripts/bundle-app.sh "$work/bundle" >/dev/null
app="$work/bundle/Moonframe.app"
codesign --verify --strict "$app"
# The bundled executable must find the app inside the bundle and load every
# module of it, headlessly, on each architecture this Mac can run.
for arch in $(lipo -archs "$app/Contents/MacOS/moonframe"); do
    if arch "-$arch" /usr/bin/true 2>/dev/null; then
        arch "-$arch" "$app/Contents/MacOS/moonframe" --check --no-user
    else
        echo "skipping the $arch check: this Mac cannot run it" >&2
    fi
done

tar -C "$work/bundle" -cf "$out/Moonframe.app.tar" Moonframe.app
scripts/release/make-dmg.sh "$app" "$out/Moonframe-$version-universal-apple-darwin.dmg"
