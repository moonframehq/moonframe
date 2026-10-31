#!/usr/bin/env bash
# Download the pinned luau-lsp, whose `analyze` type-checks the app against
# types/moonframe.d.luau, into target/tools. macOS only for now; the checksum
# is of the release's luau-lsp-macos.zip.
set -euo pipefail
version="1.70.1"
sha256="7d3936e8dec6dc77547abd061d2e950da392562a5f94858b880f23dba50d84cd"
dir="target/tools/luau-lsp-$version"
if [ -x "$dir/luau-lsp" ]; then
    echo "$dir/luau-lsp"
    exit 0
fi
test "$(uname -s)" = Darwin || { echo "only macOS is set up; see https://github.com/JohnnyMorganz/luau-lsp/releases" >&2; exit 1; }
download="target/tools/download"
mkdir -p "$download"
curl --fail --location --silent --show-error \
    --output "$download/luau-lsp-macos.zip" \
    "https://github.com/JohnnyMorganz/luau-lsp/releases/download/$version/luau-lsp-macos.zip"
echo "$sha256  $download/luau-lsp-macos.zip" | shasum -a 256 --check --status \
    || { echo "luau-lsp-macos.zip does not match its pinned checksum" >&2; exit 1; }
unzip -o -q "$download/luau-lsp-macos.zip" -d "$dir"
echo "$dir/luau-lsp"
