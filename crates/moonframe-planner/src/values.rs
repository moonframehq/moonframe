//! Messages come from Luau as JSON, so a value may be a string, number or
//! boolean where a string is meant; these read any of them, as the Python
//! provider did.

use serde_json::Value;

/// A value as text: `true` is "1", a whole float has no ".0", and a list or
/// table matches no id.
pub(crate) fn text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(true) => "1".to_owned(),
        Value::Bool(false) => "0".to_owned(),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                integer.to_string()
            } else if let Some(integer) = number.as_u64() {
                integer.to_string()
            } else {
                general(number.as_f64().unwrap_or(0.0))
            }
        },
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => "\0ref".to_owned(),
    }
}

/// C's `%.15g`.
fn general(value: f64) -> String {
    const DIGITS: i32 = 15;
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0"
        } else {
            "0"
        }
        .to_owned();
    }
    let scientific = format!("{:.*e}", (DIGITS - 1) as usize, value);
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if !(-4..DIGITS).contains(&exponent) {
        let sign = if exponent < 0 {
            '-'
        } else {
            '+'
        };
        return format!("{}e{sign}{:02}", trim_zeros(mantissa), exponent.abs());
    }
    let decimals = usize::try_from(DIGITS - 1 - exponent).unwrap_or(0);
    trim_zeros(&format!("{value:.decimals$}")).to_owned()
}

fn trim_zeros(number: &str) -> &str {
    if number.contains('.') {
        number.trim_end_matches('0').trim_end_matches('.')
    } else {
        number
    }
}

/// Python's truth: "" and "0" are false, and any list or table is true.
pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(text) => !text.is_empty() && text != "0",
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// A whole number written any way (`1`, `"1"`, `1.0`, `true`), or `None`.
/// One too large saturates, which every date then treats as out of range.
pub(crate) fn integer(value: &Value) -> Option<i64> {
    if matches!(value, Value::Null | Value::Array(_) | Value::Object(_)) {
        return None;
    }
    let text = text(value);
    let text = text.strip_suffix('\n').unwrap_or(&text);
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let saturated = if text.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    };
    Some(text.parse().unwrap_or(saturated))
}

/// A field that is there and not null, as Python's `get` is not `None`.
pub(crate) fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key).filter(|value| !value.is_null())
}

/// A string or a number: `isinstance(v, (str, int, float))` without `bool`.
pub(crate) fn scalar(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_))
}
