//! A transcript as views read it: herdr's own shape (see
//! herdr-chat-protocol's `Transcript`), trimmed to what a view can hold.
//! The latest entries are kept, long texts are cut from the front (an
//! answer's end is what is being read), pictures lose their bytes, and a
//! file change keeps its path and line counts, not both whole files.

use {
    herdr_chat_protocol::Transcript,
    serde_json::{Map, Value, json},
};

/// Entries shared, from the end, of the chat in view and of the others;
/// `hidden` counts those left out.
pub(crate) const FULL_ENTRIES: usize = 160;
pub(crate) const SHORT_ENTRIES: usize = 6;
/// The agent's answers and thoughts, in bytes: markdown views take a lot.
const MAX_ANSWER: usize = 256 * 1024;
/// Anything else that is text: what one string in a view may hold.
const MAX_TEXT: usize = 15 * 1024;
/// A tool's own output, in bytes.
const MAX_TOOL_TEXT: usize = 4 * 1024;
/// Slash commands shared, for completing `/`.
const MAX_COMMANDS: usize = 200;

pub(crate) fn transcript(transcript: &Transcript, max_entries: usize) -> Value {
    let Ok(Value::Object(mut fields)) = serde_json::to_value(transcript) else {
        return Value::Null;
    };
    if let Some(Value::Array(entries)) = fields.get_mut("entries") {
        let hidden = entries.len().saturating_sub(max_entries);
        entries.drain(..hidden);
        for entry in entries.iter_mut() {
            if let Some(Value::Object(kind)) = entry.get_mut("kind") {
                trim_entry(kind);
            }
        }
        fields.insert("hidden".to_owned(), json!(hidden));
    }
    if let Some(Value::Array(commands)) = fields.get_mut("commands") {
        commands.truncate(MAX_COMMANDS);
    }
    for key in ["queued", "permissions", "plan"] {
        if let Some(Value::Array(items)) = fields.get_mut(key) {
            for item in items {
                cut_strings(item, MAX_TEXT);
            }
        }
    }
    Value::Object(fields)
}

fn trim_entry(kind: &mut Map<String, Value>) {
    match kind.get("type").and_then(Value::as_str) {
        Some("agent" | "thought") => cut_field(kind, "text", MAX_ANSWER),
        Some("image") => {
            if let Some(Value::Object(image)) = kind.get_mut("image") {
                image.remove("data");
            }
        },
        Some("tool") => {
            cut_field(kind, "title", MAX_TEXT);
            if let Some(Value::Array(content)) = kind.get_mut("content") {
                for part in content {
                    if let Value::Object(part) = part {
                        trim_tool_content(part);
                    }
                }
            }
        },
        _ => {
            for (_, value) in kind.iter_mut() {
                cut_strings(value, MAX_TEXT);
            }
        },
    }
}

fn trim_tool_content(part: &mut Map<String, Value>) {
    match part.get("type").and_then(Value::as_str) {
        Some("text") => cut_field(part, "text", MAX_TOOL_TEXT),
        Some("diff") => {
            let count = |text: Option<&Value>| {
                text.and_then(Value::as_str)
                    .map_or(0, |text| text.lines().count())
            };
            let new = count(part.get("new"));
            let old = count(part.get("old"));
            part.remove("new");
            part.remove("old");
            part.insert("new_lines".to_owned(), json!(new));
            part.insert("old_lines".to_owned(), json!(old));
        },
        Some("image") => {
            if let Some(Value::Object(image)) = part.get_mut("image") {
                image.remove("data");
            }
        },
        _ => {},
    }
}

fn cut_field(fields: &mut Map<String, Value>, key: &str, max: usize) {
    if let Some(value) = fields.get_mut(key) {
        cut_strings(value, max);
    }
}

/// Cuts every string in `value` to its last `max` bytes.
fn cut_strings(value: &mut Value, max: usize) {
    match value {
        Value::String(text) if text.len() > max => {
            let mut cut = text.len() - max;
            while !text.is_char_boundary(cut) {
                cut += 1;
            }
            text.replace_range(..cut, "…");
        },
        Value::Array(items) => items.iter_mut().for_each(|item| cut_strings(item, max)),
        Value::Object(fields) => fields.values_mut().for_each(|item| cut_strings(item, max)),
        _ => {},
    }
}
