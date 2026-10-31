#!/usr/bin/env bash
# Prints the git state of the app's repository as one JSON line: every five
# seconds when it changed, and at once when a mod sends a message
# (`to = "provider:git"`). Exits when the app closes its stdin.
#
# Messages: anything refreshes; `{ diff = "<path>" }` adds that file's diff
# against HEAD as `diff`, kept up to date until `{ diff = false }`.
#
# Plain bash (3.2, as macOS ships, or later) and git; no jq or python.
# Every string goes through `jstr`, which escapes it for JSON whatever it
# holds. Set LAYOUTS_REPO to watch another folder.

MAX_FILES=300      # entries in `files`
MAX_BRANCHES=20    # entries in `branches`
MAX_WORKTREES=30   # entries in `worktrees`
# The host takes at most 10,000 values and 1 MiB per line, so a diff stops
# at this many lines, each cut to MAX_TEXT characters.
MAX_DIFF_LINES=1200
MAX_TEXT=300
MAX_DIFF_BYTES=600000
POLL=5             # seconds between looks
SLOW_EVERY=6       # worktrees, branches, remote and stashes every 6 looks

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

# ----------------------------------------------------------------- git ----

work=$(mktemp -d "${TMPDIR:-/tmp}/moonframe-git.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT

ROOT=
EMPTY_TREE=

# The look that decides whether anything changed: status and numstat.
look() {
    git status --porcelain=v2 --branch -z --untracked-files=normal >"$work/status" 2>/dev/null || return 1
    if git rev-parse -q --verify HEAD >/dev/null 2>&1; then
        BASE=HEAD
    else
        [ -n "$EMPTY_TREE" ] || EMPTY_TREE=$(git hash-object -t tree /dev/null)
        BASE=$EMPTY_TREE
    fi
    git diff --numstat -z "$BASE" >"$work/numstat" 2>/dev/null || : >"$work/numstat"
    cat "$work/status" "$work/numstat" >"$work/look"
}

# Per-file numstat, keyed by path in one string: \x1c path \x1d "add del".
read_numstat() {
    local record added deleted path old
    NS_PATH=()
    NS_ADDED=()
    NS_DELETED=()
    NS_AT=0
    ADDED=0
    DELETED=0
    while IFS= read -r -d '' record; do
        added=${record%%$'\t'*}
        record=${record#*$'\t'}
        deleted=${record%%$'\t'*}
        path=${record#*$'\t'}
        if [ -z "$path" ]; then
            IFS= read -r -d '' old
            IFS= read -r -d '' path
        fi
        [ "$added" = - ] && added=0
        [ "$deleted" = - ] && deleted=0
        ADDED=$((ADDED + added))
        DELETED=$((DELETED + deleted))
        NS_PATH+=("$path")
        NS_ADDED+=("$added")
        NS_DELETED+=("$deleted")
    done <"$work/numstat"
}

# Sets A and D for $1 from the numstat. Both lists are in git's path order,
# so a cursor walks them together (bash 3.2 has no associative arrays, and a
# pattern search of one long string is quadratic).
numstat_of() {
    local k=$NS_AT end=$((NS_AT + 32))
    A=0
    D=0
    while [ "$k" -lt "${#NS_PATH[@]}" ] && [ "$k" -lt "$end" ]; do
        if [ "${NS_PATH[k]}" = "$1" ]; then
            A=${NS_ADDED[k]}
            D=${NS_DELETED[k]}
            NS_AT=$((k + 1))
            return
        fi
        k=$((k + 1))
    done
}

# Reads status into the branch fields and FILES_JSON.
read_status() {
    local record xy path old kind staged unstaged counts n=0 a d
    OID=
    BRANCH=
    UPSTREAM=
    AHEAD=0
    BEHIND=0
    CHANGED=0
    FILES_JSON=
    FILES_TRUNCATED=false
    while IFS= read -r -d '' record; do
        old=
        case $record in
            '# branch.oid '*) OID=${record#'# branch.oid '} && continue ;;
            '# branch.head '*) BRANCH=${record#'# branch.head '} && continue ;;
            '# branch.upstream '*) UPSTREAM=${record#'# branch.upstream '} && continue ;;
            '# branch.ab '*)
                counts=${record#'# branch.ab +'}
                AHEAD=${counts%% *}
                BEHIND=${counts#* -}
                continue
                ;;
            '#'*) continue ;;
            '1 '*) xy=${record:2:2} && path=${record#* * * * * * * * } ;;
            '2 '*)
                xy=${record:2:2}
                path=${record#* * * * * * * * * }
                IFS= read -r -d '' old
                ;;
            'u '*) xy=UU && path=${record#* * * * * * * * * * } ;;
            '? '*) xy='??' && path=${record#'? '} ;;
            *) continue ;;
        esac
        CHANGED=$((CHANGED + 1))
        if [ "$n" -ge "$MAX_FILES" ]; then
            FILES_TRUNCATED=true
            continue
        fi
        n=$((n + 1))
        staged=
        unstaged=
        case $xy in
            '??') kind='?' ;;
            UU | AA | DD | U? | ?U) kind=U ;;
            R* | ?R | C* | ?C) kind=R ;;
            ?A | A?) kind=A ;;
            ?D | D?) kind=D ;;
            *) kind=M ;;
        esac
        case $xy in '??') ;; .*) ;; *) staged=1 ;; esac
        case $xy in '??') unstaged=1 ;; ?.) ;; *) unstaged=1 ;; esac
        a=0
        d=0
        if [ "$kind" != '?' ]; then
            numstat_of "$path"
            a=$A
            d=$D
        fi
        [ -n "$FILES_JSON" ] && FILES_JSON+=,
        jstr "$path"
        FILES_JSON+="{\"path\":$J,\"status\":\"$kind\""
        jbool "$staged"
        FILES_JSON+=",\"staged\":$J"
        jbool "$unstaged"
        FILES_JSON+=",\"unstaged\":$J,\"added\":$a,\"deleted\":$d"
        if [ -n "$old" ]; then
            jstr "$old"
            FILES_JSON+=",\"old_path\":$J"
        fi
        FILES_JSON+='}'
    done <"$work/status"
    case $OID in '(initial)') OID= ;; esac
    DETACHED=false
    case $BRANCH in '(detached)') DETACHED=true && BRANCH=HEAD ;; esac
}

