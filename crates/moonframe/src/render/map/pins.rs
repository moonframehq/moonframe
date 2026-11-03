//! A map's pins: a dot on the place, a disc on it with a label or icon,
//! or a balloon above it whose tail's tip marks it, as native maps draw a
//! place. A selected pin is drawn larger and last, above the others; a
//! click sends the pin's `msg`, else the map's `select` with its index.

use {
    super::Frame,
    crate::{color::ThemeColor, render::pointer::Sender},
    gpui_kit::{
        component::{ActiveTheme, Icon},
        *,
    },
    moonframe_core::{Data, Map, Pin, PinShape, with_fields},
    std::collections::BTreeMap,
};

/// A disc and a balloon's head, and a dot, in pixels across.
const DISC: f32 = 26.0;
const DOT: f32 = 12.0;
/// A balloon's tail below its head, in pixels.
const TAIL: f32 = 9.0;
/// How much larger a selected pin is.
const SELECTED: f32 = 1.25;

/// The pins in `frame`, selected ones last so they sit above the rest.
pub(super) fn pins(map: &Map, frame: Frame, sender: &Sender, cx: &App) -> Vec<AnyElement> {
    let mut order: Vec<usize> = (0..map.pins.len()).collect();
    order.sort_by_key(|&index| map.pins[index].selected);
    order
        .into_iter()
        .filter_map(|index| pin(map, index, frame, sender, cx))
        .collect()
}

fn pin(map: &Map, index: usize, frame: Frame, sender: &Sender, cx: &App) -> Option<AnyElement> {
    let theme = cx.theme();
    let pin = &map.pins[index];
    let (left, top) = frame.at(pin.lat, pin.lon);
    let reach = DISC * SELECTED + TAIL;
    let outside = left < -reach
        || top < -reach
        || f64::from(left) > frame.size.0 + f64::from(reach)
        || f64::from(top) > frame.size.1 + f64::from(reach);
    if outside {
        return None;
    }
    let shape = pin
        .shape
        .unwrap_or(if pin.label.is_some() || pin.icon.is_some() {
            PinShape::Disc
        } else {
            PinShape::Dot
        });
    let grow = if pin.selected {
        SELECTED
    } else {
        1.0
    };
    let fill = pin
        .color
        .map_or(theme.foreground, |color| color.hsla(theme));
    let ink = theme.background;
    let element = match shape {
        PinShape::Dot => {
            let across = DOT * grow;
            disc(across, fill, ink)
                .left(px(left - across / 2.0))
                .top(px(top - across / 2.0))
        },
        PinShape::Disc => {
            let across = DISC * grow;
            disc(across, fill, ink)
                .left(px(left - across / 2.0))
                .top(px(top - across / 2.0))
                .children(face(pin, ink, grow))
        },
        PinShape::Balloon => {
            let across = DISC * grow;
            let tail = TAIL * grow;
            // The tail's tip on the place: the head sits above it.
            div()
                .absolute()
                .left(px(left - across / 2.0))
                .top(px(top - across - tail))
                .w(px(across))
                .h(px(across + tail))
                .child(balloon(across, tail, fill, ink))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size(px(across))
                        .flex()
                        .items_center()
                        .justify_center()
                        .children(face(pin, ink, grow)),
                )
        },
    };
    let mut element = element.id(("pin", index));
    let clicked = match (&pin.msg, &map.select) {
        (Some(msg), _) => Some(msg.clone()),
        (None, Some(select)) => {
            let at = BTreeMap::from([("pin".to_owned(), Data::Number((index + 1) as f64))]);
            Some(with_fields(select, at))
        },
        (None, None) => None,
    };
    if let Some(msg) = clicked {
        let sender = sender.clone();
        element =
            element
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    sender.send(msg.clone(), window, cx);
                });
    }
    Some(element.into_any_element())
}

/// A round marker `across` pixels wide, ringed in `ink`.
fn disc(across: f32, fill: Hsla, ink: Hsla) -> Div {
    div()
        .absolute()
        .size(px(across))
        .rounded_full()
        .border_2()
        .border_color(ink)
        .bg(fill)
        .shadow_md()
        .flex()
        .items_center()
        .justify_center()
}

/// What a pin shows inside: its icon, else its label.
fn face(pin: &Pin, ink: Hsla, grow: f32) -> Option<AnyElement> {
    if let Some(icon) = &pin.icon {
        return Some(
            Icon::default()
                .path(icon.path())
                .size(px(14.0 * grow))
                .text_color(ink)
                .into_any_element(),
        );
    }
    pin.label.clone().map(|label| {
        div()
            .text_size(px(12.0 * grow))
            .font_weight(FontWeight::BOLD)
            .text_color(ink)
            .child(label)
            .into_any_element()
    })
}

/// A balloon's body: a round head with a tail narrowing to a point under
/// it, ringed in `ink`, over a soft shadow.
fn balloon(across: f32, tail: f32, fill: Hsla, ink: Hsla) -> Canvas<()> {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let radius = across / 2.0;
            let x = f32::from(bounds.origin.x) + radius;
            let y = f32::from(bounds.origin.y) + radius;
            let body = |grow: f32, dy: f32| {
                let r = radius + grow;
                // Where the tail leaves the head, either side, low on it.
                let angle = 0.62_f32;
                let (sin, cos) = angle.sin_cos();
                let mut path = PathBuilder::fill();
                path.move_to(point(px(x - r * sin), px(y + dy + r * cos)));
                path.arc_to(
                    point(px(r), px(r)),
                    px(0.),
                    true,
                    true,
                    point(px(x + r * sin), px(y + dy + r * cos)),
                );
                path.line_to(point(px(x), px(y + dy + radius + tail + grow * 0.5)));
                path.close();
                path.build().ok()
            };
            if let Some(shadow) = body(1.5, 1.5) {
                window.paint_path(shadow, black().opacity(0.25));
            }
            if let Some(ring) = body(0.0, 0.0) {
                window.paint_path(ring, ink);
            }
            if let Some(inside) = body(-2.0, 0.0) {
                window.paint_path(inside, fill);
            }
        },
    )
    .absolute()
    .size_full()
}
