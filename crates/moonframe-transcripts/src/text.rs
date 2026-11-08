//! Text as the transcripts hold it: JSON values shown as text, titles on one
//! line, and the few byte patterns searched without decoding a line.
//!
//! Whitespace is `char::is_whitespace`: Unicode's White_Space, which is what
//! the Python provider used (its `\s` without `\x1c` to `\x1f`).

use serde_json::{Map, Number, Value, json};

/// A JSON value from bytes, or `None` when it doesn't parse.
pub(crate) fn decode(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice(bytes).ok()
}

/// A JSON object from bytes, or `None`.
pub(crate) fn decode_object(bytes: &[u8]) -> Option<Map<String, Value>> {
    match decode(bytes)? {
        Value::Object(object) => Some(object),
        _ => None,
    }
}

/// A JSON scalar as text: whole numbers without ".0", nothing for null.
pub(crate) fn as_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(yes) => if *yes {
            "1"
        } else {
            "0"
        }
        .to_owned(),
        Value::Number(number) => match whole(number) {
            Some(float) => format!("{float:.0}"),
            None => number.to_string(),
        },
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// A float with no fraction, which Python shows as the integer it is.
fn whole(number: &Number) -> Option<f64> {
    let float = number.as_f64().filter(|_| number.is_f64())?;
    // `-0.0` is 0 too.
    (float.is_finite() && float.fract() == 0.0).then_some(float + 0.0)
}

/// True unless absent, null, false, 0, "" or "0".
pub(crate) fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(yes)) => *yes,
        Some(Value::Number(number)) => number.as_f64() != Some(0.0),
        Some(Value::String(text)) => !text.is_empty() && text != "0",
        Some(_) => true,
    }
}

/// At most `most` characters, the last an ellipsis when cut. A number short
/// enough is shared as the number it was.
pub(crate) fn clip(value: &Value, most: usize) -> Value {
    let shown = as_text(value);
    if shown.chars().count() > most {
        let mut cut: String = shown.chars().take(most - 1).collect();
        cut.push('…');
        return Value::String(cut);
    }
    match value {
        Value::Number(number) => match whole(number) {
            #[allow(clippy::cast_possible_truncation)]
            Some(float) if float.abs() < 9.0e15 => json!(float as i64),
            _ => value.clone(),
        },
        _ => Value::String(shown),
    }
}

/// On one line: leading whitespace gone, each run of it one space, at most
/// 90 characters.
pub(crate) fn one_line(value: &Value) -> Value {
    if value.is_number() {
        return clip(value, 90);
    }
    let mut line = String::new();
    let mut space = false;
    for c in as_text(value).trim_start().chars() {
        if c.is_whitespace() {
            if !space {
                line.push(' ');
            }
            space = true;
        } else {
            line.push(c);
            space = false;
        }
    }
    clip(&Value::String(line), 90)
}

/// Whether `text` shows anything.
pub(crate) fn visible(text: &str) -> bool {
    text.chars().any(|c| !c.is_whitespace())
}

/// The text a person typed, or `None` for tool results, commands and
/// reminders.
pub(crate) fn typed(entry: &Map<String, Value>) -> Option<Value> {
    if truthy(entry.get("isSidechain")) || truthy(entry.get("isMeta")) {
        return None;
    }
    let content = entry
        .get("message")
        .and_then(Value::as_object)
        .and_then(|message| message.get("content"));
    let mut text = None;
    match content {
        Some(Value::Array(blocks)) => {
            for block in blocks.iter().filter_map(Value::as_object) {
                match block.get("type").and_then(Value::as_str) {
                    Some("tool_result") => return None,
                    Some("text") => {
                        text = block.get("text");
                        break;
                    },
                    _ => {},
                }
            }
        },
        None | Some(Value::Object(_) | Value::Bool(_)) => {},
        Some(other) => text = Some(other),
    }
    let text = text.filter(|text| matches!(text, Value::String(_) | Value::Number(_)))?;
    let shown = as_text(text);
    let markup = shown.trim_start();
    if !visible(&shown) || markup.starts_with('<') || markup.starts_with("Caveat:") {
        return None;
    }
    Some(text.clone())
}

/// The last part of a path, trailing slashes aside.
pub(crate) fn basename(path: &str) -> &str {
    let stripped = path.trim_end_matches('/');
    if stripped.is_empty() {
        return if path.is_empty() {
            ""
        } else {
            "/"
        };
    }
    stripped.rsplit('/').next().unwrap_or(stripped)
}

