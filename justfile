default:
    @just --list

# The `ghostty` terminal feature builds Ghostty with Zig 0.15; this wrapper
# makes that work on macOS 27 (see tools/zig/zig).
export PATH := justfile_directory() / "tools/zig" + ":" + env_var("PATH")

# rustfmt.toml uses unstable options, so formatting runs on nightly while
# builds, clippy and tests use the stable rust-toolchain.toml.
nightly_toolchain := "nightly-2026-06-20"

# Open the window on the built-in app folder, with ~/.config/moonframe
# over it; it reloads as either changes.
run dir="app":
    cargo run --release -p moonframe -- {{dir}}

# Load every module headlessly, ~/.config/moonframe included, and report
# problems; run after each edit.
check dir="app":
    cargo run --quiet --release -p moonframe -- --check {{dir}}

# The same for the built-in app alone, whatever ~/.config holds.
check-builtin dir="app":
    cargo run --quiet --release -p moonframe -- --check --no-user {{dir}}

# Build Moonframe.app (release binary, staged app, icon) in target/bundle.
bundle out="target/bundle":
    scripts/bundle-app.sh {{out}}

# Build the app and install it in /Applications, to try mods that live in
# ~/.config/moonframe against the app as users get it.
install:
    scripts/install-app.sh

# The component catalogue agents read: write the moonframe-components skill from
# app/components and crates/moonframe-core/src/catalogue/elements.md.
components dir="app":
    mkdir -p .claude/skills/moonframe-components
    cargo run --quiet --release -p moonframe -- --no-user --catalogue {{dir}} > target/components.md
    mv target/components.md .claude/skills/moonframe-components/SKILL.md

# Fail when the moonframe-components skill is out of date with the components.
components-check dir="app":
    cargo run --quiet --release -p moonframe -- --no-user --catalogue {{dir}} > target/components.md
    diff -u .claude/skills/moonframe-components/SKILL.md target/components.md

# Stage the built-in app as it ships in the bundle (Contents/Resources/app):
# wasm providers prebuilt to provider.wasm, without their sources.
stage out="target/stage/app":
    scripts/stage-app.sh {{out}}

# Build and lint every wasm provider in app/providers, as the app would.
providers dir="app":
    #!/usr/bin/env bash
    set -euo pipefail
    for manifest in {{dir}}/providers/*/Cargo.toml; do
        [ -e "$manifest" ] || continue
        echo "provider: $(basename "$(dirname "$manifest")")"
        cargo clippy --quiet --release --target wasm32-wasip2 --manifest-path "$manifest" --target-dir target/providers -- -D warnings
        cargo build --quiet --release --target wasm32-wasip2 --manifest-path "$manifest" --target-dir target/providers
    done

# Download the pinned luau-lsp that `just types` uses (into target/tools).
setup-luau:
    scripts/setup-luau-lsp.sh

# Type-check the app's Luau against types/moonframe.d.luau in strict mode.
types dir="app":
    #!/usr/bin/env bash
    set -euo pipefail
    lsp=$(scripts/setup-luau-lsp.sh)
    files=$(find {{dir}} -name '*.luau' -not -name '*.d.luau' | sort)
    output=$("$lsp" analyze --platform=standard --definitions=@moonframe=types/moonframe.d.luau $files 2>&1 | grep -v '^\[INFO\]' || true)
    if [ -n "$output" ]; then
        echo "$output"
        exit 1
    fi
    echo "ok: types in {{dir}}"

# Capture a running window to a PNG (macOS); pass the PID of the one you started.
screenshot out="target/screenshot.png" pid="":
    scripts/screenshot.sh {{out}} {{pid}}

# Capture one component's preview page to a PNG from a background instance
# that never takes focus and is stopped afterwards (macOS).
component-shot name out="" dir="app":
    scripts/component-shot.sh {{name}} {{ if out == "" { "target/components/" + name + ".png" } else { out } }} {{dir}}

# Format Rust (nightly rustfmt) and TOML (taplo).
format:
    cargo +{{nightly_toolchain}} fmt --all
    taplo fmt

# Check Rust and TOML formatting without writing.
format-check:
    cargo +{{nightly_toolchain}} fmt --all -- --check
    taplo fmt --check

# Verify Cargo.lock is in sync with workspace manifests.
lockfile-check:
    cargo fetch --locked

lint: lockfile-check
    cargo clippy --locked --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --locked --workspace
    cargo test --locked --workspace --all-features

# Audit the GitHub workflows (zizmor, actionlint) and the dependency graph
# (cargo-deny: advisories, licenses, sources) as CI does.
audit:
    zizmor .github/
    actionlint
    cargo deny check advisories bans licenses sources

# Build this Mac's release packages, unsigned, as .github/workflows/package.yml
# does: the app and its disk image in target/dist.
package-macos out="target/dist":
    cargo build --locked --release -p moonframe
    scripts/release/package-macos.sh {{out}} target/release/moonframe

# Publish main's HEAD as v<version in Cargo.toml> (bump it first) through the
# Release workflow, which waits for CI on that commit; see docs/releasing.md.
release:
    #!/usr/bin/env bash
    set -euo pipefail
    git fetch --quiet origin main
    sha=$(git rev-parse origin/main)
    version=$(scripts/release/version.sh)
    test "$(git rev-parse HEAD)" = "$sha" || { echo "check out origin/main ($sha) first" >&2; exit 1; }
    test -z "$(git status --porcelain)" || { echo "the working tree is not clean" >&2; exit 1; }
    echo "Releasing v$version from $sha"
    gh workflow run release.yml --ref main -f expected_sha="$sha"

ci: format-check lint test types providers check-builtin components-check
