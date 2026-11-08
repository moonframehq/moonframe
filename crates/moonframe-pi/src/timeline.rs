//! A conversation as what the window shows: a flat list of items, each small
//! and plain, from pi's messages. A run is what follows a prompt; its tool
//! rows and thinking fold away under one line once it is over.

use {
    crate::{
        engine::Entry,
        live::{Live, ToolState},
        value::{as_list, clip, display, first_chars, last_bytes, squeezed, text_of},
    },
    serde_json::{Map, Value, json},
    std::{
        collections::{HashMap, HashSet},
        path::Path,
    },
};

const OUTPUT_LIMIT: usize = 24_000;
const SHOWN_COMMAND: usize = 140;

type Item = Map<String, Value>;

fn shortened(value: &Value, limit: usize) -> String {
    clip(&squeezed(&display(value)), limit)
}

fn path_in(cwd: &str, path: &Value) -> String {
    // Not by `is_absolute`: Windows calls `/code/a` relative, and a
    // relative path never starts with the working directory anyway.
    let path = display(path);
    if path.is_empty() {
        return path;
    }
    Path::new(&path).strip_prefix(cwd).map_or_else(
        |_| path.clone(),
        |inside| inside.to_string_lossy().into_owned(),
    )
}

fn title_of(name: &str, args: &Value, cwd: &str, status: &str) -> String {
    let given = if args["path"].is_null() {
        &args["file_path"]
    } else {
        &args["path"]
    };
    let path = path_in(cwd, given);
    let running = status == "running";
    let doing = |now: &str, then: &str| {
        if running {
            now.to_owned()
        } else {
            then.to_owned()
        }
    };
    match name {
        "read" => format!("{} {path}", doing("Reading", "Read")),
        "edit" => format!("{} {path}", doing("Editing", "Edited")),
        "write" => format!("{} {path}", doing("Writing", "Wrote")),
        "bash" => format!(
            "{} {}",
            doing("Running", "Ran"),
            shortened(&args["command"], SHOWN_COMMAND)
        ),
        "grep" => format!("Searched for {}", shortened(&args["pattern"], 60)),
        "find" => format!("Found {}", shortened(&args["pattern"], 60)),
        "ls" => format!(
            "Listed {}",
            if path.is_empty() {
                "."
            } else {
                &path
            }
        ),
        _ => match args.as_object() {
            Some(fields) if !fields.is_empty() => {
                format!("{name} {}", shortened(&json!(args.to_string()), 80))
            },
            _ => name.to_owned(),
        },
    }
}

/// Lines added and removed, from a unified patch pi's edit gives.
fn count_patch(patch: &str) -> (i64, i64) {
    let (mut added, mut deleted) = (0, 0);
    for line in patch.split('\n') {
        if line.starts_with('+') && !line.starts_with("+++") {
            added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deleted += 1;
        }
    }
    (added, deleted)
}

fn images_in(content: &Value) -> usize {
    let images = as_list(content)
        .iter()
        .filter(|block| block["type"] == "image");
    images.count()
}

fn plural(count: usize, one: &str) -> String {
    format!(
        "{count} {one}{}",
        if count == 1 {
            ""
        } else {
            "s"
        }
    )
}

fn run_summary(tools: &[&Item]) -> String {
    let count = |names: &[&str]| {
        let named = |tool: &&&Item| {
            let name = tool.get("tool").and_then(Value::as_str).unwrap_or_default();
            names.contains(&name)
        };
        tools.iter().filter(named).count()
    };
    let read = count(&["read", "grep", "find", "ls"]);
    let edited = count(&["edit", "write"]);
    let ran = count(&["bash"]);
    let other = tools.len() - read - edited - ran;
    let mut parts = Vec::new();
    if read > 0 {
        parts.push(format!("explored {}", plural(read, "file")));
    }
    if edited > 0 {
        parts.push(format!("edited {}", plural(edited, "file")));
    }
    if ran > 0 {
        parts.push(format!("ran {}", plural(ran, "command")));
    }
    if other > 0 {
        parts.push(format!("used {}", plural(other, "tool")));
    }
    let text = parts.join(", ");
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

/// `seconds` as a person would say it.
pub(crate) fn duration(seconds: f64) -> String {
    #[allow(clippy::cast_possible_truncation, reason = "a run's length in seconds")]
    let whole = seconds.round().max(1.0) as i64;
    if whole < 60 {
        return format!("{whole}s");
    }
    let minutes = whole / 60;
    if minutes < 60 {
        return format!("{minutes}m {}s", whole % 60);
    }
    format!("{}h {}m", minutes / 60, minutes % 60)
}

/// What the timeline is built with beyond the entries.
pub(crate) struct Context<'a> {
    pub(crate) cwd: &'a str,
    /// What is happening now: the message streaming in, the tools running,
    /// whether a run is going.
    pub(crate) live: Option<&'a Live>,
    /// The snapshots of finished runs by their prompt's timestamp.
    pub(crate) turns: &'a Map<String, Value>,
    /// The tool rows showing output.
    pub(crate) expanded: Option<&'a HashSet<String>>,
    pub(crate) now: f64,
}