/// Where a session ran, as a repository name: worktrees go under their repo.
/// Without a folder, the name the CLI gave the session's folder, after its
/// last `-`.
pub(crate) fn project_of(cwd: Option<&str>, folder: &str) -> String {
    if let Some(cwd) = cwd.filter(|cwd| !cwd.is_empty()) {
        const HERDR: &str = "/.herdr/worktrees/";
        for (at, _) in cwd.match_indices(HERDR) {
            let rest = &cwd[at + HERDR.len()..];
            if let Some(end) = rest.find('/').filter(|&end| end > 0) {
                return rest[..end].to_owned();
            }
        }
        const CLAUDE: &str = "/.claude/worktrees/";
        let mut repo = cwd;
        for (at, _) in cwd.match_indices(CLAUDE) {
            let rest = &cwd[at + CLAUDE.len()..];
            let name = rest.strip_suffix('/').unwrap_or(rest);
            if !name.is_empty() && !name.contains('/') {
                repo = &cwd[..at];
                break;
            }
        }
        return basename(repo).to_owned();
    }
    let name = basename(folder);
    let name = name.rfind('-').map_or(name, |at| &name[at + 1..]);
    if name.is_empty() || name == "0" {
        "unknown".to_owned()
    } else {
        name.to_owned()
    }
}

/// How long ago, shortly: "now", "5m", "3h", "2d", "4mo".
pub(crate) fn age(mtime: i64, now: i64) -> String {
    let seconds = now - mtime;
    if seconds < 60 {
        "now".to_owned()
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h", seconds / 3600)
    } else if seconds < 86400 * 30 {
        format!("{}d", seconds / 86400)
    } else {
        format!("{}mo", seconds / (86400 * 30))
    }
}

/// `text` without `cwd/` in front.
pub(crate) fn strip_cwd<'a>(text: &'a str, cwd: &str) -> &'a str {
    if cwd.is_empty() {
        return text;
    }
    text.strip_prefix(cwd)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(text)
}

/// A JSON string's value from its escaped bytes.
pub(crate) fn unescape(escaped: &[u8]) -> Option<String> {
    let mut quoted = Vec::with_capacity(escaped.len() + 2);
    quoted.push(b'"');
    quoted.extend_from_slice(escaped);
    quoted.push(b'"');
    match decode(&quoted)? {
        Value::String(text) => Some(text),
        _ => None,
    }
}

/// Where `needle` first is in `hay`, from `from` on.
pub(crate) fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    let first = *needle.first()?;
    let mut at = from;
    while at + needle.len() <= hay.len() {
        let next = at + hay[at..].iter().position(|&byte| byte == first)?;
        if hay[next..].starts_with(needle) {
            return Some(next);
        }
        at = next + 1;
    }
    None
}

/// Whether `hay` holds `needle`.
pub(crate) fn contains(hay: &[u8], needle: &[u8]) -> bool {
    find(hay, needle, 0).is_some()
}

/// Where `needle` last is in `hay`.
pub(crate) fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    let last = hay.len().checked_sub(needle.len())?;
    (0..=last).rev().find(|&at| hay[at..].starts_with(needle))
}

/// The line of `data` holding position `at`, from `at` on.
pub(crate) fn line_at(data: &[u8], at: usize) -> &[u8] {
    let end = find(data, b"\n", at).unwrap_or(data.len());
    &data[at..end]
}

/// Escaped string characters from `from` on, at most `most` of them (an
/// escape counts once), up to a quote: `(?:[^"\\]|\\.){0,most}`. Where they
/// end, and whether a quote follows.
pub(crate) fn escaped(hay: &[u8], from: usize, most: usize) -> (usize, bool) {
    let mut at = from;
    for _ in 0..most {
        match hay.get(at) {
            None | Some(b'"') => break,
            Some(b'\\') => match hay.get(at + 1) {
                None | Some(b'\n') => break,
                Some(_) => at += 2,
            },
            Some(_) => at += 1,
        }
    }
    (at, hay.get(at) == Some(&b'"'))
}

/// The first `"<key>":"<string>"` in `hay`, its string's value.
pub(crate) fn quoted(hay: &[u8], key: &[u8]) -> Option<String> {
    let mut from = 0;
    while let Some(at) = find(hay, key, from) {
        let start = at + key.len();
        let (end, closed) = escaped(hay, start, usize::MAX);
        if closed {
            return unescape(&hay[start..end]);
        }
        from = at + 1;
    }
    None
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
