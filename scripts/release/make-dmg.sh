#!/usr/bin/env bash
# Puts APP in a compressed disk image at DMG beside an Applications
# shortcut, so installing is dragging one onto the other, in a window laid
# out by assets/macos/dmg: no toolbar, the two icons over a background with
# an arrow between them, and the app's icon on the volume. dmgbuild (pinned
# by hash in requirements.txt there) writes that layout against the volume
# it builds; it needs no Finder or AppleScript, so it runs headless on CI.
set -euo pipefail
[[ $# == 2 ]] || { echo "usage: make-dmg.sh APP DMG" >&2; exit 2; }
app=$1
dmg=$2
[[ -d $app/Contents/MacOS ]] || { echo "not an app bundle: $app" >&2; exit 1; }
[[ $(basename "$app") == Moonframe.app ]] || { echo "the app must be Moonframe.app: $app" >&2; exit 1; }
[[ ! -e $dmg ]] || { echo "refusing to overwrite $dmg" >&2; exit 1; }
layout=$(cd "$(dirname -- "$0")/../../assets/macos/dmg" && pwd)
icon=$app/Contents/Resources/AppIcon.icns
[[ -f $icon ]] || { echo "the app has no icon: $icon" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
python3 -m venv "$work/venv"
"$work/venv/bin/python" -m pip install --quiet --disable-pip-version-check --no-cache-dir \
    --require-hashes --no-deps --only-binary :all: -r "$layout/requirements.txt"
"$work/venv/bin/dmgbuild" -s "$layout/settings.py" \
    -D app="$app" -D icon="$icon" -D background="$layout/background.tiff" \
    Moonframe "$dmg" >/dev/null
hdiutil verify -quiet "$dmg"
echo "$dmg"