read_head() {
    local sha short author email time subject
    HEAD_JSON=null
    [ -n "$OID" ] || return
    {
        IFS= read -r -d '' sha
        IFS= read -r -d '' short
        IFS= read -r -d '' author
        IFS= read -r -d '' email
        IFS= read -r -d '' time
        IFS= read -r -d '' subject
    } < <(git log -1 --no-show-signature --format='%H%x00%h%x00%an%x00%ae%x00%ct%x00%s%x00' HEAD 2>/dev/null)
    [ -n "$sha" ] || return
    jstr "$sha" && HEAD_JSON="{\"sha\":$J"
    jstr "$short" && HEAD_JSON+=",\"short\":$J"
    jstr "$subject" && HEAD_JSON+=",\"subject\":$J"
    jstr "$author" && HEAD_JSON+=",\"author\":$J"
    jstr "$email" && HEAD_JSON+=",\"email\":$J"
    jnum "$time" && HEAD_JSON+=",\"time\":$J}"
}

# "ahead 2, behind 1", "ahead 2", "behind 1", "gone" or "".
track() {
    TRACK_AHEAD=0
    TRACK_BEHIND=0
    TRACK_GONE=false
    case $1 in
        gone) TRACK_GONE=true ;;
        *) ;;
    esac
    case $1 in
        *'ahead '*) TRACK_AHEAD=${1#*ahead } && TRACK_AHEAD=${TRACK_AHEAD%%,*} ;;
    esac
    case $1 in
        *'behind '*) TRACK_BEHIND=${1#*behind } ;;
    esac
}

# Recent local branches, newest commit first.
read_branches() {
    local name sha time upstream trackinfo head
    BRANCHES_JSON=
    while IFS=$'\x1f' read -r name sha time upstream trackinfo head; do
        [ -n "$name" ] || continue
        track "$trackinfo"
        [ -n "$BRANCHES_JSON" ] && BRANCHES_JSON+=,
        jstr "$name" && BRANCHES_JSON+="{\"name\":$J"
        jstr "$sha" && BRANCHES_JSON+=",\"head\":$J"
        jnum "$time" && BRANCHES_JSON+=",\"time\":$J"
        if [ -n "$upstream" ]; then
            jstr "$upstream" && BRANCHES_JSON+=",\"upstream\":$J"
        fi
        BRANCHES_JSON+=",\"ahead\":$TRACK_AHEAD,\"behind\":$TRACK_BEHIND,\"gone\":$TRACK_GONE"
        jbool "${head// /}" && BRANCHES_JSON+=",\"current\":$J}"
    done < <(git for-each-ref --sort=-committerdate --count="$MAX_BRANCHES" \
        --format='%(refname:short)%1f%(objectname:short)%1f%(committerdate:unix)%1f%(upstream:short)%1f%(upstream:track,nobracket)%1f%(HEAD)' \
        refs/heads/ 2>/dev/null)
}

