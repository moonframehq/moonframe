#!/usr/bin/env bash
# Prints where the app's agent skills stand in the user's folder as one JSON
# line:
#   { "available", "skills", "target", "status", "version", "installing",
#     "error", "fresh" }
# `status` is "missing" (none of them there), "stale" (some missing or
# different) or "current"; `version` is a checksum of the shipped skills, so
# a mod can tell one release's offer from the next. A checkout's skills change
# with nearly every commit, and an agent there reads the repository's own, so
# from a checkout only a missing skill is "stale" and `version` covers the
# names alone: an answer to the offer lasts past the next rebuild. Every 60
# seconds, and at
# once on any message. The message "install" from the `skills` mod copies
# them over, each skill folder replaced whole; other skills there are left
# alone. Exits when the app closes its stdin.
#
# The skills are `skills/` beside this script in a bundle (scripts/stage-app.sh
# puts them there), or the repository's `.claude/skills` when the app runs
# from a checkout. Plain bash (3.2 or later).

POLL=60
export LC_ALL=C

CHECKOUT=false
if [ -d skills ]; then
    SOURCE=$PWD/skills
elif [ -d ../../../.claude/skills ]; then
    SOURCE=$(cd ../../../.claude/skills && pwd)
    CHECKOUT=true
else
    SOURCE=
fi
CONFIG=${XDG_CONFIG_HOME:-$HOME/.config}/moonframe
TARGET=$CONFIG/.claude/skills

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

# The shipped skills' names, one per line.
names() {
    local dir
    for dir in "$SOURCE"/*/; do
        [ -f "$dir/SKILL.md" ] && basename "$dir"
    done
}

# A checksum of the named skills under $1: their paths and contents.
digest() {
    local root=$1 name file
    for name in $(names); do
        [ -d "$root/$name" ] || { printf 'missing %s\n' "$name"; continue; }
        (cd "$root" && find "$name" -type f | sort) | while IFS= read -r file; do
            printf '%s\n' "$file"
            cat "$root/$file"
        done
    done | cksum | tr ' ' '-'
}

INSTALLING=false
ERROR=
FRESH=false
LAST=
publish() {
    local available=false skills="[]" status=null version=null error=null name present=0 count=0
    if [ -n "$SOURCE" ] && [ -n "$(names)" ]; then
        available=true
        skills=
        for name in $(names); do
            jstr "$name"
            skills="${skills:+$skills,}$J"
            count=$((count + 1))
            [ -d "$TARGET/$name" ] && present=$((present + 1))
        done
        skills="[$skills]"
        local shipped
        if [ "$CHECKOUT" = true ]; then
            shipped=$(names | cksum | tr ' ' '-')
        else
            shipped=$(digest "$SOURCE")
        fi
        jstr "$shipped"
        version=$J
        if [ "$present" -eq 0 ]; then
            status='"missing"'
        elif [ "$CHECKOUT" = true ]; then
            [ "$present" -eq "$count" ] && status='"current"' || status='"stale"'
        elif [ "$(digest "$TARGET")" = "$shipped" ]; then
            status='"current"'
        else
            status='"stale"'
        fi
    fi
    if [ -n "$ERROR" ]; then
        jstr "$ERROR"
        error=$J
    fi
    # The tilde from a variable: bash 5 would expand a bare one back.
    local tilde="~"
    jstr "${TARGET/#"$HOME"/$tilde}"
    local out="{\"available\":$available,\"skills\":$skills,\"target\":$J,\"status\":$status,\"version\":$version,\"installing\":$INSTALLING,\"error\":$error,\"fresh\":$FRESH}"
    [ "$out" = "$LAST" ] && return
    printf '%s\n' "$out"
    LAST=$out
}

# Each skill is copied beside its old folder, then swapped in, so a failed
# copy leaves the old one whole.
install() {
    INSTALLING=true
    ERROR=
    publish
    local name output
    if ! output=$(mkdir -p "$TARGET" 2>&1); then
        ERROR=$output
    else
        for name in $(names); do
            rm -rf "$TARGET/.$name.new"
            if ! output=$(cp -R "$SOURCE/$name" "$TARGET/.$name.new" 2>&1 \
                && rm -rf "$TARGET/$name" \
                && mv "$TARGET/.$name.new" "$TARGET/$name" 2>&1); then
                ERROR=${output:-"cannot copy $name"}
                rm -rf "$TARGET/.$name.new"
                break
            fi
        done
    fi
    [ -z "$ERROR" ] && FRESH=true
    INSTALLING=false
}

while true; do
    publish
    # bash 3.2 returns 1 for a timeout and for the end of input alike, so a
    # `read` that failed early means the app closed stdin.
    started=$SECONDS
    if IFS= read -r -t "$POLL" line; then
        # Only the `skills` mod may install, and only with exactly "install":
        # it sends that when the user agrees on its offer.
        case $line in
        '{"from":"skills","message":"install"}') [ -n "$SOURCE" ] && install ;;
        esac
    elif [ "$?" -le 128 ] && [ $((SECONDS - started)) -lt $((POLL - 1)) ]; then
        exit 0
    fi
done
