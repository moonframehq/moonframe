#!/usr/bin/env bash
# Checks the app on its Rune renderer as a build of its own: `cargo test
# --workspace --all-features` also runs Rune's tests, but beside GPUI, whose
# features and dependencies can hide what a Rune-only build is missing.
# `just rune-check` and CI both run this.
#
#   scripts/check-rune.sh
set -euo pipefail

app=(-p moonframe-app --no-default-features --features rune)

cargo clippy --locked "${app[@]}" --all-targets -- -D warnings

# Linking no GPUI is what this build is for.
gpui=$(cargo tree --locked "${app[@]}" -e normal --prefix none | awk '{print $1}' | grep -c '^gpui' || true)
[ "$gpui" = 0 ] || { echo "the Rune build links GPUI" >&2; exit 1; }

cargo test --locked -p moonframe-rune -p moonframe-app \
    --no-default-features --features moonframe-app/rune

cargo run --locked "${app[@]}" -- --check --no-user

cargo build --locked "${app[@]}"
case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) exe=.exe ;;
    *) exe= ;;
esac
scripts/check-examples.sh "${CARGO_TARGET_DIR:-target}/debug/moonframe$exe"
