//! A row's or column's `pointer`, `wheel`, `drop` and `cursor`. A press is
//! followed wherever the pointer goes until the button comes up, so a
//! slider keeps up when the pointer leaves its track; positions stay
//! fractions of the box as it was when the button went down.

use {
    super::{Cx, Place},
    crate::app::MoonframeApp,
    gpui_kit::*,
    moonframe_core::{Action, Cursor, Data, Held, Pointer, PointerPhase, Range, Travel},
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

/// `secondary` means ⌘ here, Ctrl elsewhere.
const MAC: bool = cfg!(target_os = "macos");
/// Moves shorter than this, in pixels, are not sent: a trackpad reports
/// sub-pixel jitter that would only redraw the same thing.
const STILL: f32 = 0.5;

/// The press being followed: one at a time, as there is one pointer.
struct Drag {
    window: WindowId,
    /// The pressed element's place, which only it answers to.
    path: String,
    /// Its box and the pointer when the button went down.
    bounds: Bounds<Pixels>,
    start: Point<Pixels>,
    /// The last position sent.
    last: Point<Pixels>,
    button: MouseButton,
    clicks: usize,
    /// The `range` as it was at the press: a relative drag counts from
    /// the value it had then, not from what each move already set.
    range: Option<Range>,
}

thread_local! {
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
}

/// `element` with `pointer`'s listeners and cursor.
pub(super) fn attach(
    element: Stateful<Div>,
    pointer: &Pointer,
    place: &Place<'_>,
    cx: Cx,
) -> Stateful<Div> {
    if pointer.is_empty() {
        return element;
    }
    // The box as last laid out, for the fractions a press reports.
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let mut element = element.child(measure(bounds.clone(), pointer, place, cx));
    if let Some(cursor) = pointer.cursor {
        element = element.cursor(style(cursor));
    }
    let sender = Sender {
        app: cx.entity().downgrade(),
        owner: place.owner.to_owned(),
        to: pointer.to.clone(),
    };
    if pointer.press.is_some() {
        element = press(element, pointer, &place.path, &bounds, &sender);
    }
    if pointer.wheel.is_some() {
        element = wheel(element, pointer, &bounds, &sender);
    }
    if pointer.drop.is_some() {
        let pointer = pointer.clone();
        let (bounds, sender) = (bounds.clone(), sender.clone());
        element = element.on_drop(move |paths: &ExternalPaths, window, cx| {
            let at = travel(bounds.get(), window.mouse_position(), point(px(0.), px(0.)));
            crate::render::images::grant(paths.paths());
            if let Some(msg) = pointer.drop_message(paths.paths(), &at) {
                sender.send(msg, window, cx);
            }
        });
    }
    element
}

/// Sends a message from the mod that drew the element.
#[derive(Clone)]
pub(super) struct Sender {
    pub(super) app: WeakEntity<MoonframeApp>,
    pub(super) owner: String,
    pub(super) to: Option<String>,
}

impl Sender {
    pub(super) fn send(&self, msg: Data, window: &mut Window, cx: &mut App) {
        let action = Action::Send {
            to: self.to.clone(),
            msg,
        };
        let _ = self
            .app
            .update(cx, |app, cx| app.dispatch(&self.owner, &action, window, cx));
    }
}

/// A canvas filling the element that keeps its box and, while it is the
/// one pressed, follows the pointer across the whole window.
fn measure(
    bounds: Rc<Cell<Bounds<Pixels>>>,
    pointer: &Pointer,
    place: &Place<'_>,
    cx: Cx,
) -> impl IntoElement {
    let pointer = pointer.clone();
    let path = place.path.clone();
    let sender = Sender {
        app: cx.entity().downgrade(),
        owner: place.owner.to_owned(),
        to: pointer.to.clone(),
    };
    canvas(
        move |laid, _, _| bounds.set(laid),
        move |_, (), window, _| {
            if pointer.press.is_none() || !pressed(&path, window) {
                return;
            }
            let (moved, released) = (pointer.clone(), pointer);
            let (moving, mover) = (path.clone(), sender.clone());
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || !pressed(&moving, window) {
                    return;
                }
                let Some((travel, button, clicks, range)) = DRAG.with_borrow_mut(|drag| {
                    let drag = drag.as_mut()?;
                    let step = event.position - drag.last;
                    if f32::from(step.x).abs() < STILL && f32::from(step.y).abs() < STILL {
                        return None;
                    }
                    drag.last = event.position;
                    Some((
                        travel(drag.bounds, event.position, drag.start),
                        drag.button,
                        drag.clicks,
                        drag.range,
                    ))
                }) else {
                    return;
                };
                let held = held(&event.modifiers);
                let pressed = Pointer {
                    range,
                    ..moved.clone()
                };
                if let Some(msg) =
                    pressed.press_message(PointerPhase::Move, &travel, name(button), clicks, held)
                {
                    mover.send(msg, window, cx);
                }
            });
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let Some((travel, clicks, range)) = DRAG.with_borrow_mut(|slot| {
                    let drag = slot.as_ref()?;
                    let window_id = window.window_handle().window_id();
                    if drag.window != window_id || drag.path != path || drag.button != event.button
                    {
                        return None;
                    }
                    let drag = slot.take()?;
                    Some((
                        travel(drag.bounds, event.position, drag.start),
                        drag.clicks,
                        drag.range,
                    ))
                }) else {
                    return;
                };
                let held = held(&event.modifiers);
                let released = Pointer {
                    range,
                    ..released.clone()
                };
                if let Some(msg) = released.press_message(
                    PointerPhase::Up,
                    &travel,
                    name(event.button),
                    clicks,
                    held,
                ) {
                    sender.send(msg, window, cx);
                }
            });
        },
    )
    .absolute()
    .size_full()
}

