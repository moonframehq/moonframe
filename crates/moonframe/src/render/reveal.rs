//! A scrolling row or column's `scroll_to`: a direct child, or the first
//! row or column drawn inside it with a key, however deep, comes into view,
//! scrolled only as far as needed, once the stack is laid out with it (an
//! overlay's content first appears with no size, and grows in).
//!
//! While a key is asked for, the scrolling stack is among those its
//! descendants draw inside ([`Painter::reveal`]); the first with the key
//! draws a mark that records where it is, and the stack, which lays out
//! after its children, scrolls to it and draws again, until it stays put
//! and nothing inside the stack grows or shrinks (a card growing in before
//! it, or waiting to, may push it on for a while). Scrolling by hand in
//! between ends it.

use {
    super::{Painter, Place},
    gpui_kit::*,
    moonframe_core::Stack,
    std::{cell::Cell, rc::Rc},
};

/// What a scrolling stack is to reveal, kept until it has shown.
#[derive(Debug)]
pub(crate) struct Pending {
    target: Target,
    /// The frame the first stack with the key was drawn in.
    claimed: Cell<Option<u64>>,
    /// Where that stack was laid out, scrolled as it was.
    found: Cell<Option<Bounds<Pixels>>>,
    /// Where it was in the content when last revealed, in a box of what
    /// size, and how many frames it has been revealed in.
    last: Cell<Option<(Point<Pixels>, Size<Pixels>)>>,
    tries: Cell<u32>,
    /// Something inside the stack grew or shrank this frame (a card growing
    /// in, or waiting to): the target may yet move.
    moving: Cell<bool>,
    /// The offset it last scrolled to: one that differs since is the
    /// user's, which ends the reveal.
    set: Cell<Option<Point<Pixels>>>,
    done: Cell<bool>,
}

/// The most frames a reveal follows a moving target for: a layout that
/// never settles does not hold the scroll for good.
const MAX_TRIES: u32 = 120;

#[derive(Debug)]
enum Target {
    /// A direct child, from 0.
    Child(usize),
    Key(String),
    /// Points scrolled across and down; an axis not given stays.
    At(Option<f32>, Option<f32>),
}

impl Pending {
    /// The direct child at `index`, from 0.
    pub(crate) fn child(index: usize) -> Self {
        Self::new(Target::Child(index))
    }

    pub(crate) fn key(key: String) -> Self {
        Self::new(Target::Key(key))
    }

    /// Scrolled `x` across and `y` down, in points.
    pub(crate) fn at(x: Option<f32>, y: Option<f32>) -> Self {
        Self::new(Target::At(x, y))
    }

