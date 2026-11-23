//! A scrolling row or column with `measure`: told, as it lays out and
//! scrolls, its box, its content's size, how far it is scrolled, how many
//! of its first children fit in its box, and which show. A strip shows its
//! arrows only when it overflows, a row of pills keeps the ones that fit, a
//! list keeps where it was read: each from what this says.

use {
    super::{Cx, Painter, Place, pointer::Sender},
    gpui_kit::*,
    moonframe_core::{Action, Data, Direction, with_fields},
    std::{cell::RefCell, collections::HashMap},
};

thread_local! {
    /// What each stack last told its mod, by its window and place, so it is
    /// told only of changes.
    static TOLD: RefCell<HashMap<String, Announcement>> = RefCell::new(HashMap::new());
    /// The frame each stack was last drawn in: one missing from the frame
    /// before is drawn anew, and its mod (whose state may have started over)
    /// is told again.
    static SEEN: RefCell<HashMap<String, u64>> = RefCell::new(HashMap::new());
}

struct Announcement {
    reading: Reading,
    msg: Data,
    to: Option<String>,
}

/// What a stack says, in whole pixels and one-based children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Reading {
    pub width: i64,
    pub height: i64,
    pub content_width: i64,
    pub content_height: i64,
    pub x: i64,
    pub y: i64,
    /// How many of its first children end inside its box, unscrolled.
    pub fit: i64,
    /// The first and last child any part of which shows; 0 for none.
    pub first: i64,
    pub last: i64,
}

impl Reading {
    /// What a stack of `direction` reads: `bounds` its box, `max` how far
    /// it can scroll, `offset` how far it is (GPUI's, at or below zero), and
    /// `children` its own children's boxes as laid out, before scrolling.
    pub(super) fn of(
        direction: Direction,
        bounds: Bounds<Pixels>,
        max: Point<Pixels>,
        offset: Point<Pixels>,
        children: &[Bounds<Pixels>],
    ) -> Self {
        let whole = |pixels: Pixels| f32::from(pixels).round() as i64;
        let scrolled = point(-offset.x, -offset.y);
        // Along the stack's axis: where a child starts and ends, where the
        // box does, and where the part that shows does.
        let along = |child: &Bounds<Pixels>| match direction {
            Direction::Row => (child.left(), child.right()),
            Direction::Column => (child.top(), child.bottom()),
        };
        let (start, end) = along(&bounds);
        let shift = match direction {
            Direction::Row => scrolled.x,
            Direction::Column => scrolled.y,
        };
        // Half a pixel either way is layout's rounding, not overflow.
        let slack = px(0.5);
        let fit = children
            .iter()
            .take_while(|child| along(child).1 <= end + slack)
            .count();
        let shows = |child: &Bounds<Pixels>| {
            let (near, far) = along(child);
            far > start + shift + slack && near < end + shift - slack
        };
        let first = children.iter().position(shows).map_or(0, |index| index + 1);
        let last = children
            .iter()
            .rposition(shows)
            .map_or(0, |index| index + 1);
        Reading {
            width: whole(bounds.size.width),
            height: whole(bounds.size.height),
            content_width: whole(bounds.size.width + max.x),
            content_height: whole(bounds.size.height + max.y),
            x: whole(scrolled.x),
            y: whole(scrolled.y),
            fit: fit as i64,
            first: first as i64,
            last: last as i64,
        }
    }

    /// `msg` with the reading's fields added.
    pub(super) fn message(&self, msg: &Data) -> Data {
        let fields = [
            ("width", self.width),
            ("height", self.height),
            ("content_width", self.content_width),
            ("content_height", self.content_height),
            ("x", self.x),
            ("y", self.y),
            ("fit", self.fit),
            ("first", self.first),
            ("last", self.last),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), Data::Number(value as f64)))
        .collect();
        with_fields(msg, fields)
    }
}

/// A box of no size, last among the stack's children so it adds nothing to
/// what scrolls and changes no child's index, that reads `handle` once the
/// stack has laid out and scrolled and tells `measure` what changed.
/// `children` is how many children the stack drew of its own.
pub(super) fn reader(
    measure: &Action,
    direction: Direction,
    handle: &ScrollHandle,
    children: usize,
    painter: &Painter<'_, '_>,
    place: &Place<'_>,
    cx: Cx,
) -> Option<AnyElement> {
    let Action::Send { to, msg } = measure else {
        return None;
    };
    let key = format!("{:?}/{}", painter.window, place.path);
    let frame = painter.frame;
    let last = SEEN.with_borrow_mut(|seen| seen.insert(key.clone(), frame));
    if frame > 0 && !matches!(last, Some(seen) if seen + 1 >= frame) {
        TOLD.with_borrow_mut(|told| told.remove(&key));
    }
    let sender = Sender {
        app: cx.entity().downgrade(),
        owner: place.owner.to_owned(),
        to: to.clone(),
    };
    let (msg, handle) = (msg.clone(), handle.clone());
    let element = canvas(
        move |_, window, cx| {
            let boxes: Vec<Bounds<Pixels>> = (0..children)
                .filter_map(|index| handle.bounds_for_item(index))
                .collect();
            let reading = Reading::of(
                direction,
                handle.bounds(),
                handle.max_offset(),
                handle.offset(),
                &boxes,
            );
            let changed = TOLD.with_borrow_mut(|told| {
                if told.get(&key).is_some_and(|previous| {
                    previous.reading == reading && previous.msg == msg && previous.to == sender.to
                }) {
                    false
                } else {
                    told.insert(key, Announcement {
                        reading,
                        msg: msg.clone(),
                        to: sender.to.clone(),
                    });
                    true
                }
            });
            if !changed {
                return;
            }
            let msg = reading.message(&msg);
            window.defer(cx, move |window, cx| sender.send(msg, window, cx));
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_0();
    Some(element.into_any_element())
}

#[cfg(test)]
mod tests;
