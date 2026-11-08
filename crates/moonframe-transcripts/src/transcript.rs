//! The transcript of the session opened: what was typed, what the agent
//! said, and the tools it called, the latest turns.

use {
    crate::{
        sessions::{Group, Session},
        text::{
            as_text, clip, contains, decode_object, escaped, find, one_line, strip_cwd, truthy,
            typed, unescape, visible,
        },
    },
    serde_json::{Map, Value, json},
    std::{
        fs::File,
        io::{BufRead, BufReader},
        sync::atomic::{AtomicBool, Ordering},
    },
};

/// Turns kept, the latest.
pub(crate) const TURNS: usize = 150;
/// Characters per text part.
pub(crate) const TEXT: usize = 5000;
/// Tool calls per group.
pub(crate) const TOOLS: usize = 40;
/// A tool call longer than this (a whole file written) is not decoded.
const LARGE: usize = 16 * 1024;
/// Escaped characters of a large tool call's detail looked at.
const LARGE_DETAIL: usize = 300;

struct Part {
    tools: bool,
    text: Value,
    calls: Vec<(Value, Value)>,
}

impl Part {
    fn text(text: Value) -> Self {
        Self {
            tools: false,
            text,
            calls: Vec::new(),
        }
    }

    fn json(&self) -> Value {
        let tools: Vec<Value> = self
            .calls
            .iter()
            .map(|(name, detail)| json!({ "name": name, "detail": detail }))
            .collect();
        json!({
            "kind": if self.tools { "tools" } else { "text" },
            "text": self.text,
            "tools": tools,
        })
    }
}

struct Turn {
    assistant: bool,
    parts: Vec<Part>,
}

#[derive(Default)]
struct Turns(Vec<Turn>);

impl Turns {
    /// The assistant's parts: of its current turn, or a new one.
    fn assistant(&mut self) -> &mut Vec<Part> {
        if !self.0.last().is_some_and(|turn| turn.assistant) {
            self.0.push(Turn {
                assistant: true,
                parts: Vec::new(),
            });
        }
        let last = self.0.len() - 1;
        &mut self.0[last].parts
    }

    /// The tool calls of the assistant's current group, or a new group.
    fn tools(&mut self) -> &mut Vec<(Value, Value)> {
        let parts = self.assistant();
        if !parts.last().is_some_and(|part| part.tools) {
            parts.push(Part {
                tools: true,
                text: Value::String(String::new()),
                calls: Vec::new(),
            });
        }
        let last = parts.len() - 1;
        &mut parts[last].calls
    }

    fn call(&mut self, name: Value, detail: Value) {
        let tools = self.tools();
        if tools.len() < TOOLS {
            tools.push((name, detail));
        }
    }
}

/// A tool call's detail: the first of its usual inputs, on one line.
pub(crate) fn tool_detail(params: Option<&Value>, cwd: &str) -> Value {
    let Some(params) = params.and_then(Value::as_object) else {
        return Value::String(String::new());
    };
    let detail = [
        "description",
        "command",
        "file_path",
        "path",
        "pattern",
        "url",
        "query",
        "prompt",
    ]
    .iter()
    .find_map(|key| params.get(*key).filter(|value| !value.is_null()));
    match detail {
        Some(number @ Value::Number(_)) => one_line(number),
        Some(Value::String(text)) => one_line(&Value::String(strip_cwd(text, cwd).to_owned())),
        _ => Value::String(String::new()),
    }
}

/// A large tool call's name, without decoding it.
fn large_name(line: &[u8]) -> Option<String> {
    const START: &[u8] = br#""type":"tool_use","id":""#;
    const NAME: &[u8] = br#"","name":""#;
    let mut from = 0;
    while let Some(at) = find(line, START, from) {
        from = at + 1;
        let id = at + START.len();
        let Some(end) = line[id..].iter().position(|&byte| byte == b'"') else {
            continue;
        };
        let name = id + end + NAME.len();
        if !line[id + end..].starts_with(NAME) {
            continue;
        }
        let Some(length) = line[name..].iter().position(|&byte| byte == b'"') else {
            continue;
        };
        if length > 0 {
            // Latin-1, as the Python provider read it.
            return Some(
                line[name..name + length]
                    .iter()
                    .map(|&b| char::from(b))
                    .collect(),
            );
        }
    }
    None
}

