#!/usr/bin/env bash
# Commits RENDERED_CASK (render-cask.sh) as Casks/moonframe.rb in
# penso/homebrew-tap, through that repository's Moonframe deploy key in
# HOMEBREW_TAP_SSH_KEY. Refuses to go back to an older version, and to
# change a version already in the tap.
set +x
set -euo pipefail
fail() { echo "update-homebrew.sh: $*" >&2; exit 1; }
[[ $# == 2 ]] || { echo "usage: update-homebrew.sh VERSION RENDERED_CASK" >&2; exit 2; }
version=$1
cask=$2
[[ $version =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]] || fail "not a release version: '$version'"
[[ -f $cask && ! -L $cask && -s $cask ]] || fail "the rendered cask must be a non-empty regular file"
grep -Fxq "  version \"$version\"" "$cask" || fail "the rendered cask is not version $version"
[[ -n ${HOMEBREW_TAP_SSH_KEY:-} ]] || fail "HOMEBREW_TAP_SSH_KEY is required"

umask 077
tmp=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/moonframe-tap.XXXXXXXX")
trap 'rm -rf -- "$tmp"' EXIT
printf '%s\n' "$HOMEBREW_TAP_SSH_KEY" > "$tmp/key"
unset HOMEBREW_TAP_SSH_KEY
# Trust GitHub's SSH host keys as published over HTTPS, never ssh-keyscan.
curl --fail --silent --show-error --proto '=https' --tlsv1.2 \
    --connect-timeout 15 --max-time 60 https://api.github.com/meta |
    jq -er '.ssh_keys | select(type == "array" and length > 0) | .[] | "github.com " + .' \
        > "$tmp/known_hosts"
export GIT_SSH_COMMAND="ssh -F /dev/null -i '$tmp/key' -o IdentitiesOnly=yes -o IdentityAgent=none -o BatchMode=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile='$tmp/known_hosts' -o GlobalKnownHostsFile=/dev/null"
export GIT_TERMINAL_PROMPT=0
git -c core.hooksPath=/dev/null clone --quiet --depth=1 --single-branch \
    git@github.com:penso/homebrew-tap.git "$tmp/tap"
branch=$(git -C "$tmp/tap" symbolic-ref --short HEAD)
git check-ref-format "refs/heads/$branch"
target=$tmp/tap/Casks/moonframe.rb
[[ ! -L $tmp/tap/Casks && ! -L $target ]] || fail "refusing a symlinked cask path"
if [[ -e $target ]]; then
    [[ -f $target ]] || fail "Casks/moonframe.rb is not a regular file"
    # Read only a literal version line; never evaluate the tap's Ruby.
    line=$(grep -E '^[[:blank:]]*version[[:blank:]]' "$target") || fail "the tap's cask has no version"
    [[ $line =~ ^[[:blank:]]*version[[:blank:]]+\"([0-9]+)\.([0-9]+)\.([0-9]+)\"[[:blank:]]*$ ]] \
        || fail "the tap's cask has a malformed version: $line"
    current="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}"
    if [[ $current == "$version" ]]; then
        cmp -s -- "$cask" "$target" || fail "the tap already has $version with other content; release a new version"
        echo "the Homebrew cask is already at $version"
        exit 0
    fi
    newest=$(printf '%s\n%s\n' "$current" "$version" | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)
    [[ $newest == "$version" ]] || fail "refusing to go back from $current to $version"
fi
mkdir -p "$tmp/tap/Casks"
cp -- "$cask" "$target"
git -C "$tmp/tap" add -- Casks/moonframe.rb
export GIT_AUTHOR_NAME='Moonframe release'
export GIT_AUTHOR_EMAIL='41898282+github-actions[bot]@users.noreply.github.com'
export GIT_COMMITTER_NAME=$GIT_AUTHOR_NAME GIT_COMMITTER_EMAIL=$GIT_AUTHOR_EMAIL
git -C "$tmp/tap" -c core.hooksPath=/dev/null -c commit.gpgSign=false \
    commit --quiet -m "moonframe $version"
git -C "$tmp/tap" -c core.hooksPath=/dev/null push --quiet origin "HEAD:refs/heads/$branch"
echo "Casks/moonframe.rb is now $version"
