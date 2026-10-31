#!/usr/bin/env bash
# Builds Moonframe.app (scripts/bundle-app.sh) and puts it in
# /Applications, replacing an older copy. Quit the installed app first: a
# running copy keeps the old binary until it restarts.
set -euo pipefail

bundle=$(scripts/bundle-app.sh target/bundle | tail -1)
target=/Applications/Moonframe.app
if pgrep -f "$target/Contents/MacOS/moonframe" >/dev/null; then
    echo "note: Moonframe is running from $target; restart it to use the new build" >&2
fi
rm -rf "$target"
ditto "$bundle" "$target"
echo "installed $target"
echo "your mods, providers and style.luau: ${XDG_CONFIG_HOME:-$HOME/.config}/moonframe"
