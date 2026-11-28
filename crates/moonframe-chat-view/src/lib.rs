//! What a chat view draws, prepared from herdr's chat [`Transcript`]: no
//! GUI, no framework, only data a view walks top to bottom.
//!
//! - **Rows**, in order: the user's prompts (an app's context frame left
//!   out of the bubble, the files they carried as names), the agent's
//!   answers and thoughts, runs of tool calls folded into one group with a
//!   one-line title and status, pictures, notices, and each turn's summary
//!   (how long it took, the tokens it used). A sub-agent's own steps count
//!   in its call rather than showing as rows.
//! - **What waits on the user**: permission questions with the agent's
//!   options, and the prompts queued behind the running turn.
//! - **The session's controls**: the mode, model and effort pickers with
//!   their choices, any other setting, auto-accept, usage, the plan, the
//!   running sub-agents and shells, and the slash commands to complete.
//!
//! Everything is bounded by [`Limits`], so a long session prepares into
//! what a view can hold. A prompt an app framed with [`frame`] shows
//! without its frame; [`unframe`] reads one back.

// keep-sorted start
mod controls;
mod rows;
mod text;
mod tool;
// keep-sorted end

use {
    herdr_chat_protocol::{Status, Transcript},
    serde::Serialize,
};

pub use {
    controls::{
        ActivityView, ChoiceView, CommandView, PermissionChoice, PermissionView, Picker, PlanItem,
        PlanView, QueuedView, UsageView,
    },
    rows::{DiagramRef, FileChange, QuestionView, Row, ToolGroup, ToolItem, ToolView},
    text::{duration, tokens},
};

/// Opens the context frame an app puts before a prompt.
pub const CONTEXT_OPEN: &str = "<app-context>\n";
/// Closes it; the user's own prompt follows.
pub const CONTEXT_CLOSE: &str = "\n</app-context>\n\n";

/// `prompt` with `context` before it, in a frame [`unframe`] (and so the
/// view) recognizes: the agent reads both, the user's bubble shows only
/// the prompt. An empty context frames nothing.
pub fn frame(context: &str, prompt: &str) -> String {
    let context = context.trim();
    if context.is_empty() {
        return prompt.to_owned();
    }
    format!("{CONTEXT_OPEN}{context}{CONTEXT_CLOSE}{prompt}")
}

/// A prompt's context frame and the prompt after it; no frame, no context.
pub fn unframe(text: &str) -> (Option<&str>, &str) {
    text.strip_prefix(CONTEXT_OPEN)
        .and_then(|rest| rest.split_once(CONTEXT_CLOSE))
        .map_or((None, text), |(context, prompt)| (Some(context), prompt))
}

/// How much a prepared view holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Rows kept, from the end; `hidden` counts the others.
    pub rows: usize,
    /// Bytes of an answer or a thought kept, from its end: the end is what
    /// is being read as it streams.
    pub answer: usize,
    /// Bytes of any other text kept: a prompt, a notice, a title.
    pub text: usize,
    /// Lines, and bytes, of a tool's output shown under its title.
    pub tool_lines: usize,
    pub tool_text: usize,
    /// Tool calls one group keeps, from its end; `more` counts the others.
    pub group: usize,
    /// Slash commands, choices of one picker, plan items, queued prompts
    /// and permission questions kept.
    pub commands: usize,
    pub choices: usize,
    pub plan: usize,
    pub queue: usize,
    pub permissions: usize,
}

impl Limits {
    /// The chat a view shows.
    pub const FULL: Self = Self {
        rows: 200,
        answer: 256 * 1024,
        text: 15 * 1024,
        tool_lines: 6,
        tool_text: 2 * 1024,
        group: 60,
        commands: 200,
        choices: 200,
        plan: 100,
        queue: 50,
        permissions: 16,
    };
    /// A chat in a list: its last few rows, small.
    pub const SHORT: Self = Self {
        rows: 4,
        answer: 2 * 1024,
        text: 1024,
        tool_lines: 0,
        tool_text: 0,
        group: 4,
        commands: 0,
        choices: 0,
        plan: 20,
        queue: 10,
        permissions: 4,
    };
}

/// One chat as a view draws it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatView {
    /// The agent's display name, such as `Claude`.
    pub agent: String,
    /// `"starting" "idle" "working" "failed" "exited"`.
    pub status: &'static str,
    /// Why it failed or exited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// When the running turn started (Unix ms), while one runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since_ms: Option<u64>,
    /// It takes prompts now (perhaps queued behind the running turn).
    pub accepts: bool,
    /// The session's title, as the agent named it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The agent's session id, to continue it later.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Another client drives it; this one only watches.
    pub read_only: bool,
    pub auto_accept: bool,
    /// Rows before the first one kept.
    pub hidden: usize,
    /// Herdr dropped the oldest entries of a long session.
    pub truncated: bool,
    pub rows: Vec<Row>,
    /// Questions waiting for the user, oldest first.
    pub permissions: Vec<PermissionView>,
    /// Prompts waiting for the running turn, oldest first.
    pub queue: Vec<QueuedView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<Picker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<Picker>,
    /// How long the model thinks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<Picker>,
    /// The session's other settings.
    pub settings: Vec<Picker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageView>,
    /// Slash commands, for completing `/`.
    pub commands: Vec<CommandView>,
    /// Sub-agents and background shells still running.
    pub activities: Vec<ActivityView>,
    /// One line for a list of chats: what it last said or is doing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
}

/// Prepares `transcript` for a view, within `limits`.
pub fn prepare(transcript: &Transcript, limits: &Limits) -> ChatView {
    let (status, message) = match &transcript.status {
        Status::Starting => ("starting", None),
        Status::Idle => ("idle", None),
        Status::Working => ("working", None),
        Status::Failed { message } => ("failed", Some(text::head(message, limits.text))),
        Status::Exited { message } => ("exited", Some(text::head(message, limits.text))),
    };
    let working = transcript.status == Status::Working;
    let mut all = rows::rows(transcript, working, limits);
    let last = rows::last_line(&all);
    let hidden = all.len().saturating_sub(limits.rows);
    all.drain(..hidden);
    let session = transcript.session.as_ref();
    ChatView {
        agent: text::head(&transcript.agent, 200),
        status,
        message,
        since_ms: transcript
            .turn
            .filter(|_| working)
            .map(|turn| turn.started_ms),
        accepts: transcript.status.accepts_prompts() && !transcript.read_only,
        title: session
            .and_then(|session| session.title.as_deref())
            .map(|title| text::head(title, 500)),
        session: session.map(|session| session.id.clone()),
        read_only: transcript.read_only,
        auto_accept: transcript.auto_accept,
        hidden,
        truncated: transcript.truncated,
        rows: all,
        permissions: controls::permissions(transcript, limits),
        queue: controls::queue(transcript, limits),
        plan: controls::plan(transcript, limits),
        mode: controls::mode(transcript, limits),
        model: controls::option(
            transcript,
            herdr_chat_protocol::OptionCategory::Model,
            limits,
        ),
        effort: controls::option(
            transcript,
            herdr_chat_protocol::OptionCategory::ThoughtLevel,
            limits,
        ),
        settings: controls::settings(transcript, limits),
        usage: transcript.usage.as_ref().map(controls::usage),
        commands: controls::commands(transcript, limits),
        activities: controls::activities(transcript),
        last,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
