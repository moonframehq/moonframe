//! Where a `ui.list` is asked to scroll.

use crate::Data;

/// A list's `scroll_to`, acted on when it changes: a new `seq` asks again
/// for the same place.
#[derive(Clone, Debug, PartialEq)]
pub struct ListScrollTo {
    pub to: ListScroll,
    pub seq: Data,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ListScroll {
    /// Reveal this row (one-based), scrolling only as far as needed.
    Row(usize),
    /// Scroll this many pixels down: a `y` the list's `range` told, to
    /// put it back where it was.
    Y(f32),
}
