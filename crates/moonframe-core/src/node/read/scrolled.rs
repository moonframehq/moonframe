//! The children of a row or column that scrolls along its axis keep their
//! size along it, as CSS's `min-height: auto` keeps a flex item's: squeezed
//! to the box they would draw over each other instead of scrolling.

use super::*;

/// Keeps `shrink = false` on each stack or panel child of a container that
/// scrolls along `direction`, unless the child gives `shrink` itself or
/// scrolls along the same axis, which needs the room it is given to scroll in.
pub(super) fn hold_along(
    frame: &Frame,
    direction: Direction,
    items: &[Value],
    children: &mut [Node],
) {
    if !scrolls_along(frame.scroll, direction) {
        return;
    }
    for (item, child) in items.iter().zip(children) {
        let given = match item {
            Value::Table(table) => !matches!(table.raw_get::<Value>("shrink"), Ok(Value::Nil)),
            _ => true,
        };
        let (frame, own) = match child {
            Node::Stack(stack) => (&mut stack.frame, stack.direction),
            Node::Panel(panel) => (&mut panel.frame, Direction::Column),
            _ => continue,
        };
        if !given && !scrolls(frame.scroll, own, direction) {
            frame.shrink = false;
        }
    }
}

fn scrolls_along(scroll: Scroll, direction: Direction) -> bool {
    scrolls(scroll, direction, direction)
}

/// Whether a container laid out along `own` scrolls along `axis`.
fn scrolls(scroll: Scroll, own: Direction, axis: Direction) -> bool {
    match scroll {
        Scroll::Along => own == axis,
        Scroll::Both => true,
        Scroll::X => axis == Direction::Row,
        Scroll::Y => axis == Direction::Column,
        Scroll::No => false,
    }
}