# Worktrees into parallel arrays; JSON is built at emit time so the current
# one's `changed` is fresh.
read_worktrees() {
    local delim=$'\n' line path head branch detached locked prunable bare n=0
    WT_PATH=()
    WT_HEAD=()
    WT_BRANCH=()
    WT_FLAGS=()
    if git worktree list --porcelain -z >"$work/worktrees" 2>/dev/null; then
        delim=
    else
        git worktree list --porcelain >"$work/worktrees" 2>/dev/null || return
    fi
    path=
    finish() {
        [ -n "$path" ] || return
        if [ "$n" -lt "$MAX_WORKTREES" ]; then
            WT_PATH+=("$path")
            WT_HEAD+=("$head")
            WT_BRANCH+=("$branch")
            WT_FLAGS+=("$detached$locked$prunable$bare")
        fi
        n=$((n + 1))
        path=
    }
    while IFS= read -r -d "$delim" line || [ -n "$line" ]; do
        case $line in
            'worktree '*)
                finish
                path=${line#worktree }
                head=
                branch=
                detached=
                locked=
                prunable=
                bare=
                ;;
            'HEAD '*) head=${line#HEAD } ;;
            'branch '*) branch=${line#branch } && branch=${branch#refs/heads/} ;;
            detached) detached=d ;;
            locked*) locked=l ;;
            prunable*) prunable=p ;;
            bare) bare=b ;;
        esac
    done <"$work/worktrees"
    finish
    unset -f finish
    # Ahead, behind and dirty marks for each worktree.
    local i trackinfo
    WT_AHEAD=()
    WT_BEHIND=()
    WT_CHANGED=()
    for i in "${!WT_PATH[@]}"; do
        WT_AHEAD[i]=0
        WT_BEHIND[i]=0
        WT_CHANGED[i]=0
        case ${WT_FLAGS[i]} in *p* | *b*) continue ;; esac
        if [ -n "${WT_BRANCH[i]}" ]; then
            trackinfo=$(git for-each-ref --format='%(upstream:track,nobracket)' "refs/heads/${WT_BRANCH[i]}" 2>/dev/null)
            track "$trackinfo"
            WT_AHEAD[i]=$TRACK_AHEAD
            WT_BEHIND[i]=$TRACK_BEHIND
        fi
        WT_CHANGED[i]=$(git -C "${WT_PATH[i]}" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
    done
}

worktrees_json() {
    local i flags
    WORKTREES_JSON=
    for i in "${!WT_PATH[@]}"; do
        flags=${WT_FLAGS[i]}
        [ -n "$WORKTREES_JSON" ] && WORKTREES_JSON+=,
        jstr "${WT_PATH[i]}" && WORKTREES_JSON+="{\"path\":$J"
        jstr "${WT_BRANCH[i]}" && WORKTREES_JSON+=",\"branch\":$J"
        jstr "${WT_HEAD[i]}" && WORKTREES_JSON+=",\"head\":$J"
        if [ "${WT_PATH[i]}" = "$ROOT" ]; then
            WORKTREES_JSON+=",\"current\":true,\"changed\":$CHANGED,\"ahead\":$AHEAD,\"behind\":$BEHIND"
        else
            WORKTREES_JSON+=",\"current\":false,\"changed\":${WT_CHANGED[i]:-0},\"ahead\":${WT_AHEAD[i]:-0},\"behind\":${WT_BEHIND[i]:-0}"
        fi
        case $flags in *d*) J=true ;; *) J=false ;; esac
        WORKTREES_JSON+=",\"detached\":$J"
        case $flags in *l*) J=true ;; *) J=false ;; esac
        WORKTREES_JSON+=",\"locked\":$J"
        case $flags in *p*) J=true ;; *) J=false ;; esac
        WORKTREES_JSON+=",\"prunable\":$J"
        case $flags in *b*) J=true ;; *) J=false ;; esac
        WORKTREES_JSON+=",\"bare\":$J}"
    done
}

