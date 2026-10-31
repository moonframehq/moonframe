#!/usr/bin/env bash
# Prints the git state of the app's repository as one JSON line every five
# seconds, and at once when a mod sends any message
# (`to = "provider:git"`). Exits when the app closes its stdin.

# Quotes and backslashes in a branch name, escaped for JSON.
escape() {
    printf '%s' "$1" | sed 's/[\\"]/\\&/g'
}

emit() {
    if ! branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null); then
        printf '{"repository":false}\n'
        return
    fi
    changed=$(git status --porcelain 2>/dev/null | wc -l | tr -d ' ')
    ahead=0
    behind=0
    if counts=$(git rev-list --left-right --count '@{upstream}...HEAD' 2>/dev/null); then
        read -r behind ahead <<<"$counts"
    fi
    printf '{"repository":true,"branch":"%s","changed":%d,"ahead":%d,"behind":%d}\n' \
        "$(escape "$branch")" "$changed" "$ahead" "$behind"
}

# Run where the app runs, not in this provider's folder.
cd "$(git -C ../.. rev-parse --show-toplevel 2>/dev/null || echo ../..)" || exit 1
while true; do
    emit
    # A message, or five quiet seconds, prints again. `read` fails above
    # 128 on a timeout, and with 1 when the app closes stdin, which ends.
    read -r -t 5 _
    status=$?
    if [ "$status" -ne 0 ] && [ "$status" -le 128 ]; then
        exit 0
    fi
done
