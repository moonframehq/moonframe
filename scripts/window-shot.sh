# shellcheck shell=bash
# Find a window of a process, resize it and capture it to a PNG, on macOS,
# Linux and Windows (Git Bash). Sourced by screenshot.sh and
# component-shot.sh, never run on its own.
#
# macOS: CoreGraphics finds the window and screencapture takes it; the
#   terminal needs Screen Recording permission (and Accessibility to resize).
# Linux: on X11 (or XWayland) xdotool finds the window and ImageMagick's
#   `import` takes it, covered or not. A window drawn on Wayland itself is
#   found on Sway or Hyprland only, and grim takes that part of the screen.
# Windows: PowerShell's PrintWindow (window-shot.ps1) takes the window even
#   when another one covers it.
#
# A window is named by a handle that only these functions read: a window
# number on macOS, an X window id, `wl:X,Y WxH` on Wayland, an HWND on
# Windows.

shot_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

shot_os() {
    case "$(uname -s)" in
        Darwin) echo macos ;;
        Linux) echo linux ;;
        MINGW* | MSYS* | CYGWIN*) echo windows ;;
        *) echo unknown ;;
    esac
}

# The PID Windows knows a process started from Git Bash by (`$!` is MSYS's
# own); the PID itself elsewhere.
shot_native_pid() {
    if [ "$(shot_os)" = windows ] && [ -r "/proc/$1/winpid" ]; then
        cat "/proc/$1/winpid"
    else
        echo "$1"
    fi
}

shot_need() {
    for tool in "$@"; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            echo "screenshots need $tool; $(shot_install_hint "$tool")" >&2
            return 1
        fi
    done
}

shot_install_hint() {
    case "$1" in
        xdotool) echo "install it with: sudo apt-get install xdotool" ;;
        import) echo "install ImageMagick with: sudo apt-get install imagemagick" ;;
        grim) echo "install it with: sudo apt-get install grim" ;;
        jq) echo "install it with: sudo apt-get install jq" ;;
        Xvfb) echo "install it with: sudo apt-get install xvfb" ;;
        *) echo "install it and retry" ;;
    esac
}

shot_ps() {
    powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass \
        -File "$(cygpath -w "$shot_dir/window-shot.ps1")" "$@" | tr -d '\r'
}

# Print the handle of PID's window titled TITLE (any title when empty), the
# largest when there are several, or nothing. Without a PID, a window of
# any process named moonframe.
shot_find() {
    local pid="$1" title="${2:-}"
    case "$(shot_os)" in
        macos) shot_find_macos "$pid" "$title" ;;
        linux) shot_find_linux "$pid" "$title" ;;
        windows) shot_ps -Action find ${pid:+-ProcessId "$pid"} ${title:+-Title "$title"} ;;
        *) echo "screenshots are not supported on $(uname -s)" >&2 ;;
    esac
}

shot_find_macos() {
    swift - "$1" "$2" <<'SWIFT'
import CoreGraphics
let args = CommandLine.arguments
let pid = args.count > 1 ? Int(args[1]) : nil
let title = args.count > 2 && !args[2].isEmpty ? args[2] : nil
let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] ?? []
// By PID when given: a bundled app goes by its own name, not Moonframe's.
for window in windows where window["kCGWindowLayer"] as? Int == 0
    && (pid.map { window["kCGWindowOwnerPID"] as? Int == $0 }
        ?? ((window["kCGWindowOwnerName"] as? String)?.lowercased() == "moonframe"))
    && title.map({ window["kCGWindowName"] as? String == $0 }) ?? true {
    print(window["kCGWindowNumber"] ?? "")
    break
}
SWIFT
}

shot_find_linux() {
    local pid="$1" title="$2" pids
    if [ -n "$pid" ]; then pids="$pid"; else pids="$(pgrep -x moonframe || true)"; fi
    [ -n "$pids" ] || return 0
    if [ -n "${DISPLAY:-}" ] && command -v xdotool >/dev/null 2>&1; then
        local found best="" area=0 id geometry w h
        for p in $pids; do
            # xdotool exits 1 when nothing matches.
            found="$(xdotool search --all --onlyvisible --pid "$p" ${title:+--name "^$title\$"} 2>/dev/null || true)"
            for id in $found; do
                geometry="$(xdotool getwindowgeometry --shell "$id")"
                w="$(sed -n 's/^WIDTH=//p' <<<"$geometry")"
                h="$(sed -n 's/^HEIGHT=//p' <<<"$geometry")"
                if [ $((w * h)) -gt "$area" ]; then best="$id" area=$((w * h)); fi
            done
        done
        if [ -n "$best" ]; then
            echo "$best"
            return 0
        fi
    fi
    [ -n "${WAYLAND_DISPLAY:-}" ] && shot_find_wayland "$pids" "$title"
    return 0
}

