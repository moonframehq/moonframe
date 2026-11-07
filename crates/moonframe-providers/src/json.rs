//! JSON from providers into [`Data`], bounded like any value a module hands
//! over, and messages back out as JSON.

use {
    moonframe_core::{Data, Error},
    serde_json::{Map, Number, Value},
    std::collections::BTreeMap,
};

/// Longer lines from a provider are refused.
pub(crate) const MAX_JSON: usize = 1024 * 1024;
/// The most values one publish holds, counting every list, table and field.
pub const MAX_ENTRIES: usize = 10_000;
const MAX_DEPTH: usize = 16;

pub(crate) fn data(text: &str) -> Result<Data, Error> {
    if text.len() > MAX_JSON {
        return Err(Error::new(format!("published more than {MAX_JSON} bytes")));
    }
    let value: Value = serde_json::from_str(text)
        .map_err(|error| Error::new(format!("published invalid JSON: {error}")))?;
    let mut entries = 0;
    convert(&value, 0, &mut entries)
}

fn convert(value: &Value, depth: usize, entries: &mut usize) -> Result<Data, Error> {
    *entries += 1;
    if *entries > MAX_ENTRIES {
        return Err(Error::new(format!(
            "published more than {MAX_ENTRIES} values"
        )));
    }
    if depth > MAX_DEPTH {
        return Err(Error::new(format!(
            "published values nested deeper than {MAX_DEPTH}"
        )));
    }
    Ok(match value {
        Value::Null => Data::Nil,
        Value::Bool(value) => Data::Bool(*value),
        Value::Number(number) => match number.as_i64() {
            Some(integer) => Data::Integer(integer),
            None => Data::Number(number.as_f64().unwrap_or(f64::NAN)),
        },
        Value::String(text) => Data::String(text.clone()),
        Value::Array(items) => Data::List(
            items
                .iter()
                .map(|item| convert(item, depth + 1, entries))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(fields) => Data::Map(
            fields
                .iter()
                .map(|(key, value)| Ok((key.clone(), convert(value, depth + 1, entries)?)))
                .collect::<Result<BTreeMap<_, _>, Error>>()?,
        ),
    })
}

/// JSON for `data`, as providers and the saved document hold it.
pub fn to_json(data: &Data) -> Value {
    json(data)
}

/// `value` as [`Data`], bounded like a provider's.
pub fn from_json(value: &Value) -> Result<Data, Error> {
    let mut entries = 0;
    convert(value, 0, &mut entries)
}

pub(crate) fn json(data: &Data) -> Value {
    match data {
        Data::Nil => Value::Null,
        Data::Bool(value) => Value::Bool(*value),
        Data::Integer(value) => Value::Number((*value).into()),
        Data::Number(value) => Number::from_f64(*value).map_or(Value::Null, Value::Number),
        Data::String(text) => Value::String(text.clone()),
        Data::List(items) => Value::Array(items.iter().map(json).collect()),
        Data::Map(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), json(value)))
                .collect::<Map<_, _>>(),
        ),
    }
}