    fn new(target: Target) -> Self {
        Self {
            target,
            claimed: Cell::new(None),
            found: Cell::new(None),
            last: Cell::new(None),
            tries: Cell::new(0),
            moving: Cell::new(false),
            set: Cell::new(None),
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
    /// What this stack asks to reveal, if anything.
    pending: Option<Rc<Pending>>,
    /// The marks this stack draws, being what stacks around it ask for.
    marks: Vec<AnyElement>,
    /// Whether what was drawn before this stack resizes, put back once its
    /// children have said whether they do.
    resizing: bool,
    /// Where its keyed children note their layout, as a column that keeps
    /// the first one showing in place (see `anchor`).
    anchoring: Option<Rc<super::anchor::Anchoring>>,
}

/// Before `stack` draws its children: its scroll handle, with any
/// `scroll_to` applied, and the key its descendants are looked through for.
pub(super) fn begin(stack: &Stack, painter: &mut Painter<'_, '_>, place: &Place<'_>) -> Revealing {
    let marks = match stack.key.as_deref() {
        Some(key) => painter
            .reveal
            .iter()
            .filter_map(|pending| mark(key, pending, painter.frame))
            .collect(),
        None => Vec::new(),
    };
    let (handle, pending) = painter
        .motion
        .scroll_to(&place.path, stack.scroll_to.as_ref());
    let resizing = painter.resizing;
    if let Some(pending) = &pending {
        painter.reveal.push(pending.clone());
        painter.resizing = false;
    }
    let anchoring = super::anchor::begin(stack, pending.is_some(), painter, place);
    Revealing {
        handle,
        pending,
        marks,
        resizing,
        anchoring,
    }
}

impl Revealing {
    /// Once the children are drawn: the handle, what to draw last among
    /// them (no child's index moves), and the stack's anchoring, for
    /// `anchor::wrap`.
    pub(super) fn end(
        self,
        painter: &mut Painter<'_, '_>,
    ) -> (
        ScrollHandle,
        Vec<AnyElement>,
        Option<Rc<super::anchor::Anchoring>>,
    ) {
        super::anchor::end(painter, self.anchoring.as_ref());
        if let Some(pending) = &self.pending {
            painter.reveal.pop();
            pending.moving.set(painter.resizing);
            painter.resizing |= self.resizing;
        }
        let reader = self.pending.map(|pending| reader(pending, &self.handle));
        let extra = self.marks.into_iter().chain(reader).collect();
        (self.handle, extra, self.anchoring)
    }
}

/// The mark the first stack with `pending`'s key draws each frame: a box
/// over it that records where it is laid out.
fn mark(key: &str, pending: &Rc<Pending>, frame: u64) -> Option<AnyElement> {
    let wanted = matches!(&pending.target, Target::Key(wanted) if wanted == key);
    if !wanted || pending.done() || pending.claimed.get() == Some(frame) {
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
/// the child, or the mark, was laid out (or as far as asked), each frame
/// until it stays where it was in the content (the content stays as long)
/// of a box that keeps its size.
fn reader(pending: Rc<Pending>, handle: &ScrollHandle) -> AnyElement {
    let handle = handle.clone();
    canvas(
        move |_, window, _| {
            let (view, offset, max) = (handle.bounds(), handle.offset(), handle.max_offset());
            // Where to scroll, and what must stay put for it to be done: a
            // child's place in the content, or how far the content scrolls.
            let shown = |target: Bounds<Pixels>| {
                (
                    revealed(view, target, offset, max),
                    target.origin - view.origin - offset,
                )
            };
            let placed = match pending.target {
                // Its children as laid out, before scrolling.
                Target::Child(index) => handle
                    .bounds_for_item(index)
                    .map(|child| shown(child + offset)),
                Target::Key(_) => pending.found.take().map(shown),
                Target::At(x, y) => Some((scrolled_to(x, y, offset, max), max)),
            };
            let Some((shown, at)) = placed.filter(|_| !view.size.is_zero()) else {
                if waits(&pending, view.size.is_zero()) {
                    window.refresh();
                }
                return;
            };
            let near = |a: Point<Pixels>, b: Point<Pixels>| {
                (a.x - b.x).abs() < px(0.5) && (a.y - b.y).abs() < px(0.5)
            };
            if pending.set.get().is_some_and(|set| !near(set, offset)) {
                pending.done.set(true);
                return;
            }
            if shown != offset {
                handle.set_offset(shown);
            }
            pending.set.set(Some(shown));
            let settled = pending
                .last
                .replace(Some((at, view.size)))
                .is_some_and(|(last, size)| near(last, at) && size == view.size);
            pending.tries.set(pending.tries.get() + 1);
            if (settled && !pending.moving.get()) || pending.tries.get() >= MAX_TRIES {
                pending.done.set(true);
            }
            if shown != offset || !pending.done() {
                window.refresh();
            }
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_0()
    .into_any_element()
}

/// Whether a reveal whose target is not there this frame draws again for
/// it: only while the stack has no size yet (an overlay's content as it
/// opens), and at most [`MAX_TRIES`] times. Laid out without it (a row not
/// arrived or filtered out), it waits for whatever draws the window next.
fn waits(pending: &Pending, sizeless: bool) -> bool {
    if pending.done() || !sizeless || pending.tries.get() >= MAX_TRIES {
        return false;
    }
    pending.tries.set(pending.tries.get() + 1);
    true
}

/// The offset (GPUI's, at or below zero) scrolled `x` across and `y` down,
/// as far as the content goes (`max`); an axis not given stays at `offset`.
pub(super) fn scrolled_to(
    x: Option<f32>,
    y: Option<f32>,
    offset: Point<Pixels>,
    max: Point<Pixels>,
) -> Point<Pixels> {
    let along = |to: Option<f32>, now: Pixels, max: Pixels| {
        to.map_or(now, |to| -px(to).min(max).max(px(0.)))
    };
    point(along(x, offset.x, max.x), along(y, offset.y, max.y))
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
