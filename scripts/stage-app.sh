#!/usr/bin/env bash
# Copies the built-in app folder to OUT as it ships inside the bundle
# (Moonframe.app/Contents/Resources/app): each wasm provider built to
# provider.wasm, without its Rust sources, so users need no toolchain and
# never run a build.
set -euo pipefail

out=${1:?usage: stage-app.sh OUT [APP]}
app=${2:-app}
builds=target/providers

rm -rf "$out"
mkdir -p "$out"
cp -R "$app"/. "$out"/
# The skills `app new` writes are for agents building the app, not for its users.
rm -rf "$out/.claude"

# The agent skills the `skills` provider offers to copy to the user's
# folder, beside it, as a bundle has no repository around it.
if [ -d "$out/providers/skills" ] && [ -d .claude/skills ]; then
    mkdir -p "$out/providers/skills/skills"
    for skill in .claude/skills/*; do
        [ -f "$skill/SKILL.md" ] && cp -R "$skill" "$out/providers/skills/skills/"
    done
fi

for manifest in "$app"/providers/*/Cargo.toml; do
    [ -e "$manifest" ] || continue
    dir=$(dirname "$manifest")
    name=$(basename "$dir")
    package=$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$manifest" | head -1)
    cargo build --quiet --release --locked --target wasm32-wasip2 --manifest-path "$manifest" --target-dir "$builds"
    staged="$out/providers/$name"
    cp "$builds/wasm32-wasip2/release/${package//-/_}.wasm" "$staged/provider.wasm"
    rm -rf "$staged/Cargo.toml" "$staged/Cargo.lock" "$staged/src" "$staged/target"
    echo "provider: $name -> $staged/provider.wasm"
done
