//! [`Data`] as JSON, by hand: core has no serde. Maps keep their sorted
//! keys, so the same schema always writes the same text.

use {crate::Data, std::fmt::Write};

pub(super) fn to_json(data: &Data) -> String {
    let mut out = String::new();
    write(data, &mut out);
    out
}

fn write(data: &Data, out: &mut String) {
    match data {
        Data::Nil => out.push_str("null"),
        Data::Bool(value) => out.push_str(if *value {
            "true"
        } else {
            "false"
        }),
        Data::Integer(value) => {
            let _ = write!(out, "{value}");
        },
        Data::Number(value) if value.is_finite() => {
            let _ = write!(out, "{value}");
        },
        Data::Number(_) => out.push_str("null"),
        Data::String(text) => string(text, out),
        Data::List(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write(item, out);
            }
            out.push(']');
        },
        Data::Map(entries) => {
            out.push('{');
            for (index, (key, value)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                string(key, out);
                out.push(':');
                write(value, out);
            }
            out.push('}');
        },
    }
}

fn string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            },
            c => out.push(c),
        }
    }
    out.push('"');
}
