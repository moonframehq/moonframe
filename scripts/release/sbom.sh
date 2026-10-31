#!/usr/bin/env bash
# Writes the release's CycloneDX SBOMs to OUT_DIR: the moonframe executable's
# whole locked dependency graph across every target and feature, and one per
# Wasm provider that ships prebuilt inside the app.
set -euo pipefail
[[ $# == 1 ]] || { echo "usage: sbom.sh OUT_DIR" >&2; exit 2; }
out=$1
version=$(scripts/release/version.sh)
mkdir -p "$out"
name=moonframe-release-sbom
trap 'rm -f crates/*/"$name".json app/providers/*/"$name".json' EXIT

cargo cyclonedx -q --manifest-path crates/moonframe/Cargo.toml --format json \
    --all-features --target all --spec-version 1.5 --override-filename "$name"
mv "crates/moonframe/$name.json" "$out/Moonframe-$version.cdx.json"

for manifest in app/providers/*/Cargo.toml; do
    [ -e "$manifest" ] || continue
    dir=$(dirname "$manifest")
    cargo cyclonedx -q --manifest-path "$manifest" --format json \
        --target wasm32-wasip2 --spec-version 1.5 --override-filename "$name"
    mv "$dir/$name.json" "$out/Moonframe-$version-provider-$(basename "$dir").cdx.json"
done
# The lockfiles must not have moved: the SBOM describes what was built.
git diff --exit-code -- Cargo.lock app/providers/*/Cargo.lock
ls "$out"
