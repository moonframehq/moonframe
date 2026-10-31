#!/usr/bin/env bash
# Capture a Moonframe window to a PNG, so an agent can see what its Luau
# change drew. macOS only; needs Screen Recording permission for the
# terminal. Usage: scripts/screenshot.sh [OUT.png] [PID]
# Pass the PID of the window you started, so a window someone else has open
# is never captured instead; without one it takes the first Moonframe window.
set -euo pipefail
out="${1:-target/screenshot.png}"
pid="${2:-}"
mkdir -p "$(dirname "$out")"
window=$(swift - "$pid" <<'SWIFT'
import CoreGraphics
let pid = CommandLine.arguments.count > 1 ? Int(CommandLine.arguments[1]) : nil
let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows where (window["kCGWindowOwnerName"] as? String)?.lowercased() == "moonframe"
    && window["kCGWindowLayer"] as? Int == 0
    && (pid == nil || window["kCGWindowOwnerPID"] as? Int == pid) {
    print(window["kCGWindowNumber"] ?? "")
    break
}
SWIFT
)
if [ -z "$window" ]; then
    echo "no Moonframe window${pid:+ for pid $pid} is open; start it with: just run" >&2
    exit 1
fi
screencapture -x -o -l"$window" "$out"
echo "$out"
