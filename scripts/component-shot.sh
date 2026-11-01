#!/usr/bin/env bash
# Capture one component's page in the components window to a PNG, from an
# instance of the app this script starts in the background (it never takes
# focus) and stops again. macOS only; needs Screen Recording permission.
# Usage: scripts/component-shot.sh NAME [OUT.png] [DIR]
set -euo pipefail
name="${1:?usage: component-shot.sh NAME [OUT.png] [DIR]}"
out="${2:-target/components/$name.png}"
dir="${3:-app}"
mkdir -p "$(dirname "$out")"
binary=target/release/moonframe
# Always build: an old binary would show yesterday's UI. A no-op when current.
cargo build --quiet --release -p moonframe-app
"$binary" --no-user --background --component "$name" "$dir" >/dev/null 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT
window=""
for _ in $(seq 1 40); do
    sleep 0.25
    window=$(swift - "$pid" <<'SWIFT'
import CoreGraphics
let pid = Int(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows where window["kCGWindowOwnerPID"] as? Int == pid
    && window["kCGWindowName"] as? String == "Components" {
    print(window["kCGWindowNumber"] ?? "")
    break
}
SWIFT
)
    [ -n "$window" ] && break
done
if [ -z "$window" ]; then
    echo "the components window did not open" >&2
    exit 1
fi
# Let the first frames draw.
sleep 1
screencapture -x -o -l"$window" "$out"
echo "$out"
