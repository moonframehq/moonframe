//! A row's or column's `outside`: sent when a press lands outside its box,
//! the press going on to whatever is there, as a menu or a floating panel
//! that closes on a click elsewhere lets that click through. Popovers and
//! overlays drawn inside it float beyond its box but count as inside.

use {
    super::{Cx, Painter},
    gpui_kit::*,
    moonframe_core::Action,
    std::{cell::RefCell, rc::Rc},
};

/// The boxes of the popovers and overlays a stack drew this frame, which a
/// press in is not outside it.
pub(super) type Inside = Rc<RefCell<Vec<Bounds<Pixels>>>>;

/// `element` sending `action` from `owner` when a press lands outside it
/// and outside `inside`. It listens as the press goes down the tree and
/// lets it go on.
pub(super) fn attach(
    element: Stateful<Div>,
    action: &Action,
    inside: Inside,
    owner: &str,
    cx: Cx,
) -> Stateful<Div> {
    let owner = owner.to_owned();
    let action = action.clone();
    element.on_mouse_down_out(
        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
            if event.button != MouseButton::Left
                || inside
                    .borrow()
                    .iter()
                    .any(|bounds| bounds.contains(&event.position))
            {
                return;
            }
            this.dispatch(&owner, &action, window, cx);
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
