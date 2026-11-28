//! The transcript's entries as rows: a run of tool calls (with the thoughts
//! between them) becomes one group, a sub-agent's steps count in its call,
//! and a prompt loses its context frame and its file lines.

use {
    crate::{Limits, text, tool, unframe},
    herdr_chat_protocol::{
        ATTACHED, Author, Entry, EntryKind, NoticeLevel, Tool, Transcript, TurnEnd,
    },
    serde::Serialize,
    std::collections::{HashMap, HashSet},
};

/// One row of the conversation. Each has a `key`, the same for as long as
/// the row lives, to keep its fold and to scroll to.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Row {
    /// What the user said.
    User {
        key: String,
        text: String,
        /// The names of the files it carried.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        files: Vec<String>,
        /// It went with an app's context frame, which is not shown.
        context: bool,
    },
    /// The agent's answer, Markdown.
    Answer {
        key: String,
        text: String,
        /// Still being written.
        streaming: bool,
    },
    /// The agent's reasoning, shown folded under its first line.
    Thought {
        key: String,
        text: String,
        gist: String,
        streaming: bool,
    },
    /// A run of tool calls.
    Tools(ToolGroup),
    /// A picture in the conversation; its bytes are not carried.
    Image {
        key: String,
        /// `"user"` or `"agent"`.
        author: &'static str,
    },
    /// Something the program says about the session.
    Notice {
        key: String,
        /// `"info" "warning" "error"`.
        level: &'static str,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        answers: Vec<QuestionView>,
    },
    /// How a turn ended.
    Turn {
        key: String,
        /// `"done" "cancelled" "limit" "refused" "failed" "other"`.
        end: &'static str,
        duration_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        tokens: Option<u64>,
        /// `Worked 1m 12s · 12.4k tokens`.
        text: String,
    },
}

impl Row {
    pub fn key(&self) -> &str {
        match self {
            Self::User { key, .. }
            | Self::Answer { key, .. }
            | Self::Thought { key, .. }
            | Self::Image { key, .. }
            | Self::Notice { key, .. }
            | Self::Turn { key, .. } => key,
            Self::Tools(group) => &group.key,
        }
    }
}

/// A run of tool calls between the agent's words, folded to one line.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolGroup {
    pub key: String,
    /// `Read 3 files, ran 2 commands`, or the one call's own title.
    pub title: String,
    /// `"running"` while any call runs, `"failed"` when the last one
    /// failed, else `"done"`.
    pub status: &'static str,
    /// The turn is working on it now: nothing came after it yet. A view
    /// shows it open while it is live and folds it once it is not.
    pub live: bool,
    /// Its calls, and how many of them failed.
    pub count: usize,
    pub failed: usize,
    /// The title of the call running now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    /// Files it changed, with their line counts, totalled per file.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileChange>,
    /// Items before the first one kept.
    pub more: usize,
    pub items: Vec<ToolItem>,
    /// The diagrams its `moonframe mcp diagrams` calls showed, each once
    /// in the order they came: kept whole, as `items` may drop them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagrams: Vec<DiagramRef>,
}

/// A diagram a call showed: its id, and the run its answer named, which
/// tells it from another session's diagram of the same id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DiagramRef {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

