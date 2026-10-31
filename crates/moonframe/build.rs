//! Records which checkout the binary is built from, for the dev build's
//! branch bar: the branch (empty when detached or outside git), the short
//! commit, and the worktree folder when it is a linked worktree. Cargo
//! builds again when the checkout moves to another branch or commit.

use std::{path::Path, process::Command};

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let manifest = Path::new(&manifest);
    let branch = git(manifest, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
    let commit = git(manifest, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let dir = git(manifest, &["rev-parse", "--absolute-git-dir"]);
    let common = git(manifest, &[
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
    ]);
    // A linked worktree has a git folder of its own beside the common one.
    let worktree = match (
        &dir,
        &common,
        git(manifest, &["rev-parse", "--show-toplevel"]),
    ) {
        (Some(dir), Some(common), Some(top)) if dir != common => Path::new(&top)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        _ => String::new(),
    };
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
    println!("cargo:rustc-env=MOONFRAME_BUILD_BRANCH={branch}");
    println!("cargo:rustc-env=MOONFRAME_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=MOONFRAME_BUILD_WORKTREE={worktree}");
}

/// `git args` in the crate's folder, its output trimmed; `None` on failure.
fn git(manifest: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest)
        // A build run from inside a git hook must ask about this checkout.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .ok()?;
    output.status.success().then_some(())?;
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}
