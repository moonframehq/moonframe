#!/usr/bin/env bash
# Prints the project's files as one JSON line: when they change (looked at
# every few seconds) and at once when a mod sends a message
# (`to = "provider:files"`). Exits when the app closes its stdin.
#
# Output: { root = "<folder name>", path = "<absolute path>",
#           cwd = "<where a terminal starts: the launch folder, else path>",
#           dirs = { [""] = entries, ["app/mods"] = entries, ... } }
# where entries are { name, dir, ignored } with folders first, each group
# sorted without regard to case. Only the root and the open folders are
# listed, so a large folder costs nothing until it is opened.
#
# Messages: `{ open = "app\napp/mods" }` sets which folders are open (their
# paths from the root, one per line); anything else looks again.
#
# The root is the git repository `moonframe` was started in
# (MOONFRAME_LAUNCH_DIR), else the one the app runs from; set MOONFRAME_FILES
# to show another folder. Without either it publishes `available = false`:
# it never falls back to a folder as broad as home. Plain bash (3.2 or later), no
# jq or python: every string goes through `jstr`.

POLL=3             # seconds between looks
MAX_ENTRIES=500    # entries listed per folder
MAX_OPEN=100       # folders listed besides the root
HIDDEN=".git .DS_Store"   # never listed, as an editor hides them

# ---------------------------------------------------------------- JSON ----

bs='\'
dq='"'
CTRL_CHARS=()
CTRL_CODES=()
for ((code = 1; code < 32; code++)); do
    printf -v octal '%03o' "$code"
    printf -v char "\\$octal"
    CTRL_CHARS+=("$char")
    case $code in
        9) CTRL_CODES+=('\t') ;;
        10) CTRL_CODES+=('\n') ;;
        13) CTRL_CODES+=('\r') ;;
        *) printf -v escaped '\\u%04x' "$code" && CTRL_CODES+=("$escaped") ;;
    esac
done

# Sets J to $1 as a JSON string, quotes included. No subshell: it runs for
# every path and diff line.
jstr() {
    local s=$1 i
    case $s in
        *[\\\"]*)
            s=${s//"$bs"/"$bs$bs"}
            s=${s//"$dq"/"$bs$dq"}
            ;;
    esac
    if [[ $s == *[[:cntrl:]]* ]]; then
        for ((i = 0; i < 31; i++)); do
            s=${s//"${CTRL_CHARS[i]}"/"${CTRL_CODES[i]}"}
        done
    fi
    J="\"$s\""
}

# Sets J to $1 as a JSON number, or 0 when it is not one.
jnum() {
    case $1 in
        '' | *[!0-9]*) J=0 ;;
        *) J=$((10#$1)) ;;
    esac
}

jbool() {
    if [ -n "$1" ]; then J=true; else J=false; fi
}

# The string in a JSON message after "$1": (a key the host serialised), or
# nothing. Sets FOUND to `string`, `false`, `null` or empty, and VALUE.
json_field() {
    local line=$1 key=$2 rest out='' c i hex
    FOUND=
    VALUE=
    case $line in
        *"\"$key\":\""*) rest=${line#*"\"$key\":\""} ;;
        *"\"$key\":false"*) FOUND=false && return ;;
        *"\"$key\":null"*) FOUND=null && return ;;
        *) return ;;
    esac
    i=0
    while [ "$i" -lt "${#rest}" ]; do
        c=${rest:i:1}
        if [ "$c" = '"' ]; then
            FOUND=string
            VALUE=$out
            return
        fi
        if [ "$c" = '\' ]; then
            i=$((i + 1))
            c=${rest:i:1}
            case $c in
                n) c=$'\n' ;;
                t) c=$'\t' ;;
                r) c=$'\r' ;;
                b) c=$'\b' ;;
                f) c=$'\f' ;;
                u)
                    hex=${rest:i+1:4}
                    i=$((i + 4))
                    case $hex in
                        00[0-7][0-9a-fA-F]) printf -v c "\\x${hex:2:2}" ;;
                        *) printf -v c "\\u$hex" ;;   # bash 4.2+
                    esac
                    ;;
            esac
        fi
        out+=$c
        i=$((i + 1))
    done
}

# --------------------------------------------------------------- files ----

OPEN=()