/// A call in a group, or a thought between calls.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ToolItem {
    Tool(ToolView),
    Thought {
        key: String,
        gist: String,
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolView {
    /// Transcript entry used to retrieve the original text on demand.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_key: Option<String>,
    /// The agent's id for the call.
    pub id: String,
    /// `"read" "edit" "delete" "move" "search" "execute" "think" "fetch"
    /// "switch_mode" "subagent" "other"`.
    pub kind: &'static str,
    pub title: String,
    /// `"pending" "running" "done" "failed"`.
    pub status: &'static str,
    /// The end of its output, or a sub-agent's running step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileChange>,
    /// Calls a sub-agent made under it.
    pub steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileChange {
    pub path: String,
    /// Lines added and removed, when herdr carried both sides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<u64>,
    /// A file the call wrote anew.
    pub new: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QuestionView {
    pub question: String,
    pub answer: String,
}

/// Every entry of `transcript` as rows, oldest first.
pub(crate) fn rows(transcript: &Transcript, working: bool, limits: &Limits) -> Vec<Row> {
    let calls: HashSet<&str> = transcript
        .entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            EntryKind::Tool(tool) => Some(tool.call_id.as_str()),
            _ => None,
        })
        .collect();
    // A sub-agent's steps, by the call they are steps of.
    let mut steps: HashMap<&str, Vec<&Tool>> = HashMap::new();
    for entry in &transcript.entries {
        if let EntryKind::Tool(tool) = &entry.kind
            && let Some(parent) = tool.parent.as_deref()
            && calls.contains(parent)
        {
            steps.entry(parent).or_default().push(tool);
        }
    }
    let is_step = |tool: &Tool| {
        tool.parent
            .as_deref()
            .is_some_and(|parent| calls.contains(parent))
    };
    let mut rows = Vec::new();
    let mut run: Vec<&Entry> = Vec::new();
    let last = transcript.entries.len().saturating_sub(1);
    for (index, entry) in transcript.entries.iter().enumerate() {
        let streaming = working && index == last;
        let key = key(entry);
        match &entry.kind {
            EntryKind::Tool(tool) if is_step(tool) => {},
            EntryKind::Tool(_) | EntryKind::Thought { .. } => run.push(entry),
            other => {
                flush(&mut rows, &mut run, &steps, false, limits);
                rows.push(row(key, other, streaming, limits));
            },
        }
    }
    flush(&mut rows, &mut run, &steps, working, limits);
    rows
}

fn key(entry: &Entry) -> String {
    format!("e{}", entry.id.0)
}

/// A row for an entry that is not a tool call or a thought.
fn row(key: String, kind: &EntryKind, streaming: bool, limits: &Limits) -> Row {
    match kind {
        EntryKind::User { text, files } => user(key, text, *files, limits),
        EntryKind::Agent { text } => Row::Answer {
            key,
            text: text::tail(text, limits.answer),
            streaming,
        },
        EntryKind::Image { author, .. } => Row::Image {
            key,
            author: match author {
                Author::User => "user",
                Author::Agent => "agent",
            },
        },
        EntryKind::Notice {
            answers,
            details,
            level,
            text,
        } => Row::Notice {
            key,
            level: match level {
                NoticeLevel::Info => "info",
                NoticeLevel::Warning => "warning",
                NoticeLevel::Error => "error",
            },
            text: text::head(text, limits.text),
            details: details
                .as_deref()
                .map(|details| text::head(details, limits.text)),
            answers: answers
                .iter()
                .take(32)
                .map(|answer| QuestionView {
                    question: text::head(&answer.question, limits.text.min(2048)),
                    answer: text::head(&answer.answer, limits.text.min(2048)),
                })
                .collect(),
        },
        EntryKind::Turn(summary) => {
            let end = match summary.end {
                TurnEnd::Done => "done",
                TurnEnd::Cancelled => "cancelled",
                TurnEnd::Limit => "limit",
                TurnEnd::Refused => "refused",
                TurnEnd::Failed => "failed",
                TurnEnd::Other => "other",
            };
            let took = text::duration(summary.duration_ms);
            let mut line = match summary.end {
                TurnEnd::Done | TurnEnd::Other => format!("Worked {took}"),
                TurnEnd::Cancelled => format!("Stopped after {took}"),
                TurnEnd::Limit => format!("Reached its limit after {took}"),
                TurnEnd::Refused => format!("Refused after {took}"),
                TurnEnd::Failed => format!("Failed after {took}"),
            };
            let tokens = summary
                .tokens
                .map(|tokens| tokens.total)
                .filter(|total| *total > 0);
            if let Some(total) = tokens {
                line.push_str(&format!(" · {} tokens", text::tokens(total)));
            }
            Row::Turn {
                key,
                end,
                duration_ms: summary.duration_ms,
                tokens,
                text: line,
            }
        },
        // Tool calls and thoughts go through `flush`.
        EntryKind::Thought { .. } | EntryKind::Tool(_) => Row::Answer {
            key,
            text: String::new(),
            streaming,
        },
    }
}

/// A prompt as its bubble shows it: no context frame, no file lines.
fn user(key: String, text: &str, files: usize, limits: &Limits) -> Row {
    let (context, prompt) = unframe(text);
    let mut lines: Vec<&str> = prompt.split('\n').collect();
    let mut names = Vec::new();
    for _ in 0..files {
        match lines.last() {
            Some(line) if line.starts_with(ATTACHED) => {
                names.push(text::head(&line[ATTACHED.len()..], 512));
                lines.pop();
            },
            _ => break,
        }
    }
    names.reverse();
    Row::User {
        key,
        text: text::head(lines.join("\n").trim_end(), limits.text),
        files: names,
        context: context.is_some(),
    }
}

/// Ends a run of thoughts and tool calls: one group when it called any
/// tool, else each thought as a row of its own.
fn flush(
    rows: &mut Vec<Row>,
    run: &mut Vec<&Entry>,
    steps: &HashMap<&str, Vec<&Tool>>,
    live: bool,
    limits: &Limits,
) {
    let entries = std::mem::take(run);
    let tools: Vec<&Tool> = entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            EntryKind::Tool(tool) => Some(tool),
            _ => None,
        })
        .collect();
    let last = entries.len().saturating_sub(1);
    if tools.is_empty() {
        for (index, entry) in entries.iter().enumerate() {
            if let EntryKind::Thought { text } = &entry.kind {
                rows.push(Row::Thought {
                    key: key(entry),
                    gist: gist(text),
                    text: text::tail(text, limits.answer),
                    streaming: live && index == last,
                });
            }
        }
        return;
    }
    let Some(first) = entries.first() else {
        return;
    };
    let mut items: Vec<ToolItem> = entries
        .iter()
        .map(|entry| match &entry.kind {
            EntryKind::Tool(tool) => {
                let steps = steps
                    .get(tool.call_id.as_str())
                    .map_or(&[][..], Vec::as_slice);
                ToolItem::Tool(tool::view(tool, steps, limits, key(entry)))
            },
            EntryKind::Thought { text } => ToolItem::Thought {
                key: key(entry),
                gist: gist(text),
                text: text::tail(text, limits.text),
            },
            _ => ToolItem::Thought {
                key: key(entry),
                gist: String::new(),
                text: String::new(),
            },
        })
        .collect();
    let running = tools.iter().any(|tool| tool::running(tool.status));
    let failed = tools
        .iter()
        .filter(|tool| tool.status == herdr_chat_protocol::ToolStatus::Failed)
        .count();
    let status = if running {
        "running"
    } else if tools
        .last()
        .is_some_and(|tool| tool.status == herdr_chat_protocol::ToolStatus::Failed)
    {
        "failed"
    } else {
        "done"
    };
    let current = tools
        .iter()
        .rev()
        .find(|tool| tool::running(tool.status))
        .and_then(|tool| text::first_line(&tool.title, 300));
    let files = totals(&items);
    let mut diagrams: Vec<DiagramRef> = Vec::new();
    for shown in tools.iter().filter_map(|tool| tool::diagram(tool)) {
        if !diagrams.contains(&shown) {
            diagrams.push(shown);
        }
    }
    let more = items.len().saturating_sub(limits.group);
    items.drain(..more);
    rows.push(Row::Tools(ToolGroup {
        key: key(first),
        title: tool::group_title(&tools, limits),
        status,
        live,
        count: tools.len(),
        failed,
        current,
        files,
        more,
        items,
        diagrams,
    }));
}

