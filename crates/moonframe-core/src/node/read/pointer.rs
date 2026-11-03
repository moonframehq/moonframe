//! A row's or column's `pointer`, `wheel`, `drop`, `carry`, `accept`,
//! `hover`, `range` and `cursor`, and a frame's `area`.

use super::*;

/// The keys [`Pointer`] reads.
pub(super) const POINTER: &[&str] = &[
    "pointer", "wheel", "drop", "carry", "accept", "hover", "range", "cursor",
];

/// Most a range's ends can be, either way.
const LIMIT: std::ops::RangeInclusive<f32> = -1e12..=1e12;

impl Pointer {
    pub(super) fn from_fields(fields: &Fields) -> Result<Self, Error> {
        let pointer = Self {
            press: fields.get("pointer")?,
            wheel: fields.get("wheel")?,
            drop: fields.get("drop")?,
            carry: fields.get("carry")?,
            accept: fields.get("accept")?,
            to: fields.get("to")?,
            hover: fields.get("hover")?.unwrap_or(false),
            range: fields.get("range")?,
            cursor: fields.get("cursor")?,
        };
        if pointer.hover && pointer.press.is_none() {
            return Err(fields
                .cx()
                .key("hover")
                .fail("`hover` sends its moves as `pointer` messages; give `pointer` too"));
        }
        if pointer.range.is_some() && pointer.press.is_none() {
            return Err(fields
                .cx()
                .key("range")
                .fail("`range` adds a `value` to `pointer` messages; give `pointer` too"));
        }
        if pointer.carry.is_some() && pointer.press.is_some() {
            return Err(fields.cx().key("carry").fail(
                "`carry` and `pointer` both follow a press; a carried row hears its drop through \
                 the target's `accept`",
            ));
        }
        Ok(pointer)
    }

    /// Whether `to` has something to go with.
    pub(super) fn sends(fields: &Fields) -> bool {
        ["pointer", "wheel", "drop", "accept"]
            .iter()
            .any(|key| fields.raw(key).is_some())
    }
}

/// `range = { min = 0, max = 100, step = 1, value = 40, speed = 0.5, axis
/// = "x" }`: `min` and `max` needed.
impl Read for Range {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&[
            "min", "max", "step", "value", "speed", "axis",
        ]])?;
        let min = fields
            .number("min", LIMIT)?
            .ok_or_else(|| cx.key("min").fail("a range needs `min` and `max`"))?;
        let max = fields
            .number("max", LIMIT)?
            .ok_or_else(|| cx.key("max").fail("a range needs `min` and `max`"))?;
        let speed = fields.number("speed", LIMIT)?;
        let value = fields.number("value", LIMIT)?;
        if speed.is_some() && value.is_none() {
            return Err(cx
                .key("speed")
                .fail("a drag with `speed` moves from `value`; give the current `value`"));
        }
        Ok(Self {
            min,
            max,
            step: fields.number("step", 0.0..=1e12)?,
            value: value.unwrap_or(min),
            speed,
            axis: fields.get("axis")?.unwrap_or(Axis::X),
        })
    }
}

/// `area = { left, top, width, height }`, each a fraction of the parent.
impl Read for Area {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        const FRACTION: std::ops::RangeInclusive<f32> = -100.0..=100.0;
        let fields = Fields::read(value, cx, &[&["left", "top", "width", "height"]])?;
        Ok(Self {
            left: fields.number("left", FRACTION)?,
            top: fields.number("top", FRACTION)?,
            width: fields.number("width", 0.0..=100.0)?,
            height: fields.number("height", 0.0..=100.0)?,
        })
    }
}
