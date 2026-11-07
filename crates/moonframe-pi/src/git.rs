//! The little git a thread needs: worktrees, and a snapshot of the working
//! tree at each end of a run, so a run's changes can be shown on their own.

use {
    crate::{
        Config,
        config::{base_name, real_path},
        run::{Ran, run},
        runtime::Runtime,
    },
    serde_json::{Value, json},
    std::{
        fs,
        path::{Path, PathBuf},
        time::Duration,
    },
    uuid::Uuid,
};

const GIT_TIMEOUT: Duration = Duration::from_secs(60);

/// Runs git in `cwd`. It polls and runs beside an agent, so it must not take
/// the index lock to tidy up, or a commit at that moment fails.
pub(crate) fn run_git(
    runtime: &Runtime,
    cwd: &str,
    args: &[&str],
    env: &[(&str, &str)],
    input: Option<&str>,
) -> Ran {
    let mut command = runtime.command("git");
    command
        .args(args)
        .current_dir(cwd)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .envs(env.iter().copied());
    run(command, input, GIT_TIMEOUT)
}

/// git's output, or its complaint.
pub(crate) fn git(runtime: &Runtime, cwd: &str, args: &[&str]) -> Result<String, String> {
    run_git(runtime, cwd, args, &[], None).output()
}

pub(crate) fn repo_root(runtime: &Runtime, cwd: &str) -> Option<String> {
    let top = git(runtime, cwd, &["rev-parse", "--show-toplevel"]).ok()?;
    Some(top.trim().to_owned())
}

pub(crate) fn current_branch(runtime: &Runtime, cwd: &str) -> Option<String> {
    let branch = git(runtime, cwd, &["symbolic-ref", "--short", "-q", "HEAD"]).ok()?;
    Some(branch.trim().to_owned()).filter(|branch| !branch.is_empty())
}

/// The tree the working folder would commit as, ignored files aside,
/// through a throwaway index so the user's staging stays as it is.
pub(crate) fn snapshot(runtime: &Runtime, cwd: &str) -> Option<String> {
    repo_root(runtime, cwd)?;
    let index = std::env::temp_dir().join(format!("pilot-index-{}", Uuid::new_v4().simple()));
    // A repository with nothing staged yet has no index to start from.
    if let Ok(real) = git(runtime, cwd, &["rev-parse", "--git-path", "index"]) {
        let _ = fs::copy(Path::new(cwd).join(real.trim()), &index);
    }
    let path = index.to_string_lossy().into_owned();
    let env = [("GIT_INDEX_FILE", path.as_str())];
    let added = run_git(runtime, cwd, &["add", "-A"], &env, None);
    let tree = added
        .ok
        .then(|| {
            run_git(runtime, cwd, &["write-tree"], &env, None)
                .output()
                .ok()
        })
        .flatten()
        .map(|tree| tree.trim().to_owned());
    let _ = fs::remove_file(&index);
    let _ = fs::remove_file(format!("{path}.lock"));
    tree
}

/// What changed between two trees: `{ files, added, deleted }`.
pub(crate) fn changes_between(
    runtime: &Runtime,
    cwd: &str,
    from: &str,
    to: &str,
) -> Result<Value, String> {
    let args = ["diff", "--numstat", "--no-renames", "--relative", from, to];
    let out = git(runtime, cwd, &args)?;
    let mut files = Vec::new();
    let (mut added, mut deleted) = (0_i64, 0_i64);
    for line in out.lines() {
        let mut parts = line.split('\t');
        let (Some(plus), Some(minus), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (plus, minus) = (plus.parse().unwrap_or(0), minus.parse().unwrap_or(0));
        files.push(json!({ "path": path, "added": plus, "deleted": minus }));
        added += plus;
        deleted += minus;
    }
    Ok(json!({ "files": files, "added": added, "deleted": deleted }))
}

fn slugged(text: &str) -> String {
    let lower = text.to_lowercase();
    let words = lower.split(|c: char| !c.is_ascii_lowercase() && !c.is_ascii_digit());
    let joined = words
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let short = joined.get(..40).unwrap_or(&joined).trim_end_matches('-');
    if short.is_empty() {
        "thread".to_owned()
    } else {
        short.to_owned()
    }
}

/// A worktree on a new branch, off what the workspace has checked out:
/// `(path, branch)`.
pub(crate) fn add_worktree(
    config: &Config,
    runtime: &Runtime,
    workspace: &str,
    words: &str,
) -> Result<(String, String), String> {
    let root = repo_root(runtime, workspace)
        .ok_or("A worktree needs a git repository, and this folder is not one.")?;
    let tail = Uuid::new_v4().simple().to_string();
    let slug = format!("{}-{}", slugged(words), &tail[..4]);
    let branch = format!("pilot/{slug}");
    let folder: PathBuf = config.worktrees_dir().join(base_name(&root));
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let path = folder.join(&slug).to_string_lossy().into_owned();
    git(runtime, &root, &["worktree", "add", "-b", &branch, &path])?;
    // pi names a session folder after the real path, links resolved.
    Ok((real_path(&path)?, branch))
}

pub(crate) fn remove_worktree(
    runtime: &Runtime,
    workspace: &str,
    path: &str,
) -> Result<(), String> {
    git(runtime, workspace, &["worktree", "remove", "--force", path]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_keeps_letters_and_digits() {
        assert_eq!(slugged("Fix the Price!"), "fix-the-price");
        assert_eq!(slugged("!!!"), "thread");
        assert_eq!(slugged(&"ab ".repeat(30)).len(), 40);
        assert_eq!(
            slugged(&"abc ".repeat(30)).len(),
            39,
            "no dash left at the end"
        );
    }
}
