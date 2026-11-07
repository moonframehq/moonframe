//! Adds and removes worktrees for a mod, so each branch in flight can have
//! a checkout of its own.
//!
//! A mod names a branch, never a folder: the host puts each worktree beside
//! the repository, in `<repository>.worktrees/`. Removing one never deletes
//! its branch, and git refuses one with uncommitted changes or a lock, with
//! no way for a mod to force it: the work there is the user's. Nor does it
//! take the checkout the user works in, even one they moved to just now.

use {
    crate::{
        Repo,
        refs::{Worktree, parse_worktrees},
    },
    herdr_vcs::git as vcs,
    moonframe_core::Data,
    std::{
        path::{Path, PathBuf},
        time::{Duration, Instant},
    },
};

/// How long a change may take: a large checkout writes many files, and the
/// repository's hooks run.
const TIMEOUT: Duration = Duration::from_secs(120);
/// Output kept from one command.
const LIMIT: usize = 1 << 20;

/// A worktree a mod asks to add or remove.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    // keep-sorted start
    /// A worktree on `branch`; made from `base`, or HEAD, if it is new.
    Add {
        branch: String,
        base: Option<String>,
    },
    Remove {
        path: String,
    },
    // keep-sorted end
}

impl Change {
    /// `{ branch, base? }`, what `worktree_add` takes.
    pub(crate) fn add(value: &Data) -> Result<Self, String> {
        let Data::Map(fields) = value else {
            return Err("`worktree_add` takes { branch = \"...\", base = \"...\" }".to_owned());
        };
        let text = |key: &str| match fields.get(key) {
            Some(Data::String(text)) => Some(text.clone()),
            _ => None,
        };
        let branch = text("branch").ok_or("`worktree_add` needs a `branch`")?;
        Ok(Self::Add {
            branch,
            base: text("base"),
        })
    }

    /// Runs the change in `root`. `in_use` names the checkout the user works
    /// in now, which may have moved since `root` was read.
    pub(crate) fn apply(
        &self,
        root: &Path,
        in_use: &dyn Fn() -> Option<PathBuf>,
    ) -> Result<String, String> {
        match self {
            Self::Add { branch, base } => add(root, branch, base.as_deref()),
            Self::Remove { path } => remove(root, path, in_use),
        }
    }
}

/// Runs a git command that changes something, keeping what it says on
/// failure, which is the user's next step.
fn git(root: &Path, args: &[&str], operation: &'static str) -> Result<(), String> {
    let checkout = root.to_string_lossy();
    let deadline = Instant::now() + TIMEOUT;
    vcs::git_bytes(&checkout, args, operation, deadline, &|| false, LIMIT)
        .map(drop)
        .map_err(|error| error.to_string())
}

fn listed(repo: &Repo) -> Result<Vec<Worktree>, String> {
    repo.text(&["worktree", "list", "--porcelain", "-z"], "list worktrees")
        .map(|text| parse_worktrees(&text))
        .ok_or_else(|| "cannot list the worktrees".to_owned())
}

/// A name git takes for a new branch. The check also refuses what would
/// read as an option, such as `--force`, and `@{-1}`, which it would
/// expand to another branch.
fn checked_branch(repo: &Repo, branch: &str) -> Result<(), String> {
    let name = repo.text(
        &["check-ref-format", "--branch", branch],
        "check a branch name",
    );
    if name.as_deref() == Some(branch) {
        Ok(())
    } else {
        Err(format!("`{branch}` is not a branch name"))
    }
}

/// What `base` names, as a commit id: nothing a mod sends reaches
/// `git worktree add` as it came.
fn commit_of(repo: &Repo, base: &str) -> Result<String, String> {
    let commit = format!("{base}^{{commit}}");
    repo.text(
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &commit,
        ],
        "find the base",
    )
    .ok_or_else(|| format!("no commit `{base}` to start from"))
}

/// Where a worktree of `branch` goes: `<repository>.worktrees/<branch>`
/// beside the main checkout, one folder deep. A branch's `/` is `%2F` and its
/// `%` is `%25`, so no two branches share a folder: `feat/login` is not
/// `feat-login`, and neither sits inside another worktree.
fn folder_for(main: &Path, branch: &str) -> Result<PathBuf, String> {
    let (Some(parent), Some(name)) = (main.parent(), main.file_name()) else {
        return Err(format!("no folder beside {}", main.display()));
    };
    let mut worktrees = name.to_owned();
    worktrees.push(".worktrees");
    let folder = branch.replace('%', "%25").replace('/', "%2F");
    Ok(parent.join(worktrees).join(folder))
}

fn add(root: &Path, branch: &str, base: Option<&str>) -> Result<String, String> {
    let repo = Repo::at(root);
    checked_branch(&repo, branch)?;
    let worktrees = listed(&repo)?;
    let main = worktrees.first().ok_or("no main checkout")?;
    let folder = folder_for(Path::new(&main.path), branch)?;
    if folder.exists() {
        return Err(format!("{} already exists", folder.display()));
    }
    let path = folder.to_string_lossy().into_owned();
    let reference = format!("refs/heads/{branch}");
    let exists = repo
        .text(
            &["rev-parse", "--verify", "--quiet", &reference],
            "find a branch",
        )
        .is_some();
    if exists {
        let args = ["worktree", "add", "--quiet", &path, branch];
        git(root, &args, "add a worktree")?;
    } else {
        // Only a new branch starts somewhere: one that exists needs no
        // commit here, and HEAD may have none yet.
        let start = commit_of(&repo, base.unwrap_or("HEAD"))?;
        let args = ["worktree", "add", "--quiet", "-b", branch, &path, &start];
        git(root, &args, "add a worktree")?;
    }
    // As git lists it, which is how `app.git.worktrees` and a later removal
    // name it: on Windows that differs from the path given here.
    let added = listed(&repo)?
        .into_iter()
        .find(|worktree| Path::new(&worktree.path) == folder);
    Ok(added.map_or(path, |worktree| worktree.path))
}

fn remove(root: &Path, path: &str, in_use: &dyn Fn() -> Option<PathBuf>) -> Result<String, String> {
    let worktrees = listed(&Repo::at(root))?;
    let asked = Path::new(path);
    let Some(at) = worktrees
        .iter()
        .position(|worktree| Path::new(&worktree.path) == asked)
    else {
        return Err(format!("{path} is not a worktree of this repository"));
    };
    if at == 0 {
        return Err("the main checkout cannot be removed".to_owned());
    }
    // Asked last, right before git: the user may have moved here since this
    // began, which `root`, read then, cannot show.
    if asked == root || in_use().as_deref() == Some(asked) {
        return Err("this is the checkout in use; switch to another first".to_owned());
    }
    git(root, &["worktree", "remove", path], "remove a worktree")?;
    let branch = &worktrees[at].branch;
    Ok(if branch.is_empty() {
        format!("removed {path}")
    } else {
        format!("removed {path}; branch {branch} is kept")
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "worktrees/tests.rs"]
mod tests;
