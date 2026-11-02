//! A row's or column's `carry` and `accept`: dragging things inside the
//! app. Dragging a row with `carry` lifts a box of its size (the
//! stylesheet's `carry` part) that follows the pointer while the row dims;
//! a row with `accept` hears it move over, leave, and drop, as GPUI's own
//! drag and drop delivers them. The innermost target under the pointer
//! takes the drop.

use {
    super::{
        Cx, Painter, Place,
        pointer::{Sender, travel},
        styling::{LookExt as _, StyleExt as _},
    },
    crate::color::ThemeColor as _,
    gpui_kit::{component::ActiveTheme as _, *},
    moonframe_core::{Border, CarryPhase, Data, Look, Part, Pointer, Resolved},
    std::{cell::RefCell, rc::Rc},
};

mod session;
mod target;

use {session::Hovers, target::Measured};

/// How faint a row is while it is carried.
const CARRIED: f32 = 0.4;
/// Moves over a target shorter than this, in pixels, are not sent.
const STILL: f32 = 2.0;
/// The box's corners when the stylesheet gives none.
const RADIUS: f32 = 8.0;

/// What a drag carries: the mod's data, the mod that drew it, and the
/// place of the row it came from.
#[derive(Clone)]
struct Carried {
    data: Data,
    from: String,
    path: String,
    /// Lives for the drag, rather than for one rendered target.
    visits: Rc<RefCell<Hovers<(WindowId, String), Visit>>>,
}

#[derive(Clone)]
struct Visit {
    sender: Sender,
    pointer: Pointer,
    bounds: Bounds<Pixels>,
    at: Point<Pixels>,
}

impl Carried {
    fn finish(&self, winner: Option<&(WindowId, String)>, cx: &mut App) {
        let visits = self.visits.borrow_mut().finish(winner);
        for ((id, _), visit) in visits {
            let Some(window) = cx
                .windows()
                .into_iter()
                .find(|window| window.window_id() == id)
            else {
                continue;
            };
            let at = travel(visit.bounds, visit.at, visit.at);
            if let Some(msg) =
                visit
                    .pointer
                    .accept_message(CarryPhase::Leave, &self.data, &self.from, &at)
            {
                // Cleanup may originate in another window, or while the
                // drop's window is borrowed. Deliver after that event.
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, window, cx| visit.sender.send(msg, window, cx));
                });
            }
        }
    }
}

thread_local! {
    /// The place of the row being carried, to draw it faint.
    static CARRYING: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The box that follows the pointer: the carried row's size, in the
/// `carry` look.
struct Ghost {
    size: Size<Pixels>,
    look: Resolved,
    carried: Carried,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let base = &self.look.base;
        let mut ghost = div()
            .w(self.size.width)
            .h(self.size.height)
            .rounded(px(RADIUS))
            .bg(theme.popover.opacity(0.9))
            .border_color(theme.primary)
            .shadow_lg();
        if let Some(color) = base.border_color {
            ghost = ghost.border_color(color.hsla(theme));
        }
        ghost
            .look(&self.look, theme)
            .edges(base.border.or(Some(Border::All)), base, theme)
    }
}

/// `element` with `pointer`'s `carry` and `accept`.
pub(super) fn attach(
    mut element: Stateful<Div>,
    pointer: &Pointer,
    painter: &Painter<'_, '_>,
    place: &Place<'_>,
    cx: Cx,
) -> AnyElement {
    if pointer.carry.is_none() && pointer.accept.is_none() {
        return element.into_any_element();
    }
    let measured = Measured::state();
    if let Some(data) = &pointer.carry {
        element = carry(element, data, &measured, painter, place, cx);
    }
    if pointer.accept.is_some() {
        element = accept(element, pointer, &measured, place, cx);
    }
    Measured::new(element, measured).into_any_element()
}

/// A press that moves a few pixels lifts the row; while it is carried the
/// row stays in place, faint.
fn carry(
    element: Stateful<Div>,
    data: &Data,
    measured: &Rc<RefCell<Option<Hitbox>>>,
    painter: &Painter<'_, '_>,
    place: &Place<'_>,
    cx: Cx,
) -> Stateful<Div> {
    let measured = measured.clone();
    let look = painter.look(Part::Carry, &Look::default(), false, place);
    let carried = Carried {
        data: data.clone(),
        from: place.owner.to_owned(),
        path: place.path.clone(),
        visits: Rc::new(RefCell::new(Hovers::default())),
    };
    let lifted = cx.has_active_drag()
        && CARRYING.with_borrow(|carrying| carrying.as_deref() == Some(place.path.as_str()));
    let element = element.on_drag(carried, move |carried, _, _, cx| {
        CARRYING.set(Some(carried.path.clone()));
        let size = measured
            .borrow()
            .as_ref()
            .map(|hitbox| hitbox.bounds.size)
            .unwrap_or_default();
        let look = look.clone();
        cx.new(|cx| {
            // Releasing the native drag also covers cancellation and
            // dropping outside every accepting element.
            cx.on_release(|ghost: &mut Ghost, cx| {
                ghost.carried.finish(None, cx);
                CARRYING.set(None);
            })
            .detach();
            Ghost {
                size,
                look,
                carried: carried.clone(),
            }
        })
    });
    if lifted {
        element.opacity(CARRIED)
    } else {
        element
    }
}

/// Sends `accept` as something carried moves over the row, leaves it, and
/// is dropped on it.
fn accept(
    element: Stateful<Div>,
    pointer: &Pointer,
    measured: &Rc<RefCell<Option<Hitbox>>>,
    place: &Place<'_>,
    cx: Cx,
) -> Stateful<Div> {
    let sender = Sender {
        app: cx.entity().downgrade(),
        owner: place.owner.to_owned(),
        to: pointer.to.clone(),
    };
    let (moved, moving, mover) = (pointer.clone(), measured.clone(), sender.clone());
    let (path, dropped_path) = (place.path.clone(), place.path.clone());
    let dropped = pointer.clone();
    let measured = measured.clone();
    element
        .on_drag_move::<Carried>(move |event, window, cx| {
            let Some(hitbox) = moving.borrow().clone() else {
                return;
            };
            let at = event.event.position;
            let carried = event.drag(cx);
            let key = (window.window_handle().window_id(), path.clone());
            let visit = Visit {
                sender: mover.clone(),
                pointer: moved.clone(),
                bounds: hitbox.bounds,
                at,
            };
            let phase = if hitbox.is_hovered(window) {
                if !carried.visits.borrow_mut().over(key, at, visit) {
                    return;
                }
                CarryPhase::Over
            } else if carried.visits.borrow_mut().leave(&key).is_some() {
                CarryPhase::Leave
            } else {
                return;
            };
            let (data, from) = (carried.data.clone(), carried.from.clone());
            let at = travel(hitbox.bounds, at, at);
            if let Some(msg) = moved.accept_message(phase, &data, &from, &at) {
                mover.send(msg, window, cx);
            }
        })
        .on_drop::<Carried>(move |carried, window, cx| {
            let Some(hitbox) = measured.borrow().clone() else {
                return;
            };
            let key = (window.window_handle().window_id(), dropped_path.clone());
            carried.finish(Some(&key), cx);
            CARRYING.set(None);
            let now = window.mouse_position();
            let at = travel(hitbox.bounds, now, now);
            if let Some(msg) =
                dropped.accept_message(CarryPhase::Drop, &carried.data, &carried.from, &at)
            {
                sender.send(msg, window, cx);
            }
        })
}
