//! A row's or column's `pointer`: presses followed until the button comes up,
//! so a slider keeps up when the pointer leaves its track, the wheel, and the
//! cursor. Positions are fractions of the element, as on GPUI.

use {
    super::{Painter, Place},
    crate::Ui,
    moonframe_core::{Cursor, Held, Pointer, PointerPhase, Travel},
    rune::{Cursor as RuneCursor, Drag, El, Phase, Tx, WheelEvent},
};

/// Moves shorter than this, in points, are not sent: a trackpad reports
/// sub-point jitter that would only redraw the same thing.
const STILL: f32 = 0.5;
/// Wheel lines to a step of `zoom`, as the map takes them.
const WHEEL_ZOOM: f32 = 120.;

fn shape(cursor: Cursor) -> RuneCursor {
    match cursor {
        Cursor::Pointer => RuneCursor::Pointer,
        Cursor::Text => RuneCursor::Text,
        Cursor::Crosshair => RuneCursor::Crosshair,
        Cursor::Grab => RuneCursor::Grab,
        Cursor::Grabbing => RuneCursor::Grabbing,
        Cursor::Move => RuneCursor::Move,
        Cursor::EwResize | Cursor::ColResize => RuneCursor::ResizeColumn,
        Cursor::NsResize | Cursor::RowResize => RuneCursor::ResizeRow,
        Cursor::NotAllowed => RuneCursor::NotAllowed,
        _ => RuneCursor::Default,
    }
}

/// Where `drag` is, as fractions of its element.
fn travel(drag: &Drag) -> Travel {
    let fraction = |at: f32, size: f32| {
        if size > 0. {
            at / size
        } else {
            0.
        }
    };
    Travel {
        x: fraction(drag.x, drag.w),
        y: fraction(drag.y, drag.h),
        dx: drag.dx,
        dy: drag.dy,
        width: drag.w,
        height: drag.h,
    }
}

impl<'w> Painter<'w> {
    /// `element` with `pointer`'s listeners and cursor. `carry`, `accept`,
    /// `drop` and `gestures` are not on this backend yet.
    pub(super) fn pointered(&self, element: El, pointer: &Pointer, place: &Place<'w>) -> El {
        let lacking = pointer.carry.is_some()
            || pointer.accept.is_some()
            || pointer.drop.is_some()
            || pointer.hover
            || !pointer.gestures.is_empty();
        if lacking {
            let mut left_out = self.ui.left_out.borrow_mut();
            left_out.insert("pointer carry, drop, hover or gestures".to_owned());
        }
        let element = match pointer.cursor {
            Some(cursor) => element.cursor(shape(cursor)),
            None => element,
        };
        let owner = place.owner.to_owned();
        let to = pointer.to.clone().unwrap_or_else(|| owner.clone());
        let element = if pointer.press.is_some() {
            let (pointer, owner, to) = (pointer.clone(), owner.clone(), to.clone());
            element.on_drag(move |ui: &mut Ui, drag: &mut Drag, _: &mut Tx| {
                // A relative drag counts from the value it had at the press,
                // not from what each move already set.
                let mut pressed = pointer.clone();
                let phase = match drag.phase {
                    Phase::Down => {
                        drag.data = pointer.range.map_or(0., |range| range.value);
                        PointerPhase::Down
                    },
                    Phase::Move => PointerPhase::Move,
                    Phase::Up => PointerPhase::Up,
                };
                if let Some(range) = &mut pressed.range {
                    range.value = drag.data;
                }
                let at = travel(drag);
                let clicks = usize::try_from(drag.clicks).unwrap_or(1);
                let held = Held::default();
                if let Some(msg) = pressed.press_message(phase, &at, "left", clicks, held) {
                    ui.send(&owner, &to, msg);
                }
            })
        } else {
            element
        };
        if pointer.wheel.is_some() {
            let pointer = pointer.clone();
            element.on_wheel(move |ui: &mut Ui, event: WheelEvent, _: &mut Tx| {
                if event.dx.abs() < STILL && event.dy.abs() < STILL {
                    return false;
                }
                let at = Travel {
                    x: 0.,
                    y: 0.,
                    dx: event.dx,
                    dy: event.dy,
                    width: 0.,
                    height: 0.,
                };
                let zoom = event.dy / WHEEL_ZOOM;
                match pointer.wheel_message(&at, zoom, Held::default()) {
                    Some(msg) => {
                        ui.send(&owner, &to, msg);
                        true
                    },
                    None => false,
                }
            })
        } else {
            element
        }
    }
}
