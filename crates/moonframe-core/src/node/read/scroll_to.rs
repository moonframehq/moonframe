//! A row's or column's `scroll_to`: a direct child's place, or a key.

use {super::*, layout::PlainId};

/// `scroll_to = 12` (direct child 12), `scroll_to = "row-7"` (the first row
/// or column with that key, however deep) or `scroll_to = { key = "row-7",
/// seq = n }`, which a new `seq` asks again.
impl Read for ScrollTo {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Integer(_) | Value::Number(_) => match i64::read(value, cx)? {
                index if index >= 1 => Ok(Self::Child(index as usize)),
                _ => Err(cx.fail("expected a positive child index")),
            },
            Value::String(_) => Ok(Self::Key {
                key: PlainId::read(value, cx)?.0,
                seq: Data::Nil,
            }),
            Value::Table(_) => {
                let fields = Fields::read(value, cx, &[&["key", "seq"]])?;
                let key = fields.get::<PlainId>("key")?.ok_or_else(|| {
                    cx.key("key")
                        .fail("`scroll_to = { key = ... }` names the row or column to reveal")
                })?;
                Ok(Self::Key {
                    key: key.0,
                    seq: fields.get("seq")?.unwrap_or(Data::Nil),
                })
            },
            _ => Err(cx.fail("expected a child index, a key, or `{ key = \"...\", seq = n }`")),
        }
    }
}
