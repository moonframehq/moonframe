//! Which git checkout a folder is in, for the dev build's branch bar: read at
//! build time for Moonframe's own (`build.rs` compiles this file too), and at
//! launch for the project of an app on the library.

use std::{path::Path, process::Command};

/// A checkout's branch (empty when detached or outside git), short commit,
/// and folder name when it is a linked worktree.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Checkout {
    pub(crate) branch: String,
    pub(crate) commit: String,
    pub(crate) worktree: String,
}

impl Checkout {
    /// The checkout `dir` is in; all empty outside git.
    pub(crate) fn of(dir: &Path) -> Self {
        let own = git(dir, &["rev-parse", "--absolute-git-dir"]);
        let common = git(dir, &[
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ]);
        // A linked worktree has a git folder of its own beside the common one.
        let worktree = match (own, common, git(dir, &["rev-parse", "--show-toplevel"])) {
            (Some(own), Some(common), Some(top)) if own != common => Path::new(&top)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            _ => String::new(),
        };
        Self {
            branch: git(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default(),
            commit: git(dir, &["rev-parse", "--short", "HEAD"]).unwrap_or_default(),
            worktree,
        }
    }
}

/// `git args` in `dir`, its output trimmed; `None` on failure.
pub(crate) fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
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
