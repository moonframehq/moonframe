//! A row's or column's `scroll_to`: a direct child's place, a key, or how
//! far it is scrolled.

use {super::*, layout::PlainId};

/// How far a stack may be asked to scroll, in points.
const AT: std::ops::RangeInclusive<f32> = 0.0..=1.0e9;

/// `scroll_to = 12` (direct child 12), `scroll_to = "row-7"` (the first row
/// or column with that key, however deep), `scroll_to = { key = "row-7",
/// seq = n }`, which a new `seq` asks again, or `{ y = 480, seq = n }` (and
/// `x`), back where `measure` told it was.
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
                let fields = Fields::read(value, cx, &[&["key", "x", "y", "seq"]])?;
                let seq = fields.get("seq")?.unwrap_or(Data::Nil);
                let (x, y) = (fields.number("x", AT)?, fields.number("y", AT)?);
                match fields.get::<PlainId>("key")? {
                    Some(_) if x.is_some() || y.is_some() => {
                        Err(cx.fail("give `key`, or `x` and `y`, not both"))
                    },
                    Some(key) => Ok(Self::Key { key: key.0, seq }),
                    None if x.is_some() || y.is_some() => Ok(Self::At { x, y, seq }),
                    None => Err(cx.fail(
                        "`scroll_to = { ... }` takes the `key` of a row or column to reveal, or the `y` (and `x`) to scroll to",
                    )),
                }
            },
            _ => {
                Err(cx
                    .fail("expected a child index, a key, or `{ key = \"...\" | y = n, seq = n }`"))
            },
        }
    }
}