# The remote that the branch tracks, else origin, else the first.
read_remote() {
    local name url rest host path owner repo kind
    REMOTE_JSON=null
    name=
    [ "$BRANCH" != HEAD ] && name=$(git config --get "branch.$BRANCH.remote" 2>/dev/null)
    case $name in '' | .) name= ;; esac
    if [ -z "$name" ]; then
        if git config --get remote.origin.url >/dev/null 2>&1; then
            name=origin
        else
            name=$(git remote 2>/dev/null | head -n 1)
        fi
    fi
    [ -n "$name" ] || return
    url=$(git remote get-url "$name" 2>/dev/null) || return
    # scp-like user@host:owner/repo, or scheme://[user@]host[:port]/owner/repo
    case $url in
        *://*)
            rest=${url#*://}
            host=${rest%%/*}
            path=${rest#*/}
            [ "$path" = "$rest" ] && path=
            host=${host##*@}
            host=${host%%:*}
            ;;
        *:*)
            host=${url%%:*}
            host=${host##*@}
            path=${url#*:}
            ;;
        *) host= && path=$url ;;
    esac
    path=${path%/}
    path=${path%.git}
    path=${path#/}
    case $host in
        *github*) kind=github ;;
        *gitlab*) kind=gitlab ;;
        *) kind=other ;;
    esac
    repo=${path##*/}
    owner=
    case $path in */*) owner=${path%/*} ;; esac
    jstr "$name" && REMOTE_JSON="{\"name\":$J"
    jstr "$url" && REMOTE_JSON+=",\"url\":$J"
    jstr "$host" && REMOTE_JSON+=",\"host\":$J,\"kind\":\"$kind\""
    jstr "$owner" && REMOTE_JSON+=",\"owner\":$J"
    jstr "$repo" && REMOTE_JSON+=",\"repo\":$J"
    if [ -n "$host" ] && [ -n "$owner" ]; then
        jstr "https://$host/$path" && REMOTE_JSON+=",\"web_url\":$J"
    fi
    REMOTE_JSON+='}'
}

read_stashes() {
    STASHES=$(git rev-list --walk-reflogs --count refs/stash 2>/dev/null)
    jnum "$STASHES"
    STASHES=$J
}

# ---------------------------------------------------------------- diff ----

DIFF_PATH=
DIFF_JSON=

# One file's diff against HEAD (or, untracked, against nothing) as hunks.
read_diff() {
    local path=$DIFF_PATH line old=0 new=0 count=0 hunk_open='' lines='' hunks='' truncated=false binary=false rest kind text size=0
    : >"$work/diff"
    git -c core.quotePath=false diff --no-color --no-ext-diff --no-textconv "$BASE" -- ":(literal)$path" >"$work/diff" 2>/dev/null
    if [ ! -s "$work/diff" ] && [ -f "$path" ] &&
        ! git ls-files --error-unmatch -- ":(literal)$path" >/dev/null 2>&1; then
        git diff --no-index --no-color --no-ext-diff -- /dev/null "$path" >"$work/diff" 2>/dev/null
    fi
    while IFS= read -r line || [ -n "$line" ]; do
        case $line in
            '@@ '*)
                [ "$count" -ge "$MAX_DIFF_LINES" ] && truncated=true && break
                [ "$size" -ge "$MAX_DIFF_BYTES" ] && truncated=true && break
                if [ -n "$hunk_open" ]; then
                    hunks+="$lines]}"
                    hunks+=,
                fi
                rest=${line#'@@ -'}
                old=${rest%%[, ]*}
                rest=${rest#* +}
                new=${rest%%[, ]*}
                jstr "$line"
                hunks+="{\"header\":$J,\"old_start\":$((10#${old:-0})),\"new_start\":$((10#${new:-0})),\"lines\":["
                hunk_open=1
                lines=
                ;;
            'Binary files '* | 'GIT binary patch'*) binary=true ;;
            *)
                [ -n "$hunk_open" ] || continue
                case $line in
                    '+'*) kind=add ;;
                    '-'*) kind=del ;;
                    ' '* | '') kind=ctx ;;
                    *) continue ;;   # "\ No newline at end of file"
                esac
                if [ "$count" -ge "$MAX_DIFF_LINES" ] || [ "$size" -ge "$MAX_DIFF_BYTES" ]; then
                    truncated=true
                    break
                fi
                count=$((count + 1))
                text=${line:1}
                [ "${#text}" -gt "$MAX_TEXT" ] && text=${text:0:MAX_TEXT}
                jstr "$text"
                size=$((size + ${#J} + 40))
                [ -n "$lines" ] && lines+=,
                case $kind in
                    add) lines+="{\"kind\":\"add\",\"new\":$new,\"text\":$J}" && new=$((new + 1)) ;;
                    del) lines+="{\"kind\":\"del\",\"old\":$old,\"text\":$J}" && old=$((old + 1)) ;;
                    ctx) lines+="{\"kind\":\"ctx\",\"old\":$old,\"new\":$new,\"text\":$J}" && old=$((old + 1)) && new=$((new + 1)) ;;
                esac
                ;;
        esac
    done <"$work/diff"
    [ -n "$hunk_open" ] && hunks+="$lines]}"
    jstr "$path"
    DIFF_JSON="{\"path\":$J,\"binary\":$binary,\"lines\":$count,\"truncated\":$truncated,\"hunks\":[$hunks]}"
}

# ---------------------------------------------------------------- loop ----

LAST=
TICKS=0
FRESH=1

emit_unavailable() {
    FRESH=1
    jstr "$1"
    OUT="{\"available\":false,\"repository\":false,\"reason\":$J}"
    if [ "$OUT" != "$LAST" ] || [ -n "$2" ]; then
        printf '%s\n' "$OUT"
        LAST=$OUT
    fi
}

# $1: non-empty to recompute and print even when nothing changed.
emit() {
    local force=$1 changed='' old_oid=$OID old_branch=$BRANCH
    if ! command -v git >/dev/null 2>&1; then
        emit_unavailable "git is not installed" "$force"
        return
    fi
    if ! ROOT=$(git rev-parse --show-toplevel 2>/dev/null); then
        emit_unavailable "not a git repository" "$force"
        return
    fi
    # A busy index fails a look now and then; try again next time.
    look || return
    if [ -n "$force" ] || [ -n "$FRESH" ] || ! cmp -s "$work/look" "$work/last-look"; then
        changed=1
        mv "$work/look" "$work/last-look"
        read_numstat
        read_status
        [ "$OID" != "$old_oid" ] || [ -z "$HEAD_JSON" ] && read_head
    fi
    TICKS=$((TICKS + 1))
    if [ -n "$force" ] || [ "$TICKS" -ge "$SLOW_EVERY" ] || [ -n "$FRESH" ] ||
        [ "$OID" != "$old_oid" ] || [ "$BRANCH" != "$old_branch" ]; then
        TICKS=0
        changed=1
        read_worktrees
        read_branches
        read_remote
        read_stashes
    fi
    FRESH=
    [ -n "$changed" ] || return
    if [ -n "$DIFF_PATH" ]; then
        read_diff
    else
        DIFF_JSON=
    fi
    worktrees_json
    jstr "$BRANCH"
    OUT="{\"available\":true,\"repository\":true,\"branch\":$J,\"changed\":$CHANGED,\"ahead\":$((10#$AHEAD)),\"behind\":$((10#$BEHIND))"
    jstr "$ROOT" && OUT+=",\"root\":$J"
    OUT+=",\"detached\":$DETACHED,\"head\":$HEAD_JSON"
    if [ -n "$UPSTREAM" ]; then jstr "$UPSTREAM"; else J=null; fi
    OUT+=",\"upstream\":$J,\"added\":$ADDED,\"deleted\":$DELETED"
    OUT+=",\"files\":[$FILES_JSON],\"files_truncated\":$FILES_TRUNCATED"
    OUT+=",\"worktrees\":[$WORKTREES_JSON],\"branches\":[$BRANCHES_JSON]"
    OUT+=",\"remote\":$REMOTE_JSON,\"stashes\":$STASHES"
    [ -n "$DIFF_JSON" ] && OUT+=",\"diff\":$DIFF_JSON"
    OUT+='}'
    if [ "$OUT" != "$LAST" ] || [ -n "$force" ]; then
        printf '%s\n' "$OUT"
        LAST=$OUT
    fi
}

# Run where the app runs, not in this provider's folder.
if [ -n "$LAYOUTS_REPO" ]; then
    cd "$LAYOUTS_REPO" || exit 1
else
    cd "$(git -C ../.. rev-parse --show-toplevel 2>/dev/null || echo ../..)" || exit 1
fi

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
        json_field "$message" diff
        case $FOUND in
            string) DIFF_PATH=$VALUE ;;
            false | null) DIFF_PATH= ;;
        esac
    else
        status=$?
        if [ "$status" -le 128 ] && [ $((SECONDS - started)) -lt "$((POLL - 1))" ]; then
            exit 0
        fi
    fi
done
