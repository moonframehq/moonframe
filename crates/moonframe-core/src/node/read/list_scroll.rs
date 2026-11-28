//! A `ui.list`'s `scroll_to`: a row, or a `y` to go back to.

use super::*;

/// How far down a list may be: millions of rows, each up to many pixels.
const Y: std::ops::RangeInclusive<f32> = 0.0..=1.0e10;

/// `scroll_to = 12`, `{ row = 12, seq = n }` or `{ y = 480, seq = n }`.
impl Read for ListScrollTo {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let row = |value: &Value, cx: &Cx| match i64::read(value, cx)? {
            row if row >= 1 => Ok(ListScroll::Row(row as usize)),
            _ => Err(cx.fail("expected a row from 1")),
        };
        match value {
            Value::Integer(_) | Value::Number(_) => Ok(Self {
                to: row(value, cx)?,
                seq: Data::Nil,
            }),
            Value::Table(_) => {
                let fields = Fields::read(value, cx, &[&["row", "y", "seq"]])?;
                let to = match (fields.raw("row"), fields.number("y", Y)?) {
                    (Some(value), None) => row(value, &cx.key("row"))?,
                    (None, Some(y)) => ListScroll::Y(y),
                    (Some(_), Some(_)) => {
                        return Err(cx.fail("give `row` or `y`, not both"));
                    },
                    (None, None) => {
                        return Err(cx.fail("`scroll_to` takes `row` (from 1) or `y` (pixels)"));
                    },
                };
                Ok(Self {
                    to,
                    seq: fields.get("seq")?.unwrap_or(Data::Nil),
                })
            },
            _ => Err(cx.fail("expected a row, `{ row = n, seq = s }` or `{ y = px, seq = s }`")),
        }
    }
}
