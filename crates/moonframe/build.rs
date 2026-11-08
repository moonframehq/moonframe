//! Records which checkout the binary is built from, for the dev build's
//! branch bar: the branch (empty when detached or outside git), the short
//! commit, and the worktree folder when it is a linked worktree. Cargo
//! builds again when the checkout moves to another branch or commit.

#[path = "src/checkout.rs"]
mod checkout;

use {
    checkout::{Checkout, git},
    std::path::Path,
};

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let manifest = Path::new(&manifest);
    let checkout = Checkout::of(manifest);
    let dir = git(manifest, &["rev-parse", "--absolute-git-dir"]);
    let common = git(
        manifest,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    // Switching branches moves HEAD; committing moves the branch's ref,
    // which is a file under refs/heads (Cargo watches a folder whole) or a
    // line in packed-refs.
    let watched = [
        dir.as_deref().map(|dir| format!("{dir}/HEAD")),
        common
            .as_deref()
            .map(|common| format!("{common}/refs/heads")),
        common
            .as_deref()
            .map(|common| format!("{common}/packed-refs")),
    ];
    for path in watched
        .into_iter()
        .flatten()
        .filter(|path| Path::new(path).exists())
    {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/checkout.rs");
    println!("cargo:rustc-env=MOONFRAME_BUILD_BRANCH={}", checkout.branch);
    println!("cargo:rustc-env=MOONFRAME_BUILD_COMMIT={}", checkout.commit);
    println!(
        "cargo:rustc-env=MOONFRAME_BUILD_WORKTREE={}",
        checkout.worktree
    );
}
