#!/usr/bin/env bash
# Capture a Moonframe window to a PNG, so an agent can see what its Luau
# change drew. Works on macOS, Linux and Windows (Git Bash); window-shot.sh
# lists what each needs. Usage: scripts/screenshot.sh [OUT.png] [PID]
# Pass the PID of the window you started, so a window someone else has open
# is never captured instead; without one it takes the first Moonframe window.
set -euo pipefail
# shellcheck source=scripts/window-shot.sh
source "$(dirname "$0")/window-shot.sh"
out="${1:-target/screenshot.png}"
pid="${2:-}"
mkdir -p "$(dirname "$out")"
window=$(shot_find "$pid")
if [ -z "$window" ]; then
    echo "no Moonframe window${pid:+ for pid $pid} is open; start it with: just run" >&2
    if [ "$(shot_os)" = linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
        echo "on Wayland outside Sway and Hyprland, start it under XWayland: env -u WAYLAND_DISPLAY just run" >&2
    fi
    exit 1
fi
shot_capture "$window" "$out"
echo "$out"
