//! The agents `acp` knows how to start, and finding their commands: the
//! table herdr-acp uses (`herdr_chat_protocol::Agent`), with each agent's
//! ACP command first and an `npx` fallback for the adapters.

use std::{ffi::OsStr, path::PathBuf};

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

pub(crate) fn find(name: &str) -> Option<&'static Agent> {
    AGENTS.iter().find(|agent| agent.name == name)
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
            let Some(found) = moonframe_providers::program::find(program, path) else {
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