/// A large tool call's detail, from its first description, file or command.
fn large_detail(line: &[u8]) -> Option<String> {
    let start = [
        &br#""description":""#[..],
        br#""file_path":""#,
        br#""command":""#,
    ]
    .iter()
    .filter_map(|key| find(line, key, 0).map(|at| at + key.len()))
    .min()?;
    let (end, _) = escaped(line, start, LARGE_DETAIL);
    unescape(&line[start..end])
}

/// An assistant's entry: its text and tool calls.
fn assistant(turns: &mut Turns, message: &Map<String, Value>, cwd: &str) {
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    turns.assistant();
    for block in content.iter().filter_map(Value::as_object) {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = block.get("text").unwrap_or(&Value::Null);
                if visible(&as_text(text)) {
                    turns.assistant().push(Part::text(clip(text, TEXT)));
                }
            },
            Some("tool_use") => {
                let name = block
                    .get("name")
                    .filter(|name| !name.is_null())
                    .cloned()
                    .unwrap_or_else(|| Value::String("Tool".to_owned()));
                turns.call(name, tool_detail(block.get("input"), cwd));
            },
            _ => {},
        }
    }
}

/// One line of the transcript.
fn line(turns: &mut Turns, line: &[u8], cwd: &str) {
    let user = contains(line, br#""type":"user""#);
    // Tool results are left out, and decoding them is the slow part.
    if user && contains(line, br#""tool_result""#) {
        return;
    }
    if !user && !contains(line, br#""type":"assistant""#) {
        return;
    }
    let is_assistant = contains(line, br#""role":"assistant""#);
    let text = contains(line, br#""type":"text""#);
    let tool = contains(line, br#""type":"tool_use""#);
    // Thinking alone shows nothing here.
    if is_assistant && contains(line, br#""type":"thinking""#) && !text && !tool {
        return;
    }
    // A large tool call (a whole file written): its name and a line from
    // its input are enough, without decoding it.
    if is_assistant
        && line.len() > LARGE
        && tool
        && !text
        && !contains(line, br#""isSidechain":true"#)
    {
        let name = large_name(line).unwrap_or_else(|| "Tool".to_owned());
        let detail = large_detail(line).unwrap_or_default();
        let detail = one_line(&Value::String(strip_cwd(&detail, cwd).to_owned()));
        turns.call(Value::String(name), detail);
        return;
    }
    let Some(entry) = decode_object(line) else {
        return;
    };
    if truthy(entry.get("isSidechain")) {
        return;
    }
    match entry.get("type").and_then(Value::as_str) {
        Some("user") => {
            if let Some(text) = typed(&entry) {
                turns.0.push(Turn {
                    assistant: false,
                    parts: vec![Part::text(clip(&text, TEXT))],
                });
            }
        },
        Some("assistant") => {
            if let Some(message) = entry.get("message").and_then(Value::as_object) {
                assistant(turns, message, cwd);
            }
        },
        _ => {},
    }
}

/// The session's transcript, its latest [`TURNS`]; `None` when it can't be
/// read.
pub(crate) fn read(session: &Session, group: &Group, stopped: &AtomicBool) -> Option<Value> {
    let file = File::open(session.path.as_ref()?).ok()?;
    let mut reader = BufReader::new(file);
    let mut turns = Turns::default();
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(_) => line(&mut turns, &buffer, &group.path),
        }
        if stopped.load(Ordering::Relaxed) {
            return None;
        }
    }
    let mut turns: Vec<Turn> = turns
        .0
        .into_iter()
        .filter(|turn| !turn.parts.is_empty())
        .collect();
    let hidden = turns.len().saturating_sub(TURNS);
    turns.drain(..hidden);
    let turns: Vec<Value> = turns
        .iter()
        .map(|turn| {
            json!({
                "from": if turn.assistant { "assistant" } else { "user" },
                "parts": turn.parts.iter().map(Part::json).collect::<Vec<_>>(),
            })
        })
        .collect();
    Some(json!({
        "id": session.id,
        "title": session.title,
        "project": group.name,
        "branch": session.branch,
        "hidden": hidden,
        "turns": turns,
    }))
}
