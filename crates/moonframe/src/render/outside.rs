//! A row's or column's `outside`: sent when a click (pressed and let go)
//! lands outside its box, the click going on to whatever is there, as a
//! menu or a floating panel that closes on a click elsewhere lets that
//! click through. It is sent as the button comes up, with the click, so
//! the view it changes does not move what was clicked from under it.
//! Popovers and overlays drawn inside it float beyond its box but count as
//! inside.

use {
    super::{Cx, Painter},
    gpui_kit::*,
    moonframe_core::Action,
    std::{cell::RefCell, collections::HashSet, rc::Rc},
};

/// The boxes of the popovers and overlays a stack drew this frame, which a
/// press in is not outside it.
pub(super) type Inside = Rc<RefCell<Vec<Bounds<Pixels>>>>;

thread_local! {
    /// The stacks, by owner and path, a press began outside of: its
    /// release outside sends their `outside`.
    static PRESSED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// `element` sending `action` from `owner` when a click begins and ends
/// outside it and outside `inside`. It listens as the click goes down the
/// tree and lets it go on.
pub(super) fn attach(
    element: Stateful<Div>,
    action: &Action,
    inside: Inside,
    owner: &str,
    path: &str,
    cx: Cx,
) -> Stateful<Div> {
    let key = format!("{owner}\u{0}{path}");
    let (owner, action) = (owner.to_owned(), action.clone());
    let within = move |position: &Point<Pixels>| {
        inside
            .borrow()
            .iter()
            .any(|bounds| bounds.contains(position))
    };
    let within_up = within.clone();
    let (down, held, up) = (key.clone(), key.clone(), key);
    element
        .capture_any_mouse_down(move |_, _, _| {
            PRESSED.with_borrow_mut(|pressed| pressed.remove(&held));
        })
        .on_mouse_down_out(move |event: &MouseDownEvent, _, _| {
            if event.button == MouseButton::Left && !within(&event.position) {
                PRESSED.with_borrow_mut(|pressed| pressed.insert(down.clone()));
            } else {
                PRESSED.with_borrow_mut(|pressed| pressed.remove(&down));
            }
        })
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseUpEvent, window, cx| {
                let began = PRESSED.with_borrow_mut(|pressed| pressed.remove(&up));
                if began && !within_up(&event.position) {
                    this.dispatch(&owner, &action, window, cx);
                }
            }),
        )
}

/// A layer over floating content drawn inside stacks that hear `outside`,
/// recording its box in each of them; `None` inside no such stack.
pub(super) fn recorder(painter: &Painter<'_, '_>) -> Option<AnyElement> {
    if painter.outside.is_empty() {
        return None;
    }
    let stacks = painter.outside.clone();
    Some(
        canvas(
            move |bounds, _, _| {
                for inside in &stacks {
                    let mut boxes = inside.borrow_mut();
                    if !boxes.contains(&bounds) {
                        boxes.push(bounds);
                    }
                }
            },
            |_, (), _, _| {},
        )
        .absolute()
        .size_full()
        .into_any_element(),
    )
}
