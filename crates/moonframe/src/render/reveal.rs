//! A scrolling row or column's `scroll_to` by key: the first row or column
//! drawn inside it with that key, however deep, comes into view, scrolled
//! only as far as needed, once it is drawn.
//!
//! While a key is asked for, the scrolling stack is what its descendants
//! draw inside ([`Painter::reveal`]); the first with the key draws a mark
//! that records where it is, and the stack, which lays out after its
//! children, scrolls to it and draws again.

use {
    super::{Painter, Place},
    gpui_kit::*,
    moonframe_core::Stack,
    std::{cell::Cell, rc::Rc},
};

/// A key a scrolling stack is to reveal, kept until it has shown.
#[derive(Debug)]
pub(crate) struct Pending {
    key: String,
    /// The frame the first stack with the key was drawn in.
    claimed: Cell<Option<u64>>,
    /// Where that stack was laid out, scrolled as it was.
    found: Cell<Option<Bounds<Pixels>>>,
    done: Cell<bool>,
}

impl Pending {
    pub(crate) fn new(key: String) -> Self {
        Self {
            key,
            claimed: Cell::new(None),
            found: Cell::new(None),
            done: Cell::new(false),
        }
    }

    pub(crate) fn done(&self) -> bool {
        self.done.get()
    }
}

/// What [`begin`] set up for one stack, until its children are drawn.
pub(super) struct Revealing {
    handle: ScrollHandle,
    /// The key this stack asks for, if it does.
    pending: Option<Rc<Pending>>,
    /// The one asked for around it, to restore.
    outer: Option<Rc<Pending>>,
    /// The mark this stack draws, being the one asked for around it.
    mark: Option<AnyElement>,
}

/// Before `stack` draws its children: its scroll handle, with any
/// `scroll_to` applied, and the key its descendants are looked through for.
pub(super) fn begin(stack: &Stack, painter: &mut Painter<'_, '_>, place: &Place<'_>) -> Revealing {
    let mark = stack
        .key
        .as_deref()
        .and_then(|key| mark(key, painter.reveal.as_ref()?, painter.frame));
    let (handle, pending) = painter
        .motion
        .scroll_to(&place.path, stack.scroll_to.as_ref());
    let outer = match &pending {
        Some(pending) => painter.reveal.replace(pending.clone()),
        None => painter.reveal.clone(),
    };
    Revealing {
        handle,
        pending,
        outer,
        mark,
    }
}

impl Revealing {
    /// Once the children are drawn: the handle, and what to draw last
    /// among them (no child's index moves).
    pub(super) fn end(self, painter: &mut Painter<'_, '_>) -> (ScrollHandle, Vec<AnyElement>) {
        painter.reveal = self.outer;
        let reader = self.pending.map(|pending| reader(pending, &self.handle));
        let extra = self.mark.into_iter().chain(reader).collect();
        (self.handle, extra)
    }
}

/// The mark the first stack with `pending`'s key draws each frame: a box
/// over it that records where it is laid out.
fn mark(key: &str, pending: &Rc<Pending>, frame: u64) -> Option<AnyElement> {
    if pending.key != key || pending.done() || pending.claimed.get() == Some(frame) {
        return None;
    }
    pending.claimed.set(Some(frame));
    let pending = pending.clone();
    let element = canvas(
        move |bounds, _, _| pending.found.set(Some(bounds)),
        |_, (), _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    Some(element.into_any_element())
}

/// A box of no size, last in the scrolling stack, that scrolls it to where
/// the mark was laid out, once.
fn reader(pending: Rc<Pending>, handle: &ScrollHandle) -> AnyElement {
    let handle = handle.clone();
    canvas(
        move |_, window, _| {
            let Some(target) = pending.found.take() else {
                return;
            };
            pending.done.set(true);
            let offset = handle.offset();
            let shown = revealed(handle.bounds(), target, offset, handle.max_offset());
            if shown != offset {
                handle.set_offset(shown);
                window.refresh();
            }
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_0()
    .into_any_element()
}

/// The offset (GPUI's, at or below zero) that shows `target`, laid out at
/// `offset` in a box at `view` that scrolls as far as `max`: unchanged if
/// it shows, else moved just enough, its start winning when it is larger.
pub(super) fn revealed(
    view: Bounds<Pixels>,
    target: Bounds<Pixels>,
    offset: Point<Pixels>,
    max: Point<Pixels>,
) -> Point<Pixels> {
    let along = |near: Pixels, far: Pixels, start: Pixels, end: Pixels| {
        if near < start {
            start - near
        } else if far > end {
            -(far - end).min(near - start)
        } else {
            px(0.)
        }
    };
    let x = along(target.left(), target.right(), view.left(), view.right());
    let y = along(target.top(), target.bottom(), view.top(), view.bottom());
    point(
        (offset.x + x).clamp(-max.x, px(0.)),
        (offset.y + y).clamp(-max.y, px(0.)),
    )
}

#[cfg(test)]
mod tests;