# A window drawn on Wayland: its place on screen, from the compositors that
# tell it (Sway, Hyprland). GNOME and KDE do not; there the window opens
# under XWayland with WAYLAND_DISPLAY unset, as component-shot.sh does.
shot_find_wayland() {
    local pids="$1" title="$2" json
    command -v jq >/dev/null 2>&1 || return 0
    local wanted
    wanted="$(tr ' ' '\n' <<<"$pids" | jq -Rn '[inputs | tonumber]')"
    if command -v hyprctl >/dev/null 2>&1 && json="$(hyprctl clients -j 2>/dev/null)"; then
        jq -r --argjson pids "$wanted" --arg title "$title" '
            [.[] | select(.mapped and (.pid as $p | $pids | index($p)) and ($title == "" or .title == $title))]
            | max_by(.size[0] * .size[1]) // empty
            | "wl:\(.at[0]),\(.at[1]) \(.size[0])x\(.size[1])"' <<<"$json"
    elif command -v swaymsg >/dev/null 2>&1 && json="$(swaymsg -t get_tree 2>/dev/null)"; then
        jq -r --argjson pids "$wanted" --arg title "$title" '
            [.. | objects | select(.pid? and .visible and (.pid as $p | $pids | index($p)) and ($title == "" or .name == $title))]
            | max_by(.rect.width * .rect.height) // empty
            | "wl:\(.rect.x),\(.rect.y) \(.rect.width)x\(.rect.height)"' <<<"$json"
    fi
}

# Capture the window HANDLE to OUT.
shot_capture() {
    local handle="$1" out="$2"
    case "$(shot_os)" in
        macos) screencapture -x -o -l"$handle" "$out" ;;
        linux)
            case "$handle" in
                wl:*)
                    shot_need grim || return 1
                    grim -g "${handle#wl:}" "$out"
                    ;;
                *)
                    shot_need import || return 1
                    import -window "$handle" "$out"
                    ;;
            esac
            ;;
        windows)
            mkdir -p "$(dirname "$out")"
            shot_ps -Action capture -Handle "$handle" -Out "$(cygpath -wa "$out")" >/dev/null
            ;;
    esac
}

# Make the window HANDLE of PID, titled TITLE, 900 points wide and HEIGHT
# tall, as far as the screen allows.
shot_resize() {
    local pid="$1" handle="$2" title="$3" height="$4"
    case "$(shot_os)" in
        macos)
            osascript -e "tell application \"System Events\" to tell (first process whose unix id is $pid) to set size of window \"$title\" to {900, $height}" >/dev/null
            ;;
        linux)
            case "$handle" in
                wl:*) return 1 ;;
                *) xdotool windowsize --sync "$handle" 900 "$height" ;;
            esac
            ;;
        windows) shot_ps -Action resize -Handle "$handle" -Height "$height" >/dev/null ;;
    esac
}

# On Linux with no desktop (a container, CI over SSH), start a virtual X
# display for the window to open on. Needs Xvfb; the caller stops it at
# exit by the PID left in shot_xvfb.
shot_xvfb=""
shot_virtual_display() {
    [ "$(shot_os)" = linux ] || return 0
    [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ] || return 0
    shot_need Xvfb || return 1
    local fifo number
    fifo="$(mktemp -u)"
    mkfifo "$fifo"
    Xvfb -displayfd 3 -screen 0 1920x1200x24 -nolisten tcp 3>"$fifo" >/dev/null 2>&1 &
    # shellcheck disable=SC2034 # read by the script that sourced this one
    shot_xvfb=$!
    number=""
    read -r -t 10 number <"$fifo" || true
    rm -f "$fifo"
    if [ -z "$number" ]; then
        echo "Xvfb did not start a display" >&2
        return 1
    fi
    export DISPLAY=":$number"
}
