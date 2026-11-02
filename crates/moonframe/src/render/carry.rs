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
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

/// How faint a row is while it is carried.
const CARRIED: f32 = 0.4;
/// Moves over a target shorter than this, in pixels, are not sent.
const STILL: f32 = 2.0;
/// The box's corners when the stylesheet gives none.
const RADIUS: f32 = 8.0;

/// What a drag carries: the mod's data, the mod that drew it, and the
/// place of the row it came from.
struct Carried {
    data: Data,
    from: String,
    path: String,
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
) -> Stateful<Div> {
    if let Some(data) = &pointer.carry {
        element = carry(element, data, painter, place, cx);
    }
    if pointer.accept.is_some() {
        element = accept(element, pointer, place, cx);
    }
    element
}

/// A press that moves a few pixels lifts the row; while it is carried the
/// row stays in place, faint.
fn carry(
    element: Stateful<Div>,
    data: &Data,
    painter: &Painter<'_, '_>,
    place: &Place<'_>,
    cx: Cx,
) -> Stateful<Div> {
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let laid = bounds.clone();
    let measure = canvas(move |found, _, _| laid.set(found), |_, (), _, _| {})
        .absolute()
        .size_full();
    let look = painter.look(Part::Carry, &Look::default(), false, place);
    let carried = Carried {
        data: data.clone(),
        from: place.owner.to_owned(),
        path: place.path.clone(),
    };
    let lifted = cx.has_active_drag()
        && CARRYING.with_borrow(|carrying| carrying.as_deref() == Some(place.path.as_str()));
    let element = element
        .child(measure)
        .on_drag(carried, move |carried, _, _, cx| {
            CARRYING.set(Some(carried.path.clone()));
            let size = bounds.get().size;
            let look = look.clone();
            cx.new(|_| Ghost { size, look })
        });
    if lifted {
        element.opacity(CARRIED)
    } else {
        element
    }
}

/// Sends `accept` as something carried moves over the row, leaves it, and
/// is dropped on it.
fn accept(element: Stateful<Div>, pointer: &Pointer, place: &Place<'_>, cx: Cx) -> Stateful<Div> {
    let sender = Sender {
        app: cx.entity().downgrade(),
        owner: place.owner.to_owned(),
        to: pointer.to.clone(),
    };
    // The row's box as last laid out (an `over` redraws the row, so the
    // drop meets a new one), and where the pointer was when `over` was last
    // sent: `None` while it is outside.
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let laid = bounds.clone();
    let measure = canvas(move |found, _, _| laid.set(found), |_, (), _, _| {})
        .absolute()
        .size_full();
    let last = Rc::new(Cell::new(None::<Point<Pixels>>));
    let (moved, moving, mover) = (pointer.clone(), last.clone(), sender.clone());
    let dropped = pointer.clone();
    element
        .child(measure)
        .on_drag_move::<Carried>(move |event, window, cx| {
            let at = event.event.position;
            let phase = match (event.bounds.contains(&at), moving.get()) {
                (true, Some(before)) if (at - before).magnitude() < f64::from(STILL) => return,
                (true, _) => {
                    moving.set(Some(at));
                    CarryPhase::Over
                },
                (false, Some(_)) => {
                    moving.set(None);
                    CarryPhase::Leave
                },
                (false, None) => return,
            };
            let carried = event.drag(cx);
            let (data, from) = (carried.data.clone(), carried.from.clone());
            let at = travel(event.bounds, at, at);
            if let Some(msg) = moved.accept_message(phase, &data, &from, &at) {
                mover.send(msg, window, cx);
            }
        })
        .on_drop::<Carried>(move |carried, window, cx| {
            last.set(None);
            CARRYING.set(None);
            let now = window.mouse_position();
            let at = travel(bounds.get(), now, now);
            if let Some(msg) =
                dropped.accept_message(CarryPhase::Drop, &carried.data, &carried.from, &at)
            {
                sender.send(msg, window, cx);
            }
        })
}
