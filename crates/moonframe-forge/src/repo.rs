//! The checkout the provider asks the forge about: its remote, the branch
//! it is on, and the branches its worktrees have out, read with git
//! through herdr-vcs (no shell, no prompts, a deadline).

use {
    herdr_vcs::git,
    std::{
        path::{Path, PathBuf},
        time::{Duration, Instant},
    },
};

/// Branches asked about at most, the current one first.
pub const BRANCHES: usize = 20;

fn text(root: &Path, args: &[&str]) -> Option<String> {
    let bytes = git::git_bytes(
        &root.to_string_lossy(),
        args,
        "read for the forge",
        Instant::now() + Duration::from_secs(10),
        &|| false,
        1 << 20,
    )
    .ok()?;
    Some(String::from_utf8_lossy(&bytes).trim_end().to_owned())
}

/// The repository the app's folder is in, as the git provider finds it:
/// `$LAYOUTS_REPO`, else two folders up from the provider's.
pub fn root(dir: &Path) -> Option<PathBuf> {
    let start = std::env::var_os("LAYOUTS_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.join("..").join(".."));
    let start = std::path::absolute(&start).unwrap_or(start);
    text(&start, &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

/// What the provider reads from git.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checkout {
    /// The remote's host and path (`owner/repo`, `group/sub/repo`).
    pub remote: Option<(String, String)>,
    pub branch: String,
    /// The current branch, then each worktree's, without repeats.
    pub branches: Vec<String>,
}

pub fn read(root: &Path) -> Checkout {
    let branch = text(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let remote = remote(root, &branch);
    let mut branches = Vec::new();
    if !branch.is_empty() && branch != "HEAD" {
        branches.push(branch.clone());
    }
    if let Some(list) = text(root, &["worktree", "list", "--porcelain"]) {
        for name in worktree_branches(&list) {
            if branches.len() >= BRANCHES {
                break;
            }
            if !branches.contains(&name) {
                branches.push(name);
            }
        }
    }
    Checkout {
        remote,
        branch,
        branches,
    }
}

/// The branches `git worktree list --porcelain` names.
pub fn worktree_branches(list: &str) -> Vec<String> {
    list.lines()
        .filter_map(|line| line.strip_prefix("branch refs/heads/"))
        .map(str::to_owned)
        .collect()
}

/// The remote the branch tracks, else `origin`, else the first.
fn remote(root: &Path, branch: &str) -> Option<(String, String)> {
    let tracked = (branch != "HEAD" && !branch.is_empty())
        .then(|| {
            text(root, &[
                "config",
                "--get",
                &format!("branch.{branch}.remote"),
            ])
        })
        .flatten()
        .filter(|name| !name.is_empty() && name != ".");
    let name = tracked
        .or_else(|| {
            text(root, &["config", "--get", "remote.origin.url"]).map(|_| "origin".to_owned())
        })
        .or_else(|| {
            text(root, &["remote"]).and_then(|names| names.lines().next().map(str::to_owned))
        })?;
    let url = text(root, &["remote", "get-url", &name])?;
    let remote = moonframe_git::remote_json(&name, &url);
    let host = remote["host"].as_str()?.to_ascii_lowercase();
    let owner = remote["owner"].as_str()?;
    let repo = remote["repo"].as_str()?;
    (!host.is_empty() && !owner.is_empty() && !repo.is_empty())
        .then(|| (host, format!("{owner}/{repo}")))
}
