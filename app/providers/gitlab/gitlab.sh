#!/usr/bin/env bash
# Prints the GitLab state of the app's repository as one JSON line, shaped
# like the github provider's (merge requests under `prs` and `current_pr`):
# every minute, and at once when a mod sends any message
# (`to = "provider:gitlab"`). Exits when the app closes its stdin.
#
# It asks the `glab` CLI, with the account `glab auth login` signed in; it
# never sees a token. glab has no built-in jq, so this one needs `jq`.
# Avatars go to ${XDG_CACHE_HOME:-~/.cache}/moonframe/images/avatars as
# gitlab-<login>.png. Set LAYOUTS_REPO to watch another folder.

POLL=60
MAX_PRS=30
MAX_ISSUES=30
CACHE=${XDG_CACHE_HOME:-$HOME/.cache}/moonframe/images/avatars

bs='\'
dq='"'
jstr() {
    local s=$1
    s=${s//"$bs"/"$bs$bs"}
    s=${s//"$dq"/"$bs$dq"}
    if [[ $s == *[[:cntrl:]]* ]]; then
        s=$(printf '%s' "$s" | tr '\000-\037' ' ')
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

# A query-string value, percent-encoded byte by byte.
urlencode() {
    local LC_ALL=C s=$1 i c out=
    for ((i = 0; i < ${#s}; i++)); do
        c=${s:i:1}
        case $c in
            [A-Za-z0-9._~-]) out+=$c ;;
            *) printf -v c '%%%02X' "'$c" && out+=$c ;;
        esac
    done
    URL=$out
}

# Shared jq: GitLab's REST shapes into the github provider's.
JQ_LIB='
def secs: if . == null then null else (sub("\\.[0-9]+"; "") | sub("(?<z>[+-][0-9]{2}):?[0-9]{2}$"; "Z") | fromdateiso8601) end;
def person: if . == null then null else
    {login: .username, name: (.name // null), bot: (.bot // false), avatar_url: (.avatar_url // null),
     avatar_path: ($avatars[.username // ""] // null)} end;
def outcome: if . == "success" then "passed"
    elif . == "skipped" or . == "manual" then "skipped"
    elif . == "failed" or . == "canceled" then "failed"
    else "pending" end;
def summary($o): {total: ($o | length), passed: ([$o[] | select(. == "passed")] | length),
    failed: ([$o[] | select(. == "failed")] | length), pending: ([$o[] | select(. == "pending")] | length),
    skipped: ([$o[] | select(. == "skipped")] | length)};
def checks($jobs): if ($jobs | length) > 0 then summary([$jobs[] | .status | outcome])
    elif .head_pipeline then summary([.head_pipeline.status | outcome])
    else summary([]) end;
def tone($jobs): (.detailed_merge_status // "") as $m | checks($jobs) as $c
    | if .state == "merged" then "merged"
      elif .state == "closed" or .state == "locked" then "closed"
      elif .draft or .work_in_progress then "draft"
      elif .has_conflicts or $m == "conflict" or $m == "requested_changes"
        or $m == "discussions_not_resolved" or $c.failed > 0
        or ($m == "ci_must_pass" and $c.pending == 0 and .head_pipeline != null) then "attention"
      elif $m == "mergeable" then "ready"
      elif $m == "need_rebase" or $m == "blocked_status" or $m == "merge_request_blocked"
        or $m == "external_status_checks" or $m == "jira_association_missing" then "held"
      else "waiting" end;
def pr($jobs): {number: .iid, title, state: (if .state == "opened" then "open" else .state end),
    draft: (.draft // .work_in_progress // false), url: .web_url,
    branch: .source_branch, base: .target_branch, author: (.author | person),
    updated: (.updated_at | secs),
    review_decision: ((.detailed_merge_status // "") as $m
        | if $m == "not_approved" then "review_required"
          elif $m == "requested_changes" then "changes_requested" else "" end),
    mergeable: (if .has_conflicts then "conflicting" else "mergeable" end),
    merge_state: (.detailed_merge_status // ""),
    checks: checks($jobs), tone: tone($jobs)};
'

read_avatars() {
    local file login
    AVATARS='{'
    for file in "$CACHE"/gitlab-*.png; do
        [ -s "$file" ] || continue
        login=${file##*/gitlab-}
        login=${login%.png}
        [ "$AVATARS" = '{' ] || AVATARS+=,
        jstr "$login" && AVATARS+="$J:"
        jstr "$file" && AVATARS+=$J
    done
    AVATARS+='}'
}

# Reads "login<TAB>url" lines; downloads up to ten missing avatars.
fetch_avatars() {
    local login url n=0
    command -v curl >/dev/null 2>&1 || return
    mkdir -p "$CACHE" || return
    while IFS=$'\t' read -r login url; do
        case $login in '' | *[!A-Za-z0-9._-]*) continue ;; esac
        case $url in https://*) ;; *) continue ;; esac
        [ -s "$CACHE/gitlab-$login.png" ] && continue
        [ "$n" -ge 10 ] && break
        n=$((n + 1))
        if curl -fsSL -m 10 -o "$CACHE/gitlab-$login.png.part" "$url"; then
            mv "$CACHE/gitlab-$login.png.part" "$CACHE/gitlab-$login.png"
        else
            rm -f "$CACHE/gitlab-$login.png.part"
        fi
    done
}

# Whether any remote points at GitLab: a host with "gitlab" in it, or one
# glab knows.
gitlab_remote() {
    local urls
    urls=$(git remote -v 2>/dev/null)
    case $urls in *gitlab*) return 0 ;; esac
    [ -n "$GITLAB_HOST" ] && return 0
    glab repo view >/dev/null 2>&1
}

look() {
    local force=$1 repo prs current issues branch iid pipeline jobs
    if ! git rev-parse --git-dir >/dev/null 2>&1; then
        unavailable "not a git repository" "$force"
        return
    fi
    if ! command -v glab >/dev/null 2>&1; then
        case $(git remote -v 2>/dev/null) in
            *gitlab*) unavailable "glab is not installed (https://gitlab.com/gitlab-org/cli)" "$force" ;;
            *) unavailable "no GitLab remote" "$force" ;;
        esac
        return
    fi
    if ! gitlab_remote; then
        unavailable "no GitLab remote" "$force"
        return
    fi
    if ! command -v jq >/dev/null 2>&1; then
        unavailable "jq is not installed (needed to read glab's output)" "$force"
        return
    fi
    if ! repo=$(glab api 'projects/:id' 2>&1); then
        case $repo in
            *auth* | *401* | *token*) unavailable "glab is not signed in: run glab auth login" "$force" ;;
            *) unavailable "glab: ${repo%%$'\n'*}" "$force" ;;
        esac
        return
    fi
    repo=$(printf '%s' "$repo" | jq -c '{name_with_owner: .path_with_namespace, url: .web_url, default_branch}') || repo=null
    read_avatars
    if [ -z "$VIEWER" ] || [ "$VIEWER_AGE" -ge 60 ]; then
        VIEWER_RAW=$(glab api user 2>/dev/null) || VIEWER_RAW=
        VIEWER_AGE=0
    fi
    VIEWER_AGE=$((VIEWER_AGE + 1))
    VIEWER=$(printf '%s' "${VIEWER_RAW:-null}" | jq -c --argjson avatars "$AVATARS" "$JQ_LIB"' if . == null then null else person end' 2>/dev/null) || VIEWER=null
    if ! prs=$(glab api "projects/:id/merge_requests?state=opened&per_page=$MAX_PRS&order_by=updated_at" 2>&1); then
        printf 'glab merge_requests: %s\n' "$prs" >&2
        [ -n "$LAST" ] && [ "${LAST#\{\"available\":false}" = "$LAST" ] && return
        unavailable "glab: ${prs%%$'\n'*}" "$force"
        return
    fi
    RAW_PRS=$prs
    prs=$(printf '%s' "$prs" | jq -c --argjson avatars "$AVATARS" "$JQ_LIB"' [.[] | pr([])]') || prs='[]'
    current=null
    if branch=$(git symbolic-ref --short -q HEAD 2>/dev/null); then
        urlencode "$branch"
        iid=$(glab api "projects/:id/merge_requests?source_branch=$URL&state=all&order_by=updated_at&per_page=1" 2>/dev/null |
            jq -r '.[0].iid // empty' 2>/dev/null)
        if [ -n "$iid" ] && current=$(glab api "projects/:id/merge_requests/$iid" 2>/dev/null); then
            pipeline=$(printf '%s' "$current" | jq -r '.head_pipeline.id // empty' 2>/dev/null)
            jobs='[]'
            if [ -n "$pipeline" ]; then
                jobs=$(glab api "projects/:id/pipelines/$pipeline/jobs?per_page=50" 2>/dev/null) || jobs='[]'
            fi
            # Jobs are the checks: their count, and the tone they give.
            current=$(printf '%s' "$current" | jq -c --argjson avatars "$AVATARS" --argjson jobs "$jobs" "$JQ_LIB"'
                pr($jobs) + {check_list: [$jobs[] | {name, outcome: (.status | outcome), url: .web_url}]}' 2>/dev/null)
            [ -n "$current" ] || current=null
        fi
    fi
    issues=$(glab api "projects/:id/issues?state=opened&per_page=$MAX_ISSUES&order_by=updated_at" 2>/dev/null |
        jq -c --argjson avatars "$AVATARS" "$JQ_LIB"' [.[] | {number: .iid, title, url: .web_url, author: (.author | person),
            updated: (.updated_at | secs), labels: [.labels[] | {name: ., color: null}],
            assignees: [(.assignees // [])[].username]}]' 2>/dev/null) || issues='[]'
    [ -n "$issues" ] || issues='[]'
    OUT="{\"available\":true,\"repo\":$repo,\"viewer\":$VIEWER,\"current_pr\":$current,\"prs\":$prs,\"issues\":$issues}"
    if [ "$OUT" != "$LAST" ] || [ -n "$force" ]; then
        printf '%s\n' "$OUT"
        LAST=$OUT
    fi
    printf '%s\n%s' "${VIEWER_RAW:-null}" "$RAW_PRS" |
        jq -r '(if type == "array" then .[].author else . end) | select(. != null) | "\(.username)\t\(.avatar_url // "")"' 2>/dev/null |
        sort -u | fetch_avatars
}

if [ -n "$LAYOUTS_REPO" ]; then
    cd "$LAYOUTS_REPO" || exit 1
else
    cd "$(git -C ../.. rev-parse --show-toplevel 2>/dev/null || echo ../..)" || exit 1
fi

VIEWER=
VIEWER_RAW=
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
