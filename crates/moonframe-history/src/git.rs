//! Git on the history's own repository, with the folder as its work tree.
//!
//! The user's configuration stays out of it: no system or global config,
//! no inherited `GIT_*` variables, no prompts, and a fixed author, so a
//! snapshot is the same whatever the machine has set.

use {
    crate::{Error, Result},
    std::{
        path::{Path, PathBuf},
        process::{Command, Stdio},
    },
};

/// The history's repository and the folder it keeps.
#[derive(Debug)]
pub(crate) struct Git {
    pub(crate) dir: PathBuf,
    pub(crate) work: PathBuf,
}

impl Git {
    /// Runs git and returns what it printed, without the trailing newline.
    pub(crate) fn run(&self, args: &[&str]) -> Result<String> {
        let output = self
            .command(args)
            .output()
            .map_err(|error| Error(format!("cannot run git: {error}")))?;
        if output.status.success() {
            let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.truncate(text.trim_end_matches(['\r', '\n']).len());
            Ok(text)
        } else {
            let said = String::from_utf8_lossy(&output.stderr);
            Err(Error(format!(
                "git {}: {}",
                args.first().copied().unwrap_or_default(),
                said.trim()
            )))
        }
    }

    /// Writes the commit `id`'s files to the folder `to`, through an index
    /// of its own: the history's index and the kept folder stay as they are.
    pub(crate) fn export(&self, id: &str, to: &Path) -> Result<()> {
        let index = self
            .dir
            .join(format!("export-{}.index", std::process::id()));
        let prefix = format!("--prefix={}/", to.display());
        let run = |args: &[&str]| {
            let output = self
                .command(args)
                .env("GIT_INDEX_FILE", &index)
                .output()
                .map_err(|error| Error(format!("cannot run git: {error}")))?;
            if output.status.success() {
                return Ok(());
            }
            let said = String::from_utf8_lossy(&output.stderr);
            Err(Error(format!("git export: {}", said.trim())))
        };
        let done = run(&["read-tree", id]).and_then(|()| run(&["checkout-index", "-a", &prefix]));
        // Only a leftover if it stays: the next export writes over it.
        let _ = std::fs::remove_file(&index);
        done
    }

    /// [`Git::run`] for a question git answers by failing: `None` then.
    pub(crate) fn ask(&self, args: &[&str]) -> Option<String> {
        self.run(args).ok().filter(|answer| !answer.is_empty())
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = plain(&self.work);
        command
            .env("GIT_DIR", &self.dir)
            .env("GIT_WORK_TREE", &self.work)
            .args(["-c", "core.fsmonitor=false", "-c", "core.autocrlf=false"])
            .args(args);
        command
    }
}

/// Git in `folder` with nothing of the caller's environment or
/// configuration, for the history's repository or for creating it.
pub(crate) fn plain(folder: &Path) -> Command {
    let mut command = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    command
        .current_dir(folder)
        .stdin(Stdio::null())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "Moonframe")
        .env("GIT_AUTHOR_EMAIL", "moonframe@localhost")
        .env("GIT_COMMITTER_NAME", "Moonframe")
        .env("GIT_COMMITTER_EMAIL", "moonframe@localhost");
    command
}
