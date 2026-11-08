#!/usr/bin/env bash
# Packages each app scripts/release/apps.py names in OUT_DIR, from the
# release's executable (a bundle is that executable and the app's files, so
# nothing is compiled per app but its Wasm providers): on macOS a universal
# Title-VERSION-universal-apple-darwin.zip each and apps.tar for the signing
# job, on Linux a Title-VERSION-TARGET.tar.gz each. Each is checked headlessly
# against its own app. Developer ID signing happens later, in sign-apps.sh.
set -euo pipefail
[[ $# -ge 2 ]] || { echo "usage: package-apps.sh OUT_DIR BINARY..." >&2; exit 2; }
out=$1
shift
mkdir -p "$out"
out=$(cd "$out" && pwd)

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
binary=$work/moonframe
if [[ $(uname -s) == Darwin ]]; then
    if [[ $# -gt 1 ]]; then
        lipo -create "$@" -output "$binary"
    else
        cp "$1" "$binary"
    fi
    # A signature made for another bundle does not run outside it.
    codesign --remove-signature "$binary"
    codesign --force --sign - "$binary"
else
    cp "$1" "$binary"
fi

# With a user folder, as a bundle's app is only the app with one; without,
# its check would load the built-in app and pass whatever this one holds.
check() {
    local executable=$1 user output
    shift
    user=$(mktemp -d "$work/check.XXXXXX")
    output=$("$@" "$executable" --user "$user" --check)
    echo "$output" | cut -c1-120
    [[ $output == *bundle/app* ]] || { echo "$executable did not load its own app" >&2; exit 1; }
}

user=$work/user
mkdir -p "$user"
apps=$(python3 scripts/release/apps.py names)
[[ -n $apps ]] || { echo "no app has a [release] table" >&2; exit 1; }
for name in $apps; do
    "$binary" --user "$user" app link "examples/$name" --yes >/dev/null
    bundle=$(DIST=$out MOONFRAME_BINARY=$binary MOONFRAME_USER=$user \
        scripts/bundle-app.sh "$work/bundle" "$name" | tail -1)
    if [[ $(uname -s) == Darwin ]]; then
        codesign --verify --strict "$bundle"
        executable=$bundle/Contents/MacOS/moonframe
        for arch in $(lipo -archs "$executable"); do
            if arch "-$arch" /usr/bin/true 2>/dev/null; then
                check "$executable" arch "-$arch"
            else
                echo "skipping the $arch check: this Mac cannot run it" >&2
            fi
        done
    else
        check "$bundle/bin/moonframe-$name" env
    fi
done

# Written over all the platforms' files when the release is put together.
rm -f -- "$out/SHA256SUMS"
if [[ $(uname -s) == Darwin ]]; then
    tar -C "$work/bundle" -cf "$out/apps.tar" .
fi