struct Run {
    id: String,
    key: Option<String>,
    started: f64,
    last: f64,
    header: usize,
}

struct Builder<'a> {
    context: &'a Context<'a>,
    items: Vec<Item>,
    tools: HashMap<String, usize>,
    run: Option<Run>,
}

fn item(value: Value) -> Item {
    match value {
        Value::Object(fields) => fields,
        _ => Item::new(),
    }
}

fn is(item: &Item, key: &str, wanted: &str) -> bool {
    item.get(key).and_then(Value::as_str) == Some(wanted)
}

fn millis(value: &Value) -> Option<f64> {
    value.as_f64().map(|stamp| stamp / 1000.0)
}

impl Builder<'_> {
    fn row_tools(&self, run: &str) -> Vec<&Item> {
        let rows = self.items.iter();
        rows.filter(|item| is(item, "k", "tool") && is(item, "run", run))
            .collect()
    }

    fn close_run(&mut self) {
        let Some(run) = self.run.take() else {
            return;
        };
        let rows = self.row_tools(&run.id);
        let (summary, steps) = (run_summary(&rows), rows.len());
        let header = &mut self.items[run.header];
        header.insert("summary".to_owned(), json!(summary));
        header.insert("steps".to_owned(), json!(steps));
        let label = format!("Worked for {}", duration(run.last - run.started));
        header.insert("label".to_owned(), json!(label));
        // The last words stay when the steps fold away.
        let after = self.items.iter_mut().skip(run.header + 1);
        if let Some(last) = after.rev().find(|item| is(item, "k", "text")) {
            last.insert("answer".to_owned(), json!(true));
        }
        let turn = run.key.as_ref().and_then(|key| self.context.turns.get(key));
        if let Some(turn) = turn.filter(|turn| !as_list(&turn["files"]).is_empty()) {
            let files = as_list(&turn["files"]);
            self.items.push(item(json!({
                "k": "changes",
                "id": format!("{}-changes", run.id),
                "run": run.id,
                "files": files.iter().take(12).collect::<Vec<_>>(),
                "count": files.len(),
                "added": turn["added"],
                "deleted": turn["deleted"],
                "from": turn["before"],
                "to": turn["after"],
            })));
        }
    }

    fn open_run(&mut self, id: &str, key: Option<String>, started: f64) {
        let run = Run {
            id: format!("run-{id}"),
            key,
            started,
            last: started,
            header: self.items.len(),
        };
        self.items.push(item(json!({
            "k": "run", "id": run.id, "label": "", "summary": "", "steps": 0, "running": false,
        })));
        self.run = Some(run);
    }

    fn add_assistant(&mut self, message: &Value, id: &str, streaming: bool) {
        if self.run.is_none() {
            let started = millis(&message["timestamp"]).unwrap_or(self.context.now);
            self.open_run(id, None, started);
        }
        let stamp = millis(&message["timestamp"]);
        let Some(run) = self.run.as_mut() else {
            return;
        };
        if let Some(stamp) = stamp {
            run.last = stamp;
        }
        let run_id = run.id.clone();
        for (index, block) in as_list(&message["content"]).iter().enumerate() {
            let key = format!("{id}-{index}");
            self.add_block(block, &key, &run_id, streaming);
        }
        let error = message["errorMessage"]
            .as_str()
            .filter(|text| !text.is_empty());
        match (message["stopReason"].as_str(), error) {
            (Some("error"), Some(error)) => self.items.push(item(json!({
                "k": "error", "id": format!("{id}-error"), "run": run_id, "text": error,
            }))),
            (Some("aborted"), _) => self.items.push(item(json!({
                "k": "note", "id": format!("{id}-stopped"), "run": run_id,
                "text": "Stopped", "tone": "muted",
            }))),
            _ => {},
        }
    }

    fn add_block(&mut self, block: &Value, key: &str, run: &str, streaming: bool) {
        let mut row = match block["type"].as_str() {
            Some("text") if block["text"].as_str().is_some_and(|text| !text.is_empty()) => {
                item(json!({ "k": "text", "id": key, "run": run, "text": block["text"] }))
            },
            Some("thinking")
                if block["thinking"]
                    .as_str()
                    .is_some_and(|text| !text.trim().is_empty()) =>
            {
                item(json!({ "k": "think", "id": key, "run": run, "text": block["thinking"] }))
            },
            Some("toolCall") => {
                let id = display(&block["id"]);
                let name = display(&block["name"]);
                let status = "running";
                let title = title_of(&name, &block["arguments"], self.context.cwd, status);
                let mut row = item(json!({
                    "k": "tool", "id": id, "run": run, "tool": name, "title": title,
                    "args": block["arguments"], "status": status,
                }));
                if self.context.expanded.is_some_and(|set| set.contains(&id)) {
                    row.insert("open".to_owned(), json!(true));
                }
                self.tools.insert(id, self.items.len());
                self.items.push(row);
                return;
            },
            _ => return,
        };
        if streaming {
            row.insert("streaming".to_owned(), json!(true));
        }
        self.items.push(row);
    }

    fn add_result(&mut self, message: &Value) {
        let Some(&index) = message["toolCallId"]
            .as_str()
            .and_then(|id| self.tools.get(id))
        else {
            return;
        };
        let failed = message["isError"].as_bool().unwrap_or(false);
        self.finish_tool(index, &message["content"], &message["details"], failed);
        let stamp = millis(&message["timestamp"]);
        if let (Some(stamp), Some(run)) = (stamp, self.run.as_mut()) {
            run.last = stamp;
        }
    }

    fn finish_tool(&mut self, index: usize, content: &Value, details: &Value, failed: bool) {
        let cwd = self.context.cwd;
        let row = &mut self.items[index];
        let status = if failed {
            "error"
        } else {
            "done"
        };
        row.insert("status".to_owned(), json!(status));
        let tool = row.get("tool").map(display).unwrap_or_default();
        let args = row.get("args").cloned().unwrap_or(Value::Null);
        row.insert(
            "title".to_owned(),
            json!(title_of(&tool, &args, cwd, status)),
        );
        let patch = details["patch"].as_str().filter(|patch| !patch.is_empty());
        if let Some(patch) = patch {
            let (added, deleted) = count_patch(patch);
            row.insert("added".to_owned(), json!(added));
            row.insert("deleted".to_owned(), json!(deleted));
        } else if tool == "write" && !args["content"].is_null() {
            let lines = display(&args["content"]).split('\n').count();
            row.insert("added".to_owned(), json!(lines));
            row.insert("deleted".to_owned(), json!(0));
        }
        if row.contains_key("open") {
            let output = text_of(content);
            let output = first_chars(&output, OUTPUT_LIMIT);
            row.insert("output".to_owned(), json!(output));
            if let Some(patch) = patch {
                row.insert("patch".to_owned(), json!(first_chars(patch, OUTPUT_LIMIT)));
            }
        }
    }

    fn add_entry(&mut self, entry: &Entry) {
        if let Some(note) = &entry.note {
            self.items.push(item(json!({
                "k": "note", "id": entry.id, "text": note,
                "tone": entry.tone.as_deref().unwrap_or("muted"),
            })));
            return;
        }
        let Some(message) = &entry.message else {
            return;
        };
        let stamp = &message["timestamp"];
        let own_id = |prefix: &str| {
            entry
                .id
                .clone()
                .unwrap_or_else(|| format!("{prefix}{}", display(stamp)))
        };
        match message["role"].as_str() {
            Some("user") => {
                self.close_run();
                let id = own_id("u");
                let mut row = item(json!({
                    "k": "user", "id": id, "text": text_of(&message["content"]),
                }));
                if let Some(entry_id) = &entry.id {
                    row.insert("entry".to_owned(), json!(entry_id));
                }
                let images = images_in(&message["content"]);
                if images > 0 {
                    row.insert("images".to_owned(), json!(images));
                }
                self.items.push(row);
                self.open_run(&id, Some(display(stamp)), millis(stamp).unwrap_or(0.0));
            },
            Some("assistant") => self.add_assistant(message, &own_id("a"), false),
            Some("toolResult") => self.add_result(message),
            Some("bashExecution") => {
                self.close_run();
                let output = display(&message["output"]);
                self.items.push(item(json!({
                    "k": "bash",
                    "id": own_id("b"),
                    "command": message["command"],
                    "code": message["exitCode"],
                    "output": last_bytes(&output, OUTPUT_LIMIT),
                })));
            },
            Some("custom") if message["display"].as_bool().unwrap_or(false) => {
                self.items.push(item(json!({
                    "k": "note", "id": entry.id, "text": text_of(&message["content"]), "tone": "muted",
                })));
            },
            _ => {},
        }
    }

    fn add_live(&mut self, live: &Live) {
        if let Some(partial) = &live.partial {
            self.add_assistant(partial, "live", true);
        }
        for (id, state) in &live.tools {
            let Some(&index) = self.tools.get(id) else {
                continue;
            };
            match state {
                ToolState::Done {
                    content,
                    details,
                    failed,
                } => self.finish_tool(index, content, details, *failed),
                ToolState::Working { content } => {
                    let row = &mut self.items[index];
                    if row.contains_key("open") && !content.is_null() {
                        let output = text_of(content);
                        row.insert(
                            "output".to_owned(),
                            json!(last_bytes(&output, OUTPUT_LIMIT)),
                        );
                    }
                },
            }
        }
    }

    fn finish(mut self) -> Vec<Item> {
        let live = self.context.live;
        let running = live.is_some_and(|live| live.running);
        match self.run.take() {
            Some(run) if running => {
                let rows = self.row_tools(&run.id);
                let (summary, steps) = (run_summary(&rows), rows.len());
                let label = format!("Working for {}", duration(self.context.now - run.started));
                let header = &mut self.items[run.header];
                header.insert("running".to_owned(), json!(true));
                header.insert("label".to_owned(), json!(label));
                header.insert("summary".to_owned(), json!(summary));
                header.insert("steps".to_owned(), json!(steps));
            },
            other => {
                self.run = other;
                self.close_run();
            },
        }
        // A run with no steps needs no line of its own.
        self.items.retain(|item| {
            let steps = item.get("steps").and_then(Value::as_u64).unwrap_or(0);
            let running = item
                .get("running")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            !is(item, "k", "run") || steps > 0 || running
        });
        self.items
    }
}

