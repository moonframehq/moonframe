//! The agents `acp` knows how to start, and finding their commands: the
//! table herdr-acp uses (`herdr_chat_protocol::Agent`), with each agent's
//! ACP command first and an `npx` fallback for the adapters.

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

/// An agent: its name in messages, its title, how to sign in, and the
/// commands that start it over ACP, most preferred first.
pub(crate) struct Agent {
    pub(crate) name: &'static str,
    pub(crate) title: &'static str,
    pub(crate) login: &'static str,
    candidates: &'static [&'static [&'static str]],
}

pub(crate) const AGENTS: &[Agent] = &[
    Agent {
        name: "claude",
        title: "Claude",
        login: "run `claude` and use /login",
        candidates: &[&["claude-agent-acp"], &[
            "npx",
            "-y",
            "@agentclientprotocol/claude-agent-acp",
        ]],
    },
    Agent {
        name: "codex",
        title: "Codex",
        login: "run `codex login`",
        candidates: &[&["codex-acp"], &[
            "npx",
            "-y",
            "@agentclientprotocol/codex-acp",
        ]],
    },
    Agent {
        name: "copilot",
        title: "Copilot",
        login: "run `copilot` and use /login",
        candidates: &[&["copilot", "--acp"]],
    },
    Agent {
        name: "gemini",
        title: "Gemini",
        login: "run `gemini` once to sign in",
        candidates: &[&["gemini", "--acp"]],
    },
    Agent {
        name: "opencode",
        title: "OpenCode",
        login: "run `opencode auth login`",
        candidates: &[&["opencode", "acp"]],
    },
];

/// The runner that fetches an adapter package on first use.
const FETCHER: &str = "npx";

/// Where agents install themselves besides `PATH`, under the home folder:
/// an app started from the Dock gets a `PATH` that leaves them out.
const USER_DIRS: [&str; 8] = [
    ".local/bin",
    ".cargo/bin",
    ".bun/bin",
    ".npm-global/bin",
    ".opencode/bin",
    ".volta/bin",
    ".claude/local",
    ".local/share/mise/shims",
];
#[cfg(unix)]
const SYSTEM_DIRS: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];
#[cfg(not(unix))]
const SYSTEM_DIRS: [&str; 0] = [];

pub(crate) fn find(name: &str) -> Option<&'static Agent> {
    AGENTS.iter().find(|agent| agent.name == name)
}

/// `path`'s folders, then the user and system folders agents install to.
pub(crate) fn search_path(path: Option<&OsStr>, home: Option<&Path>) -> OsString {
    let mut dirs: Vec<PathBuf> = path
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .collect();
    dirs.extend(
        home.into_iter()
            .flat_map(|home| USER_DIRS.iter().map(|dir| home.join(dir))),
    );
    dirs.extend(SYSTEM_DIRS.iter().map(PathBuf::from));
    std::env::join_paths(dirs).unwrap_or_default()
}

/// How an agent can be started here.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Found {
    /// Its own ACP command is installed.
    Installed(PathBuf, Vec<String>),
    /// Only `npx` can start it, downloading the adapter on first use.
    Fetched(PathBuf, Vec<String>),
    Missing,
}

impl Agent {
    pub(crate) fn locate(&self, path: &OsStr) -> Found {
        let mut fetched = None;
        for command in self.candidates {
            let Some((program, args)) = command.split_first() else {
                continue;
            };
            let Some(found) = find_program(program, path) else {
                continue;
            };
            let args = args.iter().map(|arg| (*arg).to_owned()).collect();
            if *program == FETCHER {
                fetched.get_or_insert(Found::Fetched(found, args));
            } else {
                return Found::Installed(found, args);
            }
        }
        fetched.unwrap_or(Found::Missing)
    }

    /// What to install to run it, for an agent with no command here.
    pub(crate) fn missing(&self) -> String {
        let first = self.candidates.first().and_then(|command| command.first());
        format!(
            "{} is not installed here (looked for `{}`)",
            self.title,
            first.copied().unwrap_or(self.name)
        )
    }
}

/// Looks `program` up in a `PATH`-style list of folders.
pub(crate) fn find_program(program: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path).find_map(|dir| executable(&dir, program))
}

#[cfg(unix)]
fn executable(dir: &Path, program: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let candidate = dir.join(program);
    let metadata = candidate.metadata().ok()?;
    (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then_some(candidate)
}

#[cfg(windows)]
fn executable(dir: &Path, program: &str) -> Option<PathBuf> {
    ["exe", "cmd", "bat"]
        .iter()
        .map(|extension| dir.join(program).with_extension(extension))
        .find(|candidate| candidate.is_file())
}
