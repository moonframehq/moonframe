//! A row's or column's `at_end` and `scroll_end`.

use super::*;

/// The keys [`Ends`] reads.
pub(super) const ENDS: &[&str] = &["at_end", "scroll_end"];

impl Ends {
    /// Its ends, if it has `at_end` or `scroll_end`; they need `scroll`.
    pub(super) fn from_fields(fields: &Fields) -> Result<Option<Box<Self>>, Error> {
        let ends = Self {
            at_end: fields.get("at_end")?,
            scroll_end: fields.get("scroll_end")?,
        };
        if ends == Self::default() {
            return Ok(None);
        }
        if Frame::from_fields(fields)?.scroll == Scroll::No {
            let key = if ends.at_end.is_some() {
                "at_end"
            } else {
                "scroll_end"
            };
            return Err(fields.cx().key(key).fail(format!(
                "`{key}` is a scrolling row's or column's end; give it `scroll`"
            )));
        }
        Ok(Some(Box::new(ends)))
    }
}