/// Whether the element at `path` holds the press in `window`.
fn pressed(path: &str, window: &Window) -> bool {
    let id = window.window_handle().window_id();
    DRAG.with_borrow(|drag| {
        drag.as_ref()
            .is_some_and(|drag| drag.window == id && drag.path == path)
    })
}

/// A press starts a drag; with `hover`, moves without a button are sent
/// too, and leaving it.
fn press(
    mut element: Stateful<Div>,
    pointer: &Pointer,
    path: &str,
    bounds: &Rc<Cell<Bounds<Pixels>>>,
    sender: &Sender,
) -> Stateful<Div> {
    for button in [MouseButton::Left, MouseButton::Middle, MouseButton::Right] {
        let (pointer, path, bounds, sender) = (
            pointer.clone(),
            path.to_owned(),
            bounds.clone(),
            sender.clone(),
        );
        element = element.on_mouse_down(button, move |event, window, cx| {
            // The innermost element with `pointer` takes the press.
            cx.stop_propagation();
            let laid = bounds.get();
            DRAG.set(Some(Drag {
                window: window.window_handle().window_id(),
                path: path.clone(),
                bounds: laid,
                start: event.position,
                last: event.position,
                button: event.button,
                clicks: event.click_count,
                range: pointer.range,
            }));
            let at = travel(laid, event.position, event.position);
            if let Some(msg) = pointer.press_message(
                PointerPhase::Down,
                &at,
                name(event.button),
                event.click_count,
                held(&event.modifiers),
            ) {
                sender.send(msg, window, cx);
            }
            // Draw again so the canvas follows the drag from now on.
            window.refresh();
        });
    }
    if pointer.hover {
        let (moving, path, box_now, mover) = (
            pointer.clone(),
            path.to_owned(),
            bounds.clone(),
            sender.clone(),
        );
        element = element.on_mouse_move(move |event, window, cx| {
            if event.pressed_button.is_some() || pressed(&path, window) {
                return;
            }
            let at = travel(box_now.get(), event.position, event.position);
            if let Some(msg) =
                moving.press_message(PointerPhase::Hover, &at, "", 0, held(&event.modifiers))
            {
                mover.send(msg, window, cx);
            }
        });
        let (leaving, bounds, sender) = (pointer.clone(), bounds.clone(), sender.clone());
        element = element.on_hover(move |inside, window, cx| {
            if *inside {
                return;
            }
            let at = travel(
                bounds.get(),
                window.mouse_position(),
                window.mouse_position(),
            );
            if let Some(msg) =
                leaving.press_message(PointerPhase::Leave, &at, "", 0, Held::default())
            {
                sender.send(msg, window, cx);
            }
        });
    }
    element
}

