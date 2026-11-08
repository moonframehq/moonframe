//! Where pi and this app keep things, and what pi runs with.

use std::{
    env,
    path::{Path, PathBuf},
};

/// What a provider instance was given. The app builds one from the
/// environment; tests build their own, in folders of their own.
#[derive(Clone, Debug)]
pub struct Config {
    /// pi's own folder: settings, credentials, sessions.
    pub agent_dir: PathBuf,
    /// This app's folder.
    pub data_dir: PathBuf,
    /// The home folder.
    pub home: PathBuf,
    /// The pi to prefer over any found: `PILOT_PI`.
    pub pi: Option<PathBuf>,
    /// Takes PATH and provider keys from the login shell, as a window
    /// opened from the Dock has neither.
    pub login_shell: bool,
    /// More environment for what runs here.
    pub env: Vec<(String, String)>,
}

fn given(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

impl Config {
    /// From `PI_CODING_AGENT_DIR`, `PILOT_DATA`, `XDG_DATA_HOME` and
    /// `PILOT_PI`, with pi's and the app's usual folders otherwise.
    pub fn from_env() -> Self {
        let home = given("HOME").unwrap_or_default();
        let shared = given("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
        Self {
            agent_dir: given("PI_CODING_AGENT_DIR").unwrap_or_else(|| home.join(".pi/agent")),
            data_dir: given("PILOT_DATA").unwrap_or_else(|| shared.join("moonframe/pilot")),
            pi: given("PILOT_PI"),
            home,
            login_shell: true,
            env: Vec::new(),
        }
    }

    pub(crate) fn worktrees_dir(&self) -> PathBuf {
        self.data_dir.join("worktrees")
    }

    /// Where a thread with no project runs: pi always needs a working
    /// directory.
    pub(crate) fn chats_dir(&self) -> PathBuf {
        self.data_dir.join("chats")
    }

    /// The folder pi keeps a working directory's sessions in, as pi names it.
    pub(crate) fn session_dir(&self, cwd: &str) -> PathBuf {
        let inner = cwd.strip_prefix(['/', '\\']).unwrap_or(cwd);
        let safe: String = inner
            .chars()
            .map(|c| {
                if matches!(c, '/' | '\\' | ':') {
                    '-'
                } else {
                    c
                }
            })
            .collect();
        self.agent_dir.join("sessions").join(format!("--{safe}--"))
    }

    /// `path` with the home folder as `~`.
    pub(crate) fn short_path(&self, path: &str) -> String {
        let home = self.home.to_string_lossy();
        match path.strip_prefix(home.as_ref()) {
            Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
                format!("~{rest}")
            },
            _ => path.to_owned(),
        }
    }
}

/// The last part of a path.
pub(crate) fn base_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `path` with links resolved, as text.
pub(crate) fn real_path(path: &str) -> Result<String, String> {
    std::fs::canonicalize(path)
        .map(|real| real.to_string_lossy().into_owned())
        .map_err(|error| format!("{path}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            agent_dir: "/a".into(),
            data_dir: "/d".into(),
            home: "/home/me".into(),
            pi: None,
            login_shell: false,
            env: Vec::new(),
        }
    }

    #[test]
    fn a_session_folder_is_named_as_pi_names_it() {
        assert_eq!(
            config().session_dir("/Users/me/code/x"),
            Path::new("/a/sessions/--Users-me-code-x--")
        );
    }

    #[test]
    fn the_home_folder_shortens_only_whole() {
        assert_eq!(config().short_path("/home/me/a"), "~/a");
        assert_eq!(config().short_path("/home/me"), "~");
        assert_eq!(config().short_path("/home/meat"), "/home/meat");
    }
}
