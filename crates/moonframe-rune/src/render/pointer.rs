//! A row's or column's `pointer`: presses followed until the button comes up,
//! so a slider keeps up when the pointer leaves its track, the wheel, and the
//! cursor. Positions are fractions of the element, as on GPUI.

use {
    super::{Painter, Place},
    crate::Ui,
    moonframe_core::{Cursor, Held, Pointer, PointerPhase, Travel},
    rune::{Cursor as RuneCursor, Drag, El, Phase, PointerEvent, Tx, WheelEvent},
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
        // Trackpad rotation, smart zoom and swipes: Rune hears none of them.
        if !pointer.gestures.is_empty() {
            let mut left_out = self.ui.left_out.borrow_mut();
            left_out.insert("trackpad gestures".to_owned());
        }
        let size = rune::memo::layout_size(&place.path).unwrap_or([0., 0.]);
        let element = if pointer.carry.is_some() || pointer.accept.is_some() {
            self.carried(element, pointer, place)
        } else {
            element
        };
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
        let element = if pointer.hover {
            let (moving, owner, to) = (pointer.clone(), owner.clone(), to.clone());
            let (leaving, gone_owner, gone_to) = (pointer.clone(), owner.clone(), to.clone());
            element
                .on_pointer_move_local(move |ui: &mut Ui, at: PointerEvent, _: &mut Tx| {
                    if ui.carrying.get_mut().is_some() {
                        return;
                    }
                    let travel = Travel {
                        x: if size[0] > 0. {
                            at.x / size[0]
                        } else {
                            0.
                        },
                        y: if size[1] > 0. {
                            at.y / size[1]
                        } else {
                            0.
                        },
                        dx: 0.,
                        dy: 0.,
                        width: size[0],
                        height: size[1],
                    };
                    let held = Held::default();
                    if let Some(msg) =
                        moving.press_message(PointerPhase::Hover, &travel, "", 0, held)
                    {
                        ui.send(&owner, &to, msg);
                    }
                })
                .on_hover_change(move |ui: &mut Ui, inside: bool, _: &mut Tx| {
                    if inside {
                        return;
                    }
                    let travel = Travel {
                        x: 0.,
                        y: 0.,
                        dx: 0.,
                        dy: 0.,
                        width: size[0],
                        height: size[1],
                    };
                    let held = Held::default();
                    if let Some(msg) =
                        leaving.press_message(PointerPhase::Leave, &travel, "", 0, held)
                    {
                        ui.send(&gone_owner, &gone_to, msg);
                    }
                })
        } else {
            element
        };
        let element = if pointer.drop.is_some() {
            let (dropping, owner, to) = (pointer.clone(), owner.clone(), to.clone());
            element.on_drop_files(
                move |ui: &mut Ui, files: &[std::path::PathBuf], tx: &mut Tx| {
                    let point = tx.drop_point();
                    let fraction = |at: f32, size: f32| {
                        if size > 0. {
                            at / size
                        } else {
                            0.
                        }
                    };
                    let travel = Travel {
                        x: point.map_or(0., |point| fraction(point.x, size[0])),
                        y: point.map_or(0., |point| fraction(point.y, size[1])),
                        dx: 0.,
                        dy: 0.,
                        width: size[0],
                        height: size[1],
                    };
                    if let Some(msg) = dropping.drop_message(files, &travel) {
                        ui.send(&owner, &to, msg);
                    }
                },
            )
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
