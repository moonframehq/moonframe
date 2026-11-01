#!/usr/bin/env bash
# Renders docs/hero/hero.html to docs/screenshots/hero.png, the README's
# first image, at 1800x1000 (900x500 at twice the density) with headless
# Chrome. Set CHROME to a Chrome or Chromium binary if none is found.
# Usage: scripts/render-hero.sh [OUT.png]
set -euo pipefail
root=$(cd "$(dirname -- "$0")/.." && pwd)
out=${1:-$root/docs/screenshots/hero.png}
chrome=${CHROME:-}
if [[ -z $chrome ]]; then
    for candidate in ~/Library/Caches/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-*/chrome-headless-shell \
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
        "/Applications/Chromium.app/Contents/MacOS/Chromium" /usr/bin/chromium /usr/bin/google-chrome; do
        [[ -x $candidate ]] && { chrome=$candidate; break; }
    done
fi
[[ -n $chrome ]] || { echo "no Chrome found: set CHROME to a Chrome or Chromium binary" >&2; exit 1; }
"$chrome" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=2 --allow-file-access-from-files \
    --window-size=900,500 --virtual-time-budget=3000 --screenshot="$out" "file://$root/docs/hero/hero.html" >/dev/null 2>&1
echo "$out"
