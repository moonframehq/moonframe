#!/usr/bin/env bash
# Prints the GitHub state of the app's repository as one JSON line: every
# minute (forge APIs rate-limit), and at once when a mod sends any message
# (`to = "provider:github"`). Exits when the app closes its stdin.
#
# It asks the `gh` CLI, with the account `gh auth login` signed in; it never
# sees a token. gh's built-in jq shapes the JSON, so no jq or python is
# needed. Avatars are downloaded with curl into
# ${XDG_CACHE_HOME:-~/.cache}/moonframe/images/avatars, and `avatar_path` is set
# once a file is there. Set LAYOUTS_REPO to watch another folder.

POLL=60
MAX_PRS=30
MAX_ISSUES=30
CACHE=${XDG_CACHE_HOME:-$HOME/.cache}/moonframe/images/avatars

bs='\'
dq='"'
# $1 as a JSON string in J (paths and reasons only; gh escapes the rest).
jstr() {
    local s=$1
    s=${s//"$bs"/"$bs$bs"}
    s=${s//"$dq"/"$bs$dq"}
    if [[ $s == *[[:cntrl:]]* ]]; then
        s=${s//$'\n'/ }
        s=${s//$'\t'/ }
        s=${s//$'\r'/ }
        s=$(printf '%s' "$s" | tr -d '\000-\037')
    fi
    J="\"$s\""
}

LAST=
unavailable() {
    jstr "$1"
    OUT="{\"available\":false,\"reason\":$J}"
    [ "$OUT" = "$LAST" ] && [ -z "$2" ] && return
    printf '%s\n' "$OUT"
    LAST=$OUT
}

# jq shared by every query. `avatars` maps the logins with a cached avatar
# to its file; it is spliced in as a JSON object literal.
jq_lib() {
    cat <<EOF
def avatars: $AVATARS;
def secs: if . == null or . == "" then null else (sub("\\\\.[0-9]+";"") | fromdateiso8601) end;
def person: if . == null then null else
    {login, name: (.name // null), bot: ((.is_bot // false) or ((.login // "") | startswith("app/"))),
     avatar_url: (if (.login // "") | startswith("app/") then null else "https://$HOST/\(.login).png?size=64" end),
     avatar_path: avatars[.login // ""]} end;
def outcome: (.conclusion // .state // null) as \$c
    | if (.status // "COMPLETED") != "COMPLETED" and .conclusion == null and .state == null then "pending"
      elif \$c == "SUCCESS" then "passed"
      elif \$c == "NEUTRAL" or \$c == "SKIPPED" then "skipped"
      elif \$c == null or \$c == "PENDING" or \$c == "EXPECTED" or \$c == "QUEUED" or \$c == "IN_PROGRESS" then "pending"
      else "failed" end;
def checks: [(.statusCheckRollup // [])[] | outcome] as \$o
    | {total: (\$o | length), passed: ([\$o[] | select(. == "passed")] | length),
       failed: ([\$o[] | select(. == "failed")] | length),
       pending: ([\$o[] | select(. == "pending")] | length),
       skipped: ([\$o[] | select(. == "skipped")] | length)};
def tone: checks as \$c
    | if .state == "MERGED" then "merged"
      elif .state == "CLOSED" then "closed"
      elif .isDraft then "draft"
      elif .mergeStateStatus == "DIRTY" or .mergeable == "CONFLICTING"
        or (.mergeStateStatus == "UNSTABLE" and \$c.pending == 0)
        or .reviewDecision == "CHANGES_REQUESTED" or \$c.failed > 0 then "attention"
      elif \$c.pending > 0 or .reviewDecision == "REVIEW_REQUIRED" then "waiting"
      elif .mergeStateStatus == "CLEAN" then "ready"
      elif .mergeStateStatus == "BLOCKED" or .mergeStateStatus == "BEHIND" then "held"
      else "waiting" end;
def pr: {number, title, state: (.state | ascii_downcase), draft: .isDraft, url,
    branch: .headRefName, base: .baseRefName, author: (.author | person),
    updated: (.updatedAt | secs), review_decision: (.reviewDecision // "" | ascii_downcase),
    mergeable: (.mergeable // "" | ascii_downcase), merge_state: (.mergeStateStatus // "" | ascii_downcase),
    additions, deletions, checks: checks, tone: tone};
EOF
}

PR_FIELDS=number,title,state,isDraft,url,headRefName,baseRefName,author,updatedAt,reviewDecision,mergeable,mergeStateStatus,statusCheckRollup,additions,deletions

# Logins with a cached avatar, as a JSON object of login -> path.
read_avatars() {
    local file login
    AVATARS='{'
    for file in "$CACHE"/*.png; do
        [ -s "$file" ] || continue
        login=${file##*/}
        login=${login%.png}
        jstr "$file"
        [ "$AVATARS" = '{' ] || AVATARS+=,
        AVATARS+="\"$login\":$J"
    done
    AVATARS+='}'
}

# Downloads up to ten missing avatars; the next look shows them.
fetch_avatars() {
    local login n=0
    command -v curl >/dev/null 2>&1 || return
    mkdir -p "$CACHE" || return
    for login in "$@"; do
        case $login in '' | *[!A-Za-z0-9-]*) continue ;; esac
        [ -s "$CACHE/$login.png" ] && continue
        [ "$n" -ge 10 ] && break
        n=$((n + 1))
        if curl -fsSL -m 10 -o "$CACHE/$login.png.part" "https://$HOST/$login.png?size=64"; then
            mv "$CACHE/$login.png.part" "$CACHE/$login.png"
        else
            rm -f "$CACHE/$login.png.part"
        fi
    done
}

look() {
    local force=$1 repo current prs issues logins
    if ! command -v gh >/dev/null 2>&1; then
        unavailable "gh is not installed (https://cli.github.com)" "$force"
        return
    fi
    if ! git rev-parse --git-dir >/dev/null 2>&1; then
        unavailable "not a git repository" "$force"
        return
    fi
    # Which host and repo: gh reads the remotes the way `gh pr` does.
    if ! repo=$(gh repo view --json nameWithOwner,url,defaultBranchRef \
        --jq '{name_with_owner: .nameWithOwner, url, default_branch: .defaultBranchRef.name} | tojson' 2>&1); then
        case $repo in
            *'auth login'* | *'not logged'* | *authenticat*) unavailable "gh is not signed in: run gh auth login" "$force" ;;
            *'no git remotes'* | *'none of the git remotes'* | *'not a git'*) unavailable "no GitHub remote" "$force" ;;
            *) unavailable "gh: ${repo%%$'\n'*}" "$force" ;;
        esac
        return
    fi
    HOST=${repo#*\"url\":\"https://}
    HOST=${HOST%%/*}
    read_avatars
    local lib
    lib=$(jq_lib)
    case $VIEWER in *'"avatar_path":null'*) [ -s "$CACHE/$VIEWER_LOGIN.png" ] && VIEWER= ;; esac
    if [ -z "$VIEWER" ] || [ "$VIEWER_AGE" -ge 60 ]; then
        VIEWER=$(gh api user --jq "$lib {login, name, avatar_url, avatar_path: avatars[.login]} | tojson" 2>/dev/null) || VIEWER=null
        VIEWER_AGE=0
        VIEWER_LOGIN=${VIEWER#*\"login\":\"}
        VIEWER_LOGIN=${VIEWER_LOGIN%%\"*}
    fi
    VIEWER_AGE=$((VIEWER_AGE + 1))
    if ! prs=$(gh pr list --state open --limit "$MAX_PRS" --json "$PR_FIELDS" --jq "$lib [.[] | pr] | tojson" 2>&1); then
        # Keep what was shown through a network blip.
        printf 'gh pr list: %s\n' "$prs" >&2
        [ -n "$LAST" ] && [ "${LAST#\{\"available\":false}" = "$LAST" ] && return
        unavailable "gh: ${prs%%$'\n'*}" "$force"
        return
    fi
    current=$(gh pr view --json "$PR_FIELDS" \
        --jq "$lib pr + {check_list: [(.statusCheckRollup // [])[:50][] | {name: (.name // .context // \"\"), outcome: outcome, url: (.detailsUrl // .targetUrl // null)}]} | tojson" 2>/dev/null) ||
        current=null
    issues=$(gh issue list --state open --limit "$MAX_ISSUES" \
        --json number,title,author,labels,updatedAt,url,assignees \
        --jq "$lib [.[] | {number, title, url, author: (.author | person), updated: (.updatedAt | secs), labels: [.labels[] | {name, color}], assignees: [.assignees[].login]}] | tojson" 2>/dev/null) ||
        issues='[]'
    OUT="{\"available\":true,\"repo\":$repo,\"viewer\":$VIEWER,\"current_pr\":$current,\"prs\":$prs,\"issues\":$issues}"
    if [ "$OUT" != "$LAST" ] || [ -n "$force" ]; then
        printf '%s\n' "$OUT"
        LAST=$OUT
    fi
    # Authors without a cached avatar, for next time.
    logins=$(printf '%s' "$VIEWER$prs$current" | grep -o '"login":"[A-Za-z0-9-]*"' | sed 's/.*:"//; s/"$//' | sort -u)
    # shellcheck disable=SC2086
    fetch_avatars $logins
}

if [ -n "$LAYOUTS_REPO" ]; then
    cd "$LAYOUTS_REPO" || exit 1
else
    cd "$(git -C ../.. rev-parse --show-toplevel 2>/dev/null || echo ../..)" || exit 1
fi

VIEWER=
VIEWER_AGE=0
force=1
while true; do
    look "$force"
    force=
    # bash 3.2 returns 1 for a timeout and for the end of input alike, so a
    # `read` that failed early means the app closed stdin.
    started=$SECONDS
    if IFS= read -r -t "$POLL" _; then
        force=1
    elif [ "$?" -le 128 ] && [ $((SECONDS - started)) -lt $((POLL - 1)) ]; then
        exit 0
    fi
done
