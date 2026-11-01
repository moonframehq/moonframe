#!/usr/bin/env bash
# Prints homebrew/Casks/moonframe.rb with VERSION and the disk image's
# SHA256 filled in, for scripts/release/update-homebrew.sh.
set -euo pipefail
[[ $# == 2 ]] || { echo "usage: render-cask.sh VERSION SHA256 > moonframe.rb" >&2; exit 2; }
[[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "not a release version: '$1'" >&2; exit 1; }
[[ $2 =~ ^[0-9a-fA-F]{64}$ ]] || { echo "not a SHA256: '$2'" >&2; exit 1; }
sha=$(printf '%s' "$2" | tr 'A-F' 'a-f')
template="$(dirname -- "$0")/../../homebrew/Casks/moonframe.rb"
# The template's leading comment is for this repository, not the tap.
sed -e '/^#/d' -e "s/@VERSION@/$1/g" -e "s/@SHA256@/$sha/g" "$template"
