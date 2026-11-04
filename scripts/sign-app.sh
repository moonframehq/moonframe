#!/usr/bin/env bash
# Signs an app bundle built by `just bundle-app`, for distribution: every
# binary inside it first (a payload's Python, its .so and .dylib files),
# then the app, each with the hardened runtime. Apple refuses to notarize an
# app with any unsigned binary inside, and a signature made with --deep
# doesn't carry the options or timestamp a nested binary needs.
#
#   scripts/sign-app.sh APP IDENTITY [ENTITLEMENTS]
#
# IDENTITY is `Developer ID Application: Name (TEAMID)`, or `-` to sign ad
# hoc, which proves the script ran and nothing else.
#
# NOTARY_PROFILE=NAME then notarizes and staples it, with credentials kept by
# `xcrun notarytool store-credentials NAME`.
set -euo pipefail

app=${1:?usage: sign-app.sh APP IDENTITY [ENTITLEMENTS]}
identity=${2:?usage: sign-app.sh APP IDENTITY [ENTITLEMENTS]}
entitlements=${3:-assets/macos/Moonframe.entitlements}
[ -d "$app/Contents/MacOS" ] || { echo "$app is not an app bundle" >&2; exit 1; }

options=(--force --options runtime --sign "$identity")
[ "$identity" = - ] || options+=(--timestamp)

is_macho() {
    case $(file -b "$1") in
        *Mach-O*) return 0 ;;
        *) return 1 ;;
    esac
}

# Deepest first, so each signature covers what is already signed. A
# framework or nested app is signed whole, after what it holds.
while IFS= read -r -d '' path; do
    if [ -d "$path" ]; then
        codesign "${options[@]}" "$path"
    elif [ ! -L "$path" ] && is_macho "$path"; then
        # What the app runs needs the same rights as the app: a Python
        # that loads native modules, or compiles to executable memory.
        if [ -x "$path" ]; then
            codesign "${options[@]}" --entitlements "$entitlements" "$path"
        else
            codesign "${options[@]}" "$path"
        fi
    fi
done < <(find "$app/Contents" -depth \
    \( -type f -o -type l -o -name '*.framework' -o -name '*.app' -o -name '*.xpc' \) \
    ! -path "$app/Contents/_CodeSignature/*" -print0)

codesign "${options[@]}" --entitlements "$entitlements" "$app"
codesign --verify --deep --strict --verbose=2 "$app"

if [ -n "${NOTARY_PROFILE:-}" ]; then
    archive=$(mktemp -d)/$(basename "$app").zip
    ditto -c -k --sequesterRsrc --keepParent "$app" "$archive"
    xcrun notarytool submit "$archive" --keychain-profile "$NOTARY_PROFILE" --wait
    xcrun stapler staple "$app"
    rm -rf "$(dirname "$archive")"
fi
echo "$app"