/// The files a group changed, each once, its counts summed.
fn totals(items: &[ToolItem]) -> Vec<FileChange> {
    let mut files: Vec<FileChange> = Vec::new();
    for item in items {
        let ToolItem::Tool(view) = item else {
            continue;
        };
        for change in &view.files {
            match files.iter_mut().find(|file| file.path == change.path) {
                Some(file) => {
                    file.added = file.added.zip(change.added).map(|(a, b)| a + b);
                    file.removed = file.removed.zip(change.removed).map(|(a, b)| a + b);
                },
                None => files.push(change.clone()),
            }
        }
    }
    files.truncate(50);
    files
}

/// A thought's first line, without the Markdown emphasis agents put on it.
fn gist(text: &str) -> String {
    text::first_line(text, 200)
        .map(|line| {
            line.replace("**", "")
                .replace("__", "")
                .trim_matches(|c| c == '#' || c == ' ')
                .to_owned()
        })
        .unwrap_or_default()
}

/// One line for a list of chats: the last thing said or done.
pub(crate) fn last_line(rows: &[Row]) -> Option<String> {
    rows.iter().rev().find_map(|row| match row {
        Row::Answer { text, .. } => text
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(|line| text::head(line, 200)),
        Row::Tools(group) => Some(group.current.clone().unwrap_or_else(|| group.title.clone())),
        Row::User { text, .. } => text::first_line(text, 200),
        _ => None,
    })
}
