#!/usr/bin/env bash
# Prints what this machine has for keeping terminals as one JSON line:
#   { "installed", "version", "path", "tmux", "installing", "error", "fresh" }
# every 30 seconds, and at once on any message. The message "install" runs
# herdr-core's own installer, downloaded from releases.herdr.dev and then
# run with sh (it installs into ~/.local/bin); `fresh` is true once it has.
# Only the `herdr` mod's "install" starts it, which it sends when the user
# clicks Install; any other message, or mod, only refreshes. Exits when the
# app closes its stdin. HERDR_RELEASES_URL points the installer elsewhere
# (tests).

POLL=30
RELEASES=${HERDR_RELEASES_URL:-https://releases.herdr.dev}

bs='\'
dq='"'
# $1 as a JSON string in J.
jstr() {
    local s=$1
    s=${s//"$bs"/"$bs$bs"}
    s=${s//"$dq"/"$bs$dq"}
    s=$(printf '%s' "$s" | tr '\n\t\r' '   ' | tr -d '\000-\037')
    J="\"$s\""
}

# PATH first, then where herdr-core and tmux get installed: an app opened
# from the Dock doesn't get the shell's PATH.
DIRS="$PATH:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$HOME/.local/share/mise/shims:$HOME/.nix-profile/bin:/run/current-system/sw/bin:$HOME/.cargo/bin:/usr/bin"

# The herdr-core here, in HC and HC_VERSION. A candidate counts only if its
# version says `hc …`: some window managers ship an unrelated `hc`.
find_core() {
    HC= HC_VERSION=
    local dir name version
    local IFS=:
    for dir in $DIRS; do
        for name in herdr-core hc; do
            [ -x "$dir/$name" ] || continue
            version=$("$dir/$name" --version 2>/dev/null | head -n 1)
            case $version in
            "hc "*)
                HC=$dir/$name
                HC_VERSION=${version#hc }
                return
                ;;
            esac
        done
    done
}

has_tmux() {
    local dir
    local IFS=:
    for dir in $DIRS; do
        [ -x "$dir/tmux" ] && return 0
    done
    return 1
}

INSTALLING=false
ERROR=
FRESH=false
LAST=
publish() {
    find_core
    local installed=false version=null path=null tmux=false error=null
    if [ -n "$HC" ]; then
        installed=true
        jstr "$HC_VERSION"
        version=$J
        jstr "$HC"
        path=$J
    fi
    has_tmux && tmux=true
    if [ -n "$ERROR" ]; then
        jstr "$ERROR"
        error=$J
    fi
    local out="{\"installed\":$installed,\"version\":$version,\"path\":$path,\"tmux\":$tmux,\"installing\":$INSTALLING,\"error\":$error,\"fresh\":$FRESH}"
    [ "$out" = "$LAST" ] && return
    printf '%s\n' "$out"
    LAST=$out
}

install() {
    INSTALLING=true
    ERROR=
    publish
    local output script
    # Downloaded first, then run, so a failed download is an error rather
    # than text handed to sh.
    script=$(mktemp "${TMPDIR:-/tmp}/herdr-core-install.XXXXXX") || {
        ERROR="cannot make a temporary file"
        INSTALLING=false
        return
    }
    if ! output=$(curl -fsSL "$RELEASES/core/install.sh" -o "$script" 2>&1); then
        ERROR=${output:-"cannot download the installer"}
    elif output=$(sh "$script" 2>&1); then
        find_core
        if [ -n "$HC" ]; then
            FRESH=true
        else
            ERROR="installed, but herdr-core does not run here"
        fi
    else
        ERROR=$(printf '%s\n' "$output" | grep -v '^$' | tail -n 1)
        [ -n "$ERROR" ] || ERROR="the installer failed"
    fi
    rm -f "$script"
    printf 'herdr-core installer: %s\n' "$output" >&2
    INSTALLING=false
}

while true; do
    publish
    # bash 3.2 returns 1 for a timeout and for the end of input alike, so a
    # `read` that failed early means the app closed stdin.
    started=$SECONDS
    if IFS= read -r -t "$POLL" line; then
        # Only the `herdr` mod may install, and only with exactly "install":
        # it sends that when the user clicks Install on its offer, and no
        # other mod's message runs anything here.
        case $line in
        '{"from":"herdr","message":"install"}') [ -z "$HC" ] && install ;;
        esac
    elif [ "$?" -le 128 ] && [ $((SECONDS - started)) -lt $((POLL - 1)) ]; then
        exit 0
    fi
done
