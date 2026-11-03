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
const DISC: f32 = 28.0;
const DOT: f32 = 12.0;
/// A balloon's tail below its head, in pixels.
const TAIL: f32 = 10.0;
/// A pin's fill when it gives no color.
const PIN_FILL: Rgba = Rgba {
    r: 0.11,
    g: 0.11,
    b: 0.12,
    a: 1.0,
};
/// How much larger a selected pin is.
const SELECTED: f32 = 1.25;

/// The pins in `frame`, selected ones last so they sit above the rest.
pub(super) fn pins(map: &Map, frame: Frame, sender: &Sender, cx: &App) -> Vec<AnyElement> {
    let mut order: Vec<usize> = (0..map.pins.len()).collect();
    order.sort_by_key(|&index| map.pins[index].selected);
    order
        .into_iter()
        .flat_map(|index| {
            let pin = &map.pins[index];
            let (left, top) = frame.at(pin.lat, pin.lon);
            frame
                .copies(left, left, DISC * SELECTED + TAIL)
                .filter_map(move |copy| {
                    let left = left + (copy as f64 * frame.span()) as f32;
                    self::pin(map, index, frame, sender, cx, copy, (left, top))
                })
        })
        .collect()
}

fn pin(
    map: &Map,
    index: usize,
    frame: Frame,
    sender: &Sender,
    cx: &App,
    copy: i64,
    (left, top): (f32, f32),
) -> Option<AnyElement> {
    let theme = cx.theme();
    let pin = &map.pins[index];
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
    // Dark with a white rim and face, whatever the app's theme: the map
    // under them is light on most providers, and the rim keeps them clear
    // on dark ones.
    let fill = pin.color.map_or(PIN_FILL.into(), |color| color.hsla(theme));
    let ink: Hsla = white();
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
    let mut element = element.id(format!("pin/{index}/{copy}"));
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

/// A balloon's body: a round head ringed in `ink`, with a short tail
/// tapering to a point `tail` pixels under it, the place; a soft shadow
/// under the body and a small one on the ground at the tip.
fn balloon(across: f32, tail: f32, fill: Hsla, ink: Hsla) -> Canvas<()> {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let radius = across / 2.0;
            let center = (
                f32::from(bounds.origin.x) + radius,
                f32::from(bounds.origin.y) + radius,
            );
            let tip = center.1 + radius + tail;
            // The ground shadow: a flat oval where the tip meets the map.
            if let Some(ground) = polygon(&oval((center.0, tip), radius * 0.45, 2.5)) {
                window.paint_path(ground, black().opacity(0.22));
            }
            let shapes = [
                // Shadow, a touch larger and lower.
                (
                    outline(center, radius + 1.5, tip + 1.5, 1.5),
                    black().opacity(0.18),
                ),
                // The rim.
                (outline(center, radius, tip, 0.0), ink),
                // The head and tail inside it.
                (outline(center, radius - 2.0, tip - 3.0, 0.0), fill),
            ];
            for (corners, color) in shapes {
                if let Some(path) = polygon(&corners) {
                    window.paint_path(path, color);
                }
            }
        },
    )
    .absolute()
    .size_full()
}

/// The balloon's outline: around the head from where the tail leaves it
/// on the right, over the top, to where it leaves on the left, then down
/// to the tip. `drop` moves it all down (for the shadow).
fn outline(center: (f32, f32), radius: f32, tip: f32, drop: f32) -> Vec<(f32, f32)> {
    // How far round from straight down the tail leaves the head.
    const LEAVES: f32 = 0.5;
    const STEPS: usize = 40;
    let sweep = std::f32::consts::TAU - 2.0 * LEAVES;
    let mut corners: Vec<(f32, f32)> = (0..=STEPS)
        .map(|step| {
            // 0 is straight down; growing goes right, up, then left.
            let angle = LEAVES + sweep * step as f32 / STEPS as f32;
            (
                center.0 + radius * angle.sin(),
                center.1 + drop + radius * angle.cos(),
            )
        })
        .collect();
    corners.push((center.0, tip + drop));
    corners
}

/// An oval around `center`, `wide` and `high` pixels from it.
fn oval(center: (f32, f32), wide: f32, high: f32) -> Vec<(f32, f32)> {
    (0..24)
        .map(|step| {
            let angle = std::f32::consts::TAU * step as f32 / 24.0;
            (center.0 + wide * angle.cos(), center.1 + high * angle.sin())
        })
        .collect()
}

/// A filled path through `corners`, closed.
fn polygon(corners: &[(f32, f32)]) -> Option<Path<Pixels>> {
    let (&(x, y), rest) = corners.split_first()?;
    let mut path = PathBuilder::fill();
    path.move_to(point(px(x), px(y)));
    for &(x, y) in rest {
        path.line_to(point(px(x), px(y)));
    }
    path.close();
    path.build().ok()
}
