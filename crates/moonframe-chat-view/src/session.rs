//! What the session is beyond its conversation: the agents it may hand
//! over to, the pull requests its agent worked on, and who else watches.

use {
    crate::text,
    herdr_chat_protocol::{PullRequestAction, Transcript},
    serde::Serialize,
};

/// The most pull requests a view lists.
const MAX_PULL_REQUESTS: usize = 64;

/// An agent the chat may switch to, sent as `{ agent = id, chat }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AgentChoice {
    /// Its name, such as `codex`.
    pub id: &'static str,
    /// Its display name, such as `Codex`.
    pub name: &'static str,
    /// The one the chat runs now.
    pub current: bool,
}

/// A pull request the agent worked on in this chat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PullRequestView {
    pub url: String,
    /// `OWNER/REPO`.
    pub repo: String,
    pub number: u64,
    /// `"created" "pushed" "edited" "commented" "reviewed" "ready" "merged"
    /// "closed" "reopened" "other"`, each once, first seen first.
    pub actions: Vec<&'static str>,
    /// The latest of them, for a badge: `Merged`, `Pushed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<&'static str>,
}

/// The agents a chat offers while its agent is idle; empty for one that
/// cannot switch (continued sessions, and programs before switching).
pub(crate) fn agents(transcript: &Transcript) -> Vec<AgentChoice> {
    let Some(harnesses) = &transcript.harnesses else {
        return Vec::new();
    };
    if harnesses.available.len() < 2 {
        return Vec::new();
    }
    harnesses
        .available
        .iter()
        .map(|agent| AgentChoice {
            id: agent.name(),
            name: agent.display_name(),
            current: harnesses.current == Some(*agent),
        })
        .collect()
}

pub(crate) fn pull_requests(transcript: &Transcript) -> Vec<PullRequestView> {
    transcript
        .pull_requests
        .iter()
        .take(MAX_PULL_REQUESTS)
        .map(|pull| PullRequestView {
            url: text::head(&pull.url, 512),
            repo: text::head(&pull.repo, 200),
            number: pull.number,
            actions: pull
                .actions
                .iter()
                .map(|action| action_name(*action))
                .collect(),
            state: pull.actions.last().map(|action| action_label(*action)),
        })
        .collect()
}

/// The row key of the notice a continued session's earlier conversation
/// ends at: rows up to it came before this chat, and a view may fold them.
pub(crate) fn continued(transcript: &Transcript) -> Option<String> {
    let id = transcript.session.as_ref()?.continued?;
    transcript
        .entries
        .iter()
        .any(|entry| entry.id == id)
        .then(|| format!("e{}", id.0))
}

fn action_name(action: PullRequestAction) -> &'static str {
    match action {
        PullRequestAction::Created => "created",
        PullRequestAction::Pushed => "pushed",
        PullRequestAction::Edited => "edited",
        PullRequestAction::Commented => "commented",
        PullRequestAction::Reviewed => "reviewed",
        PullRequestAction::Ready => "ready",
        PullRequestAction::Merged => "merged",
        PullRequestAction::Closed => "closed",
        PullRequestAction::Reopened => "reopened",
        PullRequestAction::Other => "other",
    }
}

fn action_label(action: PullRequestAction) -> &'static str {
    match action {
        PullRequestAction::Created => "Created",
        PullRequestAction::Pushed => "Pushed",
        PullRequestAction::Edited => "Edited",
        PullRequestAction::Commented => "Commented",
        PullRequestAction::Reviewed => "Reviewed",
        PullRequestAction::Ready => "Ready",
        PullRequestAction::Merged => "Merged",
        PullRequestAction::Closed => "Closed",
        PullRequestAction::Reopened => "Reopened",
        PullRequestAction::Other => "Updated",
    }
}