/// The items for `entries`, oldest first.
pub(crate) fn timeline(entries: &[Entry], context: &Context<'_>) -> Vec<Item> {
    let mut builder = Builder {
        context,
        items: Vec::new(),
        tools: HashMap::new(),
        run: None,
    };
    for entry in entries {
        builder.add_entry(entry);
    }
    if let Some(live) = context.live {
        builder.add_live(live);
    }
    builder.finish()
}

/// What the context holds, from the last answer's usage.
pub(crate) fn context_use(entries: &[Entry], window: Option<f64>) -> Value {
    let answers = entries
        .iter()
        .rev()
        .filter_map(|entry| entry.message.as_ref());
    for message in answers.filter(|message| message["role"] == "assistant") {
        let usage = &message["usage"];
        let part = |key: &str| usage[key].as_f64().unwrap_or(0.0);
        let tokens = part("input") + part("output") + part("cacheRead") + part("cacheWrite");
        if tokens == 0.0 {
            continue;
        }
        let used = window
            .filter(|window| *window != 0.0)
            .map(|w| (tokens / w).min(1.0));
        return json!({ "tokens": tokens, "window": window, "used": used });
    }
    Value::Null
}

/// What the conversation has cost.
pub(crate) fn total_cost(entries: &[Entry]) -> f64 {
    let messages = entries.iter().filter_map(|entry| entry.message.as_ref());
    let answers = messages.filter(|message| message["role"] == "assistant");
    let cost: f64 = answers
        .filter_map(|message| message["usage"]["cost"]["total"].as_f64())
        .sum();
    (cost * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;
