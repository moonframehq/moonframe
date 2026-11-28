//! One tool call as a line in its group, and a group's one-line title.

use {
    crate::{
        Limits,
        rows::{DiagramRef, FileChange, ToolView},
        text,
    },
    herdr_chat_protocol::{EDIT_OMITTED, Tool, ToolContent, ToolKind, ToolStatus},
};

pub(crate) fn kind_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "read",
        ToolKind::Edit => "edit",
        ToolKind::Delete => "delete",
        ToolKind::Move => "move",
        ToolKind::Search => "search",
        ToolKind::Execute => "execute",
        ToolKind::Think => "think",
        ToolKind::Fetch => "fetch",
        ToolKind::SwitchMode => "switch_mode",
        ToolKind::Subagent => "subagent",
        ToolKind::Other => "other",
    }
}

pub(crate) fn status_name(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Pending => "pending",
        ToolStatus::InProgress => "running",
        ToolStatus::Completed => "done",
        ToolStatus::Failed => "failed",
    }
}

pub(crate) fn running(status: ToolStatus) -> bool {
    matches!(status, ToolStatus::Pending | ToolStatus::InProgress)
}

/// `tool` as its line shows it; `steps` are the calls a sub-agent made
/// under it, oldest first.
pub(crate) fn view(tool: &Tool, steps: &[&Tool], limits: &Limits, key: String) -> ToolView {
    let mut files = Vec::new();
    let mut output = None;
    for item in &tool.content {
        match item {
            ToolContent::Diff { new, old, path } => {
                files.push(change(path, old.as_deref(), new, limits));
            },
            ToolContent::Text { text } if !text.trim().is_empty() => output = Some(text.as_str()),
            ToolContent::Text { .. } => {},
            ToolContent::Image { .. } | ToolContent::Terminal { .. } => {},
        }
    }
    // Files a call names but whose change it does not carry.
    if files.is_empty()
        && matches!(
            tool.kind,
            ToolKind::Edit | ToolKind::Delete | ToolKind::Move
        )
    {
        files.extend(tool.locations.iter().take(8).map(|path| FileChange {
            path: text::head(path, limits.text.min(1024)),
            added: None,
            removed: None,
            new: false,
        }));
    }
    let running_step = steps
        .iter()
        .rev()
        .find(|step| running(step.status))
        .map(|step| text::head(&step.title, 300));
    ToolView {
        output_key: output
            .filter(|_| !running(tool.status) && running_step.is_none())
            .map(|_| key),
        id: text::head(&tool.call_id, 256),
        kind: kind_name(tool.kind),
        title: title(tool, limits),
        status: status_name(tool.status),
        detail: running_step.or_else(|| {
            output.and_then(|output| text::last_lines(output, limits.tool_lines, limits.tool_text))
        }),
        files,
        steps: steps.len(),
        started_ms: tool.started_ms,
    }
}

/// The diagram a successful `moonframe mcp diagrams` call showed or
/// changed: its answer names it first, in backticks ("Showing `plan` ...",
/// "`plan`: ..."), and the run it is for on its last line (`[run ID]`).
pub(crate) fn diagram(tool: &Tool) -> Option<DiagramRef> {
    if tool.status == ToolStatus::Failed || !tool.title.contains("diagram_") {
        return None;
    }
    let answer = tool.content.iter().find_map(|item| match item {
        ToolContent::Text { text } => Some(text.as_str()),
        _ => None,
    })?;
    let (_, rest) = answer.split_once('`')?;
    let (id, _) = rest.split_once('`')?;
    let run = answer
        .trim_end()
        .lines()
        .last()
        .and_then(|line| line.strip_prefix("[run ")?.strip_suffix(']'))
        .filter(|run| safe(run))
        .map(str::to_owned);
    safe(id).then(|| DiagramRef {
        id: id.to_owned(),
        run,
    })
}

/// An id a diagram or run may have: letters, digits, `-` and `_`.
fn safe(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A call's title, on one line; its kind's name when it has none.
fn title(tool: &Tool, limits: &Limits) -> String {
    text::first_line(&tool.title, limits.text.min(500)).unwrap_or_else(|| {
        let kind = kind_name(tool.kind).replace('_', " ");
        let mut chars = kind.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(chars).collect()
        })
    })
}

/// A file change with its line counts: lines added and removed between the
/// unchanged lines both sides start and end with. A side herdr left out
/// as too large has no honest count.
fn change(path: &str, old: Option<&str>, new: &str, limits: &Limits) -> FileChange {
    let path = text::head(path, limits.text.min(1024));
    if new == EDIT_OMITTED || old == Some(EDIT_OMITTED) {
        return FileChange {
            path,
            added: None,
            removed: None,
            new: old.is_none(),
        };
    }
    let Some(old) = old else {
        return FileChange {
            path,
            added: Some(count(new.lines().count())),
            removed: Some(0),
            new: true,
        };
    };
    let (old, new): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let end = old[start..]
        .iter()
        .rev()
        .zip(new[start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    FileChange {
        path,
        added: Some(count(new.len() - start - end)),
        removed: Some(count(old.len() - start - end)),
        new: false,
    }
}

fn count(lines: usize) -> u64 {
    u64::try_from(lines).unwrap_or(u64::MAX)
}

/// What a group of calls did, as one line: its call's own title when it
/// holds one, else counts by kind in the order they first ran, such as
/// `Read 3 files, ran 2 commands`.
pub(crate) fn group_title(tools: &[&Tool], limits: &Limits) -> String {
    if let [tool] = tools {
        return title(tool, limits);
    }
    let mut counts: Vec<(ToolKind, usize)> = Vec::new();
    for tool in tools {
        match counts.iter_mut().find(|(kind, _)| *kind == tool.kind) {
            Some((_, count)) => *count += 1,
            None => counts.push((tool.kind, 1)),
        }
    }
    let parts: Vec<String> = counts
        .into_iter()
        .map(|(kind, count)| phrase(kind, count))
        .collect();
    let line = parts.join(", ");
    let mut chars = line.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn phrase(kind: ToolKind, count: usize) -> String {
    let (verb, one, many) = match kind {
        ToolKind::Read => ("read", "file", "files"),
        ToolKind::Edit => ("edited", "file", "files"),
        ToolKind::Delete => ("deleted", "file", "files"),
        ToolKind::Move => ("moved", "file", "files"),
        ToolKind::Search => ("ran", "search", "searches"),
        ToolKind::Execute => ("ran", "command", "commands"),
        ToolKind::Think => ("thought", "time", "times"),
        ToolKind::Fetch => ("fetched", "page", "pages"),
        ToolKind::SwitchMode => ("switched mode", "time", "times"),
        ToolKind::Subagent => ("ran", "agent", "agents"),
        ToolKind::Other => ("used", "tool", "tools"),
    };
    match (kind, count) {
        (ToolKind::Think | ToolKind::SwitchMode, 1) => verb.to_owned(),
        (_, 1) => format!("{verb} 1 {one}"),
        _ => format!("{verb} {count} {many}"),
    }
}
