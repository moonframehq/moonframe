//! Where a `ui.list` scrolls besides the wheel: to a row or a `y` its
//! `scroll_to` asks for, once each time that changes, and, while scrolled
//! down, along with the row it showed first, so rows arriving above do not
//! push what is being read away. At the top, arrivals join the top.
//!
//! The row followed is told by its `key` (the row's own `ui.row { key }`):
//! found again among the rows the view built, or else, when rows were added
//! and it moved past them, as far down as there are rows more.

use {
    super::super::{Painter, Place},
    gpui_kit::*,
    moonframe_core::{List, ListScroll, ListScrollTo, Node, RowHeights},
    std::{
        cell::RefCell,
        collections::HashMap,
        time::{Duration, Instant},
    },
};

/// How long a list not drawn is remembered.
const FORGET: Duration = Duration::from_secs(60);
/// Less than this is layout's rounding, not a scroll.
const SLACK: f32 = 0.5;

thread_local! {
    static KEPT: RefCell<HashMap<String, Kept>> = RefCell::new(HashMap::new());
}

#[derive(Default)]
struct Kept {
    /// The last `scroll_to`, so a redraw does not undo the wheel.
    asked: Option<ListScrollTo>,
    /// Asked for, and waiting for the list's height.
    pending: Option<ListScroll>,
    anchor: Option<Anchor>,
    seen: Option<Instant>,
}

/// The row a list showed first, while scrolled down.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Anchor {
    pub key: String,
    /// Its place among `count` rows then, and its top in the content.
    pub index: usize,
    pub top: f32,
    pub count: usize,
}

/// The list's rows as the view built them: from `start`, each one's key.
pub(super) struct Built<'a> {
    pub start: usize,
    pub keys: Vec<Option<&'a str>>,
}

impl<'a> Built<'a> {
    fn of(list: &'a List) -> Self {
        let keys = list
            .rows
            .iter()
            .map(|row| match row {
                Node::Stack(stack) => stack.key.as_deref(),
                _ => None,
            })
            .collect();
        Self {
            start: list.start,
            keys,
        }
    }

    fn key(&self, index: usize) -> Option<&'a str> {
        self.keys
            .get(index.checked_sub(self.start)?)
            .copied()
            .flatten()
    }

    fn find(&self, key: &str) -> Option<usize> {
        self.keys
            .iter()
            .position(|found| *found == Some(key))
            .map(|at| self.start + at)
    }
}

/// How far down `list` shows this frame, its `handle` moved there: last
/// frame's place, kept on its first row as rows arrive above, then where a
/// new `scroll_to` asks.
pub(super) fn place(
    list: &List,
    handle: &ScrollHandle,
    painter: &Painter<'_, '_>,
    place: &Place<'_>,
) -> f32 {
    let key = format!("{:?}/{}", painter.window, place.path);
    let now = Instant::now();
    let heights = &list.row_height;
    let built = Built::of(list);
    let height = f32::from(handle.bounds().size.height);
    let was = (-f32::from(handle.offset().y)).max(0.0);
    let scrolled = KEPT.with_borrow_mut(|kept| {
        kept.retain(|_, entry| {
            entry
                .seen
                .is_none_or(|seen| now.saturating_duration_since(seen) < FORGET)
        });
        let entry = kept.entry(key).or_default();
        entry.seen = Some(now);
        let mut scrolled = entry
            .anchor
            .as_ref()
            .filter(|_| was > SLACK)
            .and_then(|anchor| anchored(anchor, was, &built, heights, list.count))
            .unwrap_or(was);
        if entry.asked != list.scroll_to {
            entry.pending = list.scroll_to.as_ref().map(|asked| asked.to);
            entry.asked = list.scroll_to.clone();
        }
        let known = (height > 0.0).then_some(height);
        if let Some(to) = entry.pending
            && (known.is_some() || matches!(to, ListScroll::Y(_)))
        {
            scrolled = target(to, scrolled, known, heights, list.count);
            entry.pending = None;
        }
        if let Some(height) = known {
            scrolled = scrolled.min((heights.total(list.count) - height).max(0.0));
        }
        entry.anchor = anchor(scrolled, &built, heights, list.count);
        scrolled
    });
    if (scrolled - was).abs() > SLACK {
        handle.set_offset(point(handle.offset().x, px(-scrolled)));
    }
    scrolled
}

/// How far down to show `to`, from `scrolled`, in a box `height` high
/// when known: a row comes into view only as far as needed (its top when it
/// is taller than the box), a `y` as it is.
pub(super) fn target(
    to: ListScroll,
    scrolled: f32,
    height: Option<f32>,
    heights: &RowHeights,
    count: usize,
) -> f32 {
    match to {
        ListScroll::Y(y) => y,
        ListScroll::Row(row) => {
            let index = row.min(count).saturating_sub(1);
            if count == 0 {
                return scrolled;
            }
            let (top, bottom) = (
                heights.top(index),
                heights.top(index) + heights.height(index),
            );
            let height = height.unwrap_or(0.0);
            if top < scrolled {
                top
            } else if bottom > scrolled + height {
                (bottom - height).min(top)
            } else {
                scrolled
            }
        },
    }
}

/// Where `anchor`'s row is now, scrolled to keep it as far from the box's
/// top as it was at `scrolled`; `None` when it stayed or is not found.
pub(super) fn anchored(
    anchor: &Anchor,
    scrolled: f32,
    built: &Built<'_>,
    heights: &RowHeights,
    count: usize,
) -> Option<f32> {
    let index = built.find(&anchor.key).or_else(|| {
        let more = count.checked_sub(anchor.count).filter(|more| *more > 0)?;
        Some(anchor.index + more).filter(|index| *index < count)
    })?;
    let moved = heights.top(index) - anchor.top;
    (moved.abs() > SLACK).then(|| (scrolled + moved).max(0.0))
}

/// The row to follow from `scrolled`: the first that shows, if it has a
/// key; none at the top, where arrivals join it.
pub(super) fn anchor(
    scrolled: f32,
    built: &Built<'_>,
    heights: &RowHeights,
    count: usize,
) -> Option<Anchor> {
    if scrolled <= SLACK || count == 0 {
        return None;
    }
    let index = heights.at(scrolled + SLACK, count).min(count - 1);
    Some(Anchor {
        key: built.key(index)?.to_owned(),
        index,
        top: heights.top(index),
        count,
    })
}

#[cfg(test)]
mod tests;