/// Scrolls and pinches over it are sent, and kept from scrolling what is
/// around it.
fn wheel(
    element: Stateful<Div>,
    pointer: &Pointer,
    bounds: &Rc<Cell<Bounds<Pixels>>>,
    sender: &Sender,
) -> Stateful<Div> {
    let (scrolled, scroll_bounds, scroll_sender) =
        (pointer.clone(), bounds.clone(), sender.clone());
    let (pinched, bounds, sender) = (pointer.clone(), bounds.clone(), sender.clone());
    element
        .on_scroll(move |event: &ScrollEvent, window, cx| {
            cx.stop_propagation();
            let delta = event.delta.pixel_delta(window.line_height());
            let mut at = travel(scroll_bounds.get(), event.position, event.position);
            at.dx = f32::from(delta.x);
            at.dy = f32::from(delta.y);
            if at.dx == 0.0 && at.dy == 0.0 {
                return;
            }
            if let Some(msg) = scrolled.wheel_message(&at, 0.0, held(&event.modifiers)) {
                scroll_sender.send(msg, window, cx);
            }
        })
        .on_pinch(move |event: &PinchEvent, window, cx| {
            cx.stop_propagation();
            if event.delta == 0.0 {
                return;
            }
            let at = travel(bounds.get(), event.position, event.position);
            if let Some(msg) = pinched.wheel_message(&at, event.delta, held(&event.modifiers)) {
                sender.send(msg, window, cx);
            }
        })
}

/// Where `now` is in `bounds`, and how far it went from `start`.
fn travel(bounds: Bounds<Pixels>, now: Point<Pixels>, start: Point<Pixels>) -> Travel {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let fraction = |offset: Pixels, size: f32| {
        if size > 0.0 {
            f32::from(offset) / size
        } else {
            0.0
        }
    };
    Travel {
        x: fraction(now.x - bounds.origin.x, width),
        y: fraction(now.y - bounds.origin.y, height),
        dx: f32::from(now.x - start.x),
        dy: f32::from(now.y - start.y),
        width,
        height,
    }
}

fn held(modifiers: &Modifiers) -> Held {
    Held {
        shift: modifiers.shift,
        alt: modifiers.alt,
        ctrl: modifiers.control,
        cmd: modifiers.platform,
        secondary: if MAC {
            modifiers.platform
        } else {
            modifiers.control
        },
    }
}

fn name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        _ => "other",
    }
}

fn style(cursor: Cursor) -> CursorStyle {
    match cursor {
        Cursor::Default => CursorStyle::Arrow,
        Cursor::Pointer => CursorStyle::PointingHand,
        Cursor::Text => CursorStyle::IBeam,
        Cursor::Crosshair => CursorStyle::Crosshair,
        Cursor::Grab => CursorStyle::OpenHand,
        Cursor::Grabbing => CursorStyle::ClosedHand,
        Cursor::Move => CursorStyle::ClosedHand,
        Cursor::EwResize => CursorStyle::ResizeLeftRight,
        Cursor::NsResize => CursorStyle::ResizeUpDown,
        Cursor::NwseResize => CursorStyle::ResizeUpLeftDownRight,
        Cursor::NeswResize => CursorStyle::ResizeUpRightDownLeft,
        Cursor::ColResize => CursorStyle::ResizeColumn,
        Cursor::RowResize => CursorStyle::ResizeRow,
        Cursor::NotAllowed => CursorStyle::OperationNotAllowed,
    }
}
