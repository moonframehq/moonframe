use super::*;

/// A scrolling row's or column's end, for a chat or log that offers to go
/// back to the latest line: it says when it leaves or reaches its end, and
/// scrolls there when asked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ends {
    /// Sent to the drawing mod, with `at_end` added, as the stack first
    /// lays out and each time it reaches or leaves its end.
    pub at_end: Option<Data>,
    /// Scroll to the end each time this value changes (a `{ seq = n }`
    /// asked twice is two requests).
    pub scroll_end: Option<Data>,
}