# Whether folder $1 (a path from the root) is inside the root once symlinks
# are resolved: a link in the project to /etc is not listed.
inside_root() {
    local real
    real=$(cd "$ROOT${1:+/$1}" 2>/dev/null && pwd -P) || return 1
    [ "$real" = "$ROOT_REAL" ] || [[ $real == "$ROOT_REAL"/* ]]
}

# Sets ENTRIES_JSON to folder $1's entries (a path from the root, "" for the
# root itself), folders first.
list_dir() {
    local dir=$1 base name kind count=0 dirs=() files=() ignored sep=
    base=$ROOT${dir:+/$dir}
    [ -d "$base" ] && inside_root "$dir" || { ENTRIES_JSON=; return 1; }
    while IFS= read -r -d '' path; do
        name=${path##*/}
        case " $HIDDEN " in *" $name "*) continue ;; esac
        [[ $name == *$'\n'* ]] && continue
        if [ -d "$path" ]; then dirs+=("$name"); else files+=("$name"); fi
    done < <(find "$base" -mindepth 1 -maxdepth 1 -print0 2>/dev/null)
    IGNORED_SET=$'\n'
    if [ -n "$GIT" ] && [ $((${#dirs[@]} + ${#files[@]})) -gt 0 ]; then
        # One `git check-ignore` for the whole folder.
        while IFS= read -r -d '' ignored; do
            IGNORED_SET+="${ignored##*/}"$'\n'
        done < <(
            for name in "${dirs[@]}" "${files[@]}"; do
                printf '%s\0' "${dir:+$dir/}$name"
            done | git -C "$ROOT" check-ignore -z --stdin 2>/dev/null
        )
    fi
    ENTRIES_JSON=
    for kind in dir file; do
        local names=()
        if [ "$kind" = dir ]; then names=("${dirs[@]}"); else names=("${files[@]}"); fi
        [ ${#names[@]} -gt 0 ] || continue
        while IFS= read -r name; do
            count=$((count + 1))
            [ "$count" -le "$MAX_ENTRIES" ] || break
            jstr "$name"
            ENTRIES_JSON+="$sep{\"name\":$J,\"dir\":$([ "$kind" = dir ] && echo true || echo false)"
            case $IGNORED_SET in
                *$'\n'"$name"$'\n'*) ENTRIES_JSON+=',"ignored":true}' ;;
                *) ENTRIES_JSON+=',"ignored":false}' ;;
            esac
            sep=,
        done < <(printf '%s\n' "${names[@]}" | LC_ALL=C sort -f)
    done
}

emit() {
    local force=$1 dir out sep=, count=0
    list_dir "" || return
    jstr "${ROOT##*/}" && out="{\"available\":true,\"root\":$J"
    jstr "$ROOT" && out+=",\"path\":$J"
    jstr "$CWD" && out+=",\"cwd\":$J"
    out+=",\"dirs\":{\"\":[$ENTRIES_JSON]"
    for dir in "${OPEN[@]}"; do
        count=$((count + 1))
        [ "$count" -le "$MAX_OPEN" ] || break
        # Only relative paths without `..`, `.` or empty parts; list_dir
        # then checks the resolved folder is still inside the root.
        case "/$dir/" in */../* | */./* | *//*) continue ;; esac
        list_dir "$dir" || continue
        jstr "$dir"
        out+="$sep$J:[$ENTRIES_JSON]"
    done
    out+='}}'
    if [ "$out" != "$LAST" ] || [ -n "$force" ]; then
        printf '%s\n' "$out"
        LAST=$out
    fi
}

if [ -n "$MOONFRAME_FILES" ]; then
    ROOT=$MOONFRAME_FILES
else
    # The repository `moonframe` was started in, else the one holding the
    # app; never home (a dotfiles repository). The docs provider finds the
    # same root, so the editor opens the file the explorer names.
    repository() {
        local top
        top=$(git -C "$1" rev-parse --show-toplevel 2>/dev/null) || return 1
        [ -n "$top" ] && ! [ "$top" -ef "$HOME" ] && printf '%s' "$top"
    }
    ROOT=
    [ -n "$MOONFRAME_LAUNCH_DIR" ] && ROOT=$(repository "$MOONFRAME_LAUNCH_DIR")
    [ -n "$ROOT" ] || ROOT=$(repository ../..)
fi
ROOT=${ROOT%/}
ROOT_REAL=$(cd "$ROOT" 2>/dev/null && pwd -P)
# Where a terminal starts: the folder `moonframe` was started in, else the
# root.
CWD=${MOONFRAME_LAUNCH_DIR:-$ROOT}
if [ -z "$ROOT" ] || [ -z "$ROOT_REAL" ]; then
    printf '%s\n' '{"available":false,"reason":"not in a git repository; set MOONFRAME_FILES to the folder to show"}'
    # Stay until the app closes stdin, as a provider should.
    while IFS= read -r _; do :; done
    exit 0
fi
GIT=
git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1 && GIT=1

force=1
while true; do
    emit "$force"
    force=
    # A message, or POLL quiet seconds, looks again. The app closing stdin
    # ends the script. bash 3.2 returns 1 for both a timeout and the end of
    # input, so a `read` that failed before its timeout means the end.
    started=$SECONDS
    if IFS= read -r -t "$POLL" message; then
        force=1
        json_field "$message" open
        if [ "$FOUND" = string ]; then
            OPEN=()
            while IFS= read -r dir; do
                [ -n "$dir" ] && OPEN+=("$dir")
            done <<<"$VALUE"
        fi
    else
        status=$?
        if [ "$status" -le 128 ] && [ $((SECONDS - started)) -lt "$((POLL - 1))" ]; then
            exit 0
        fi
    fi
done
