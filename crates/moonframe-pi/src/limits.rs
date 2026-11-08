//! What the host takes in one publish, and cutting a value down to it. The
//! host ends a process provider whose line passes 1 MiB or 10,000 values,
//! and refuses a native publish past the same count; both providers here
//! stay a little under each.

use {
    crate::value::first_chars,
    serde_json::{Map, Value},
};

pub(crate) const MAX_BYTES: usize = 1_000_000;
pub(crate) const MAX_VALUES: usize = 9_000;

/// How many values the host would count in `value`.
pub(crate) fn count_values(value: &Value) -> usize {
    match value {
        Value::Array(items) => 1 + items.iter().map(count_values).sum::<usize>(),
        Value::Object(fields) => 1 + fields.values().map(count_values).sum::<usize>(),
        _ => 1,
    }
}

/// Whether the host takes `value`, whose JSON is `line`.
pub(crate) fn fits(line: &str, value: &Value) -> bool {
    line.len() <= MAX_BYTES && count_values(value) <= MAX_VALUES
}

/// The last resort when nothing else was enough: every list and text cut
/// down, every key kept, since views read them. Tighter until it fits.
pub(crate) fn trimmed(value: &Value, limit: usize, depth: usize) -> Value {
    match value {
        Value::String(text) if text.chars().count() > limit * 100 => {
            Value::String(format!("{}…", first_chars(text, limit * 100)))
        },
        Value::Array(_) | Value::Object(_) if depth > 12 => Value::Null,
        Value::Array(items) => Value::Array(
            items
                .iter()
                .take(limit)
                .map(|item| trimmed(item, limit, depth + 1))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, item)| (key.clone(), trimmed(item, limit, depth + 1)))
                .collect::<Map<_, _>>(),
        ),
        other => other.clone(),
    }
}
