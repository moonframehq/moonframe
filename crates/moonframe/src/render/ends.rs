//! A scrolling row's or column's end, for a chat or log with a button back
//! to the latest line: `at_end` tells the mod as it leaves or reaches its
//! end, and `scroll_end` scrolls there each time its value changes.

use {
    super::{Cx, Painter, Place, pointer::Sender},
    gpui_kit::*,
    moonframe_core::{Data, Direction, Ends, Frame, Scroll, with_fields},
    std::{
        cell::RefCell,
        collections::{BTreeMap, HashMap},
    },
};

/// How near the end still counts as there, as for `follow`: the last
/// line's rounding, or a line arriving before the next frame.
const SLACK: f32 = 24.;

thread_local! {
    /// The last `scroll_end` each stack took, by its window and place.
    static ASKED: RefCell<HashMap<String, Data>> = RefCell::new(HashMap::new());
    /// What each stack last told its mod, and the window frame it was last
    /// drawn in: one missing from the frame before is drawn anew, and its
    /// mod is told again.
    static TOLD: RefCell<HashMap<String, (bool, u64)>> = RefCell::new(HashMap::new());
}

/// The axes a stack of `direction` scrolls along: `(x, y)`.
fn axes(direction: Direction, frame: &Frame) -> (bool, bool) {
    match (direction, frame.scroll) {
        (_, Scroll::No) => (false, false),
        (Direction::Row, Scroll::Along) | (_, Scroll::X) => (true, false),
        (Direction::Column, Scroll::Along) | (_, Scroll::Y) => (false, true),
        (_, Scroll::Both) => (true, true),
    }
}

/// Whether a box scrolled `offset` (GPUI's, at or below zero), which can
/// scroll `max`, is at its end: across, unless it scrolls only sideways.
fn at_end((x, y): (bool, bool), offset: Point<Pixels>, max: Point<Pixels>) -> bool {
    let there = |offset: Pixels, max: Pixels| -f32::from(offset) >= f32::from(max) - SLACK;
    if y || !x {
        there(offset.y, max.y)
    } else {
        there(offset.x, max.x)
    }
}

/// Whether `asked` is a new `scroll_end` for the stack `key`.
fn newly_asked(key: &str, asked: &Data) -> bool {
    ASKED.with_borrow_mut(|seen| {
        if seen.get(key) == Some(asked) {
            return false;
        }
        seen.insert(key.to_owned(), asked.clone());
        true
    })
}

/// Forgets the stack at `place`'s last `scroll_end`, as it draws without
/// one: asked for again, the same value is a new request.
pub(super) fn not_asked(painter: &Painter<'_, '_>, place: &Place<'_>) {
    if ASKED.with_borrow(HashMap::is_empty) {
        return;
    }
    let key = format!("{:?}/{}", painter.window, place.path);
    ASKED.with_borrow_mut(|seen| seen.remove(&key));
}

/// Whether the stack `key`, drawn in window frame `frame` and `end` now,
/// tells its mod.
fn tell(key: &str, frame: u64, end: bool) -> bool {
    TOLD.with_borrow_mut(|told| {
        let last = told.insert(key.to_owned(), (end, frame));
        match last {
            Some((was, seen)) if frame == 0 || seen + 1 >= frame => was != end,
            _ => true,
        }
    })
}

/// Scrolls `handle` to its end if `ends` asks anew, and returns a box of no
/// size, last among the stack's children so it changes no child's index,
/// that reads `handle` once the stack has laid out and scrolled and tells
/// `at_end` when that changed.
pub(super) fn ends(
    ends: &Ends,
    direction: Direction,
    frame: &Frame,
    handle: &ScrollHandle,
    painter: &Painter<'_, '_>,
    place: &Place<'_>,
    cx: Cx,
) -> Option<AnyElement> {
    let axes = axes(direction, frame);
    let key = format!("{:?}/{}", painter.window, place.path);
    if ends.scroll_end.is_none() {
        not_asked(painter, place);
    }
    if let Some(asked) = &ends.scroll_end
        && newly_asked(&key, asked)
    {
        if axes.0 {
            // GPUI clamps this to the content's far edge in prepaint.
            handle.set_offset(point(-Pixels::MAX, handle.offset().y));
        }
        if axes.1 {
            handle.scroll_to_bottom();
        }
    }
    let msg = ends.at_end.clone()?;
    if painter.preview {
        return None;
    }
    let frame = painter.frame;
    let sender = Sender {
        app: cx.entity().downgrade(),
        owner: place.owner.to_owned(),
        to: None,
    };
    let handle = handle.clone();
    let element = canvas(
        move |_, window, cx| {
            let end = at_end(axes, handle.offset(), handle.max_offset());
            if !tell(&key, frame, end) {
                return;
            }
            let msg = with_fields(
                &msg,
                BTreeMap::from([("at_end".to_owned(), Data::Bool(end))]),
            );
            window.defer(cx, move |window, cx| sender.send(msg, window, cx));
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_0();
    Some(element.into_any_element())
}

#[cfg(test)]
mod tests {
    use {
        super::{Data, at_end, newly_asked, tell},
        gpui_kit::{point, px},
    };

    #[test]
    fn near_the_end_is_at_it_and_a_box_that_does_not_scroll_always_is() {
        let column = (false, true);
        let max = point(px(0.), px(500.));
        assert!(!at_end(column, point(px(0.), px(0.)), max));
        assert!(!at_end(column, point(px(0.), px(-400.)), max));
        assert!(at_end(column, point(px(0.), px(-480.)), max));
        assert!(at_end(column, point(px(0.), px(-500.)), max));
        assert!(at_end(column, point(px(0.), px(0.)), point(px(0.), px(0.))));
        // A row reads its own axis.
        let row = (true, false);
        assert!(!at_end(row, point(px(0.), px(0.)), point(px(300.), px(0.))));
        assert!(at_end(
            row,
            point(px(-300.), px(0.)),
            point(px(300.), px(0.))
        ));
    }

    #[test]
    fn the_mod_is_told_of_changes_and_again_when_drawn_anew() {
        assert!(tell("told", 1, true));
        assert!(!tell("told", 2, true));
        assert!(tell("told", 3, false));
        assert!(!tell("told", 4, false));
        assert!(tell("told", 5, true));
        // Not drawn in frames 6 and 7.
        assert!(tell("told", 8, true));
    }

    #[test]
    fn the_same_request_twice_scrolls_once_and_a_new_one_again() {
        let first = Data::Map([("seq".to_owned(), Data::Integer(1))].into());
        let second = Data::Map([("seq".to_owned(), Data::Integer(2))].into());
        assert!(newly_asked("asked", &first));
        assert!(!newly_asked("asked", &first));
        assert!(newly_asked("asked", &second));
        // Drawn without one, then asked for the same again: a new request.
        super::ASKED.with_borrow_mut(|seen| seen.remove("asked"));
        assert!(newly_asked("asked", &second));
    }
}
