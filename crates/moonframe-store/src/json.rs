//! Saved values as JSON text. Maps keep their string keys; lists stay lists.
//! A value that will not parse reads as nothing, so a damaged row loses one
//! state rather than the app.

use {moonframe_core::Data, serde_json::Value};

pub(crate) fn to_text(data: &Data) -> String {
    to_json(data).to_string()
}

pub(crate) fn from_text(text: &str) -> Option<Data> {
    serde_json::from_str::<Value>(text)
        .inspect_err(|error| tracing::warn!("a saved value does not parse: {error}"))
        .ok()
        .map(|value| from_json(&value))
}

fn to_json(data: &Data) -> Value {
    match data {
        Data::Nil => Value::Null,
        Data::Bool(value) => Value::Bool(*value),
        Data::Integer(value) => Value::from(*value),
        // JSON has no NaN or infinity; they save as nothing.
        Data::Number(value) => {
            serde_json::Number::from_f64(*value).map_or(Value::Null, Value::Number)
        },
        Data::String(text) => Value::String(text.clone()),
        Data::List(items) => Value::Array(items.iter().map(to_json).collect()),
        Data::Map(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), to_json(value)))
                .collect(),
        ),
    }
}

fn from_json(value: &Value) -> Data {
    match value {
        Value::Null => Data::Nil,
        Value::Bool(value) => Data::Bool(*value),
        Value::Number(number) => number
            .as_i64()
            .map(Data::Integer)
            .or_else(|| number.as_f64().map(Data::Number))
            .unwrap_or(Data::Nil),
        Value::String(text) => Data::String(text.clone()),
        Value::Array(items) => Data::List(items.iter().map(from_json).collect()),
        Value::Object(fields) => Data::Map(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), from_json(value)))
                .collect(),
        ),
    }
}
