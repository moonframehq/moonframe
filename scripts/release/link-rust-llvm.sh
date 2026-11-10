#!/usr/bin/env bash
# Lets the toolchain's rust-objcopy start on macOS. Since Rust 1.99 it links
# libLLVM.dylib but looks in rustlib/<host>/lib, not the toolchain's lib, so it
# aborts and a release build only warns and ships unstripped. Does nothing
# where rust-objcopy already runs, as it will once Rust fixes this.
set -euo pipefail
[[ $(uname -s) == Darwin ]] || exit 0
sysroot=$(rustc --print sysroot)
host=$(rustc -vV | sed -n 's/^host: //p')
rustlib="$sysroot/lib/rustlib/$host"
"$rustlib/bin/rust-objcopy" --version >/dev/null 2>&1 && exit 0
ln -sf ../../../libLLVM.dylib "$rustlib/lib/libLLVM.dylib"
"$rustlib/bin/rust-objcopy" --version >/dev/null 2>&1 || {
    echo "rust-objcopy still does not run in $rustlib" >&2
    exit 1
}
