//! Small helpers for the JSON everything here passes around.

use {
    serde_json::Value,
    std::{
        sync::{Mutex, MutexGuard, PoisonError},
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// A list field of a message. The host only sends a Luau table as a list
/// when it has entries: an empty one arrives as `{}`, so anything that may
/// be empty goes through here.
pub(crate) fn as_list(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

/// A message's content as plain text: a string, or its text blocks.
pub(crate) fn text_of(content: &Value) -> String {
    if let Some(text) = content.as_str() {
        return text.to_owned();
    }
    let blocks = as_list(content)
        .iter()
        .filter(|block| block["type"] == "text")
        .map(|block| block["text"].as_str().unwrap_or_default());
    blocks.collect::<Vec<_>>().join("\n")
}

/// `value` as text, nothing for null: strings as they are, others as JSON.
pub(crate) fn display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The first `limit` characters of `text`.
pub(crate) fn first_chars(text: &str, limit: usize) -> &str {
    match text.char_indices().nth(limit) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

/// The last `limit` bytes of `text`, starting on a character.
pub(crate) fn last_bytes(text: &str, limit: usize) -> &str {
    let mut start = text.len().saturating_sub(limit);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// `text` cut to `limit` characters, the last being an ellipsis.
pub(crate) fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() > limit {
        let kept = first_chars(text, limit.saturating_sub(1));
        format!("{kept}…")
    } else {
        text.to_owned()
    }
}

/// Runs of whitespace as one space, and none at the ends.
pub(crate) fn squeezed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The time, in Unix seconds.
pub(crate) fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

/// The time, in Unix milliseconds.
pub(crate) fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// The lock, even if a thread panicked holding it: what it guards stays
/// usable, as a provider must outlive one bad message.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn an_empty_table_is_an_empty_list() {
        assert!(as_list(&json!({})).is_empty());
        assert_eq!(as_list(&json!(["a"])).len(), 1);
    }

    #[test]
    fn text_comes_from_strings_and_blocks() {
        assert_eq!(text_of(&json!("hi")), "hi");
        let blocks = json!([
            { "type": "text", "text": "a" },
            { "type": "image" },
            { "type": "text", "text": "b" },
        ]);
        assert_eq!(text_of(&blocks), "a\nb");
        assert_eq!(text_of(&Value::Null), "");
    }

    #[test]
    fn cuts_land_on_characters() {
        assert_eq!(clip("héllo", 3), "hé…");
        assert_eq!(clip("hi", 3), "hi");
        assert_eq!(last_bytes("éa", 2), "a");
    }
}
