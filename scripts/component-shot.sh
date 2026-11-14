#!/usr/bin/env bash
# Capture one component's page in the components window to a PNG, from an
# instance of the app this script starts in the background (it never takes
# focus) and stops again. Works on macOS, Linux and Windows (Git Bash);
# window-shot.sh lists what each needs. On Linux the window opens under X11
# (XWayland on a Wayland desktop), or on a virtual display started with
# Xvfb when there is no desktop at all.
# Usage: scripts/component-shot.sh NAME [OUT.png] [DIR]
# PREVIEW_ONLY=1 captures the preview alone, filling the window
# (--preview-only). SHOT_HEIGHT=N makes the window N points tall first (as
# far as the screen allows), for tall previews; on macOS it needs
# Accessibility permission.
set -euo pipefail
# shellcheck source=scripts/window-shot.sh
source "$(dirname "$0")/window-shot.sh"
name="${1:?usage: component-shot.sh NAME [OUT.png] [DIR]}"
out="${2:-target/components/$name.png}"
dir="${3:-app}"
mkdir -p "$(dirname "$out")"
binary=target/release/moonframe
# Always build: an old binary would show yesterday's UI. A no-op when current.
cargo build --quiet --release -p moonframe-app
pid=""
trap '[ -n "$pid" ] && kill "$pid" 2>/dev/null; [ -n "$shot_xvfb" ] && kill "$shot_xvfb" 2>/dev/null; true' EXIT
shot_virtual_display
if [ "$(shot_os)" = linux ] && [ -n "${DISPLAY:-}" ]; then
    # X11 can find and capture a window by its process; Wayland cannot.
    unset WAYLAND_DISPLAY
    shot_need xdotool import
fi
"$binary" --no-user --background ${PREVIEW_ONLY:+--preview-only} --component "$name" "$dir" >/dev/null 2>&1 &
pid=$!
window=""
for _ in $(seq 1 40); do
    sleep 0.25
    window=$(shot_find "$(shot_native_pid "$pid")" Components)
    [ -n "$window" ] && break
done
if [ -z "$window" ]; then
    echo "the components window did not open" >&2
    exit 1
fi
if [ -n "${SHOT_HEIGHT:-}" ]; then
    shot_resize "$(shot_native_pid "$pid")" "$window" Components "$SHOT_HEIGHT" ||
        echo "could not resize the window; capturing it as it opened" >&2
fi
# Let the first frames draw.
sleep 1
shot_capture "$window" "$out"
echo "$out"
