#!/usr/bin/env bash
# Loads every example app in examples/ (each folder with a moonframe.toml)
# headlessly with BINARY (default target/release/moonframe), each linked as
# a profile in a throwaway user folder, so nothing in ~/.config is touched;
# then, given HOST, the app built on the library (examples/host) too.
#
#   scripts/check-examples.sh [BINARY] [HOST]
set -euo pipefail

binary=${1:-target/release/moonframe}
host=${2:-}
user=$(mktemp -d)
trap 'rm -rf "$user"' EXIT

found=0
for manifest in examples/*/moonframe.toml; do
    app=$(basename "$(dirname "$manifest")")
    "$binary" --user "$user" app link "examples/$app" --yes >/dev/null
    "$binary" --user "$user" --profile "$app" --check | cut -c1-120
    found=$((found + 1))
done
[ "$found" -gt 0 ] || { echo "no example apps in examples/" >&2; exit 1; }

if [ -n "$host" ]; then
    "$host" --user "$user" --check | cut -c1-120
fi
