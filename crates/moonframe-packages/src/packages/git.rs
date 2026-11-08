//! Running git on a package's clone.

use {
    super::printable,
    std::{path::Path, process::Command},
};

/// Runs git in `dir`, never asking for a password: a private repository
/// needs credentials git already has.
pub(super) fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // How a missing or private repository answers over HTTPS.
        let hint = if stderr.contains("terminal prompts disabled") {
            "\n(no such public repository: check the name, or use an SSH URL for a private one)"
        } else {
            ""
        };
        Err(format!("git {}: {}{hint}", args[0], stderr.trim()))
    }
}

/// Checks out what `reference` names, preferring the remote's branch of
/// that name, else the remote's default branch.
pub(super) fn checkout(dir: &Path, reference: Option<&str>) -> Result<(), String> {
    let target = resolve(dir, reference)?;
    git(dir, &["checkout", "--quiet", "--detach", &target])
        .map(drop)
        .map_err(|problem| format!("cannot check out {}: {problem}", short(&target)))
}

/// The commit `reference` names, as [`checkout`] picks it.
pub(super) fn resolve(dir: &Path, reference: Option<&str>) -> Result<String, String> {
    // From a hand-edited packages.toml, git would read it as an option.
    if let Some(reference) = reference.filter(|reference| reference.starts_with('-')) {
        return Err(format!("`{}` is not a ref", printable(reference)));
    }
    let target = match reference {
        None => "origin/HEAD".to_owned(),
        Some(reference) => {
            let branch = format!("origin/{reference}");
            if git(
                dir,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("{branch}^{{commit}}"),
                ],
            )
            .is_ok()
            {
                branch
            } else {
                reference.to_owned()
            }
        },
    };
    git(
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{target}^{{commit}}"),
        ],
    )
    .map_err(|_| format!("no branch, tag, or commit {target}"))
}

pub(super) fn head(dir: &Path) -> Result<String, String> {
    git(dir, &["rev-parse", "HEAD"])
}

pub(super) fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}
