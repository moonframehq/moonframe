//! `ui.map`: tiles from the map's provider, its routes painted over them,
//! then its pins, the provider's credit and zoom buttons. Dragging pans
//! it; the wheel, a pinch, a double click and the buttons zoom it.

mod disk;
mod stand_in;
mod tiles;
mod view;

use {
    super::{
        Cx, Draw, Painter, Place,
        pointer::Sender,
        styling::{LookExt as _, StyleExt as _},
    },
    crate::color::ThemeColor,
    gpui_kit::{component::ActiveTheme, *},
    moonframe_core::{Data, Map, MapProvider, Part, Route, with_fields},
    std::{cell::Cell, collections::BTreeMap},
    tiles::Tile,
    view::Frame,
};

/// The most tiles one map draws: a huge box shows a coarser zoom's worth.
const MOST_TILES: usize = 400;
/// How many zooms coarser the layer under the whole map is: a handful of
/// its tiles cover the box, so they arrive long before the rest and leave
/// no blank while panning to somewhere new.
const BASE: u8 = 3;
/// How long a new tile takes to fade in.
const FADE: std::time::Duration = std::time::Duration::from_millis(200);
/// Tiles fetched beyond each edge of the box, so panning finds them ready.
const RING: i64 = 1;
/// A pin with a label, and a dot without one, in pixels across.
const PIN: f32 = 26.0;
const DOT: f32 = 12.0;
/// Zoom steps per pixel of wheel or trackpad scroll.
const SCROLL_ZOOM: f64 = 1.0 / 120.0;

thread_local! {
    /// Whether a task already waits for tiles to arrive, to draw again.
    static WAITING: Cell<bool> = const { Cell::new(false) };
}

impl<'w> Draw<'w> for Map {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Map, &self.look, false, place);
        let theme = cx.theme();
        let key = format!("{:?}/{}", painter.window, place.path);
        let fallback = (
            f64::from(self.width.unwrap_or(0.0)),
            f64::from(self.height.unwrap_or(0.0)),
        );
        let (frame, scale) = view::frame(self, &key, fallback);
        let most = view::most_zoom(self);
        let sender = Sender {
            app: cx.entity().downgrade(),
            owner: place.owner.to_owned(),
            to: self.to.clone(),
        };
        let (layer, missing) = tile_layer(self, frame, scale);
        if missing && !WAITING.get() {
            WAITING.set(true);
            let arrived = tiles::arrival();
            cx.spawn(async move |this, cx| {
                let _ = arrived.await;
                WAITING.set(false);
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        let route_color = look
            .base
            .color
            .map_or(theme.primary, |color| color.hsla(theme));
        let mut element = div()
            .id(place.id())
            .relative()
            .overflow_hidden()
            .flex_shrink_0()
            .bg(placeholder(self.provider))
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .cursor_grab()
            .child(layer)
            .child(routes(
                &self.routes,
                frame,
                route_color,
                theme.background,
                cx,
            ))
            .children(pins(self, frame, &sender, cx))
            .child(credit(self, cx))
            .child(zoom_buttons(
                &key,
                frame,
                most,
                self.moved.clone(),
                &sender,
                cx,
            ))
            .child(follow(&key, self.moved.clone(), &sender));
        element = match self.width {
            Some(width) => element.w(px(width)),
            None => element.w_full(),
        };
        element = match self.height {
            Some(height) => element.h(px(height)),
            None => element.h_full(),
        };
        if self.grow {
            element = element.flex_1();
        }
        element = interact(element, &key, frame, most, self.moved.clone(), &sender);
        element.into_any_element()
    }
}

/// The tiles under `frame`, and whether any is still on its way.
fn tile_layer(map: &Map, frame: Frame, scale: f32) -> (Div, bool) {
    let source = map.provider.source();
    let z = frame.zoom.round().clamp(0.0, f64::from(source.max_zoom)) as u8;
    let count = 1i64 << z;
    let size = 256.0 * (frame.zoom - f64::from(z)).exp2();
    let span = frame.span();
    let left = frame.center.0 * span - frame.size.0 / 2.0;
    let top = frame.center.1 * span - frame.size.1 / 2.0;
    let columns = (left / size).floor() as i64..=((left + frame.size.0) / size).floor() as i64;
    let rows = (top / size).floor().max(0.0) as i64
        ..=((top + frame.size.1) / size)
            .floor()
            .min(count as f64 - 1.0) as i64;
    let retina = scale > 1.5 && tiles::has_retina(map.provider);
    let key = map.key.as_deref();
    let at = |column: i64, row: i64| Tile {
        provider: map.provider,
        retina,
        z,
        x: column.rem_euclid(count) as u32,
        y: row as u32,
    };
    // The ring first: requests are served newest first, so what shows
    // (asked for below) comes before it.
    let ring_rows = (*rows.start() - RING).max(0)..=(*rows.end() + RING).min(count - 1);
    let ring_columns = *columns.start() - RING..=*columns.end() + RING;
    let with_ring = ring_rows.clone().count() * ring_columns.clone().count() <= MOST_TILES;
    if with_ring {
        for row in ring_rows.clone() {
            for column in ring_columns.clone() {
                if !rows.contains(&row) || !columns.contains(&column) {
                    tiles::prefetch(&at(column, row), key);
                }
            }
        }
    }
    let (mut layer, base) = base_layer(map, frame, z);
    let mut missing = false;
    let mut drawn = 0;
    let (all_rows, all_columns) = if with_ring {
        (ring_rows, ring_columns)
    } else {
        (rows.clone(), columns.clone())
    };
    for row in all_rows {
        for column in all_columns.clone() {
            drawn += 1;
            if drawn > MOST_TILES {
                return (layer, missing);
            }
            // Whole pixels, so neighbours meet without a seam.
            let x0 = (column as f64 * size - left).floor();
            let y0 = (row as f64 * size - top).floor();
            let x1 = ((column + 1) as f64 * size - left).floor();
            let y1 = ((row + 1) as f64 * size - top).floor();
            let tile = at(column, row);
            let cell = div()
                .absolute()
                .overflow_hidden()
                .left(px(x0 as f32))
                .top(px(y0 as f32))
                .w(px((x1 - x0) as f32))
                .h(px((y1 - y0) as f32));
            if !rows.contains(&row) || !columns.contains(&column) {
                // Just outside the box, clipped away: drawn so GPUI has
                // decoded it before a pan brings it in.
                if let Some(file) = tiles::ready(&tile) {
                    // Counted as shown: in view, it needs no fade.
                    stand_in::settled(&file);
                    layer = layer.child(cell.child(img(file).size_full()));
                }
                continue;
            }
            if let Some(file) = tiles::tile(&tile, key) {
                // Until it has had time to decode, what stood in for it
                // stays under it, so it never blinks blank; the next
                // redraw after that drops it.
                let fresh = !stand_in::settled(&file);
                let under = fresh.then(|| stand_in::stand_in(&tile, x1 - x0)).flatten();
                let picture = img(file).size_full();
                // A new tile fades in over what stood in for it.
                let picture = if fresh {
                    let id = format!("{}/{}/{}/{}", tile.provider.name(), tile.z, tile.x, tile.y);
                    picture
                        .with_animation(
                            ElementId::Name(id.into()),
                            Animation::new(FADE).with_easing(ease_out_quint()),
                            |picture, delta| picture.opacity(delta),
                        )
                        .into_any_element()
                } else {
                    picture.into_any_element()
                };
                layer = layer.child(cell.children(under).child(picture));
                continue;
            }
            missing = true;
            // Until it arrives: the same spot from another zoom, else the
            // base layer under everything shows through.
            if let Some(stand_in) = stand_in::stand_in(&tile, x1 - x0) {
                layer = layer.child(cell.child(stand_in));
            }
        }
    }
    // Asked for last, so they arrive first: newest requests go first.
    for tile in &base {
        let _ = tiles::tile(tile, key);
    }
    (layer, missing)
}

/// The tiles [`BASE`] zooms coarser than `z` under the whole box, at
/// standard density (it is enlarged anyway), as the layer the map's own
/// tiles are drawn over; and those tiles, to ask for.
fn base_layer(map: &Map, frame: Frame, z: u8) -> (Div, Vec<Tile>) {
    let layer = div().absolute().size_full();
    let base = z.saturating_sub(BASE);
    if base == z {
        return (layer, Vec::new());
    }
    let count = 1i64 << base;
    let size = 256.0 * (frame.zoom - f64::from(base)).exp2();
    let span = frame.span();
    let left = frame.center.0 * span - frame.size.0 / 2.0;
    let top = frame.center.1 * span - frame.size.1 / 2.0;
    let columns = (left / size).floor() as i64..=((left + frame.size.0) / size).floor() as i64;
    let rows = (top / size).floor().max(0.0) as i64
        ..=((top + frame.size.1) / size)
            .floor()
            .min(count as f64 - 1.0) as i64;
    let mut tiles = Vec::new();
    let mut images = Vec::new();
    for row in rows {
        for column in columns.clone() {
            let tile = Tile {
                provider: map.provider,
                retina: false,
                z: base,
                x: column.rem_euclid(count) as u32,
                y: row as u32,
            };
            if let Some(file) = tiles::ready(&tile) {
                let x0 = (column as f64 * size - left).floor();
                let y0 = (row as f64 * size - top).floor();
                images.push(
                    img(file)
                        .absolute()
                        .left(px(x0 as f32))
                        .top(px(y0 as f32))
                        .w(px(size.ceil() as f32 + 1.0))
                        .h(px(size.ceil() as f32 + 1.0)),
                );
            }
            tiles.push(tile);
        }
    }
    (layer.children(images), tiles)
}

/// What shows where nothing has arrived yet: a land color close to the
/// provider's own.
fn placeholder(provider: MapProvider) -> Hsla {
    let dark = matches!(
        provider,
        MapProvider::CartoDark | MapProvider::StadiaSmoothDark
    );
    if dark {
        rgb(0x262626).into()
    } else {
        rgb(0xf2efe9).into()
    }
}

/// The routes, each over a casing in the background color so it reads
/// on any tile.
fn routes(routes: &[Route], frame: Frame, fallback: Hsla, casing: Hsla, cx: Cx) -> Canvas<()> {
    let theme = cx.theme();
    let lines: Vec<(Vec<(f32, f32)>, Hsla, f32, bool)> = routes
        .iter()
        .map(|route| {
            let corners = route
                .points
                .iter()
                .map(|&[lat, lon]| frame.at(lat, lon))
                .collect();
            let color = route.color.map_or(fallback, |color| color.hsla(theme));
            (corners, color, route.stroke, route.dashed)
        })
        .collect();
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            for (corners, color, stroke, dashed) in &lines {
                let at =
                    |&(x, y): &(f32, f32)| point(bounds.origin.x + px(x), bounds.origin.y + px(y));
                let points: Vec<Point<Pixels>> = corners.iter().map(at).collect();
                for (width, paint, dash) in [
                    (stroke + 3.0, casing.opacity(0.85), false),
                    (*stroke, *color, *dashed),
                ] {
                    let mut path = PathBuilder::stroke(px(width));
                    if dash {
                        path = path.dash_array(&[px(stroke * 2.0), px(stroke * 1.5)]);
                    }
                    path.move_to(points[0]);
                    for corner in &points[1..] {
                        path.line_to(*corner);
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, paint);
                    }
                }
            }
        },
    )
    .absolute()
    .size_full()
}

/// The pins in the box, each a disc with its label (a dot without one);
/// a click sends `select` with its index.
fn pins(map: &Map, frame: Frame, sender: &Sender, cx: Cx) -> Vec<AnyElement> {
    let theme = cx.theme();
    map.pins
        .iter()
        .enumerate()
        .filter_map(|(index, pin)| {
            let (left, top) = frame.at(pin.lat, pin.lon);
            let outside = left < -PIN
                || top < -PIN
                || f64::from(left) > frame.size.0 + f64::from(PIN)
                || f64::from(top) > frame.size.1 + f64::from(PIN);
            if outside {
                return None;
            }
            let fill = pin
                .color
                .map_or(theme.foreground, |color| color.hsla(theme));
            let across = if pin.label.is_some() {
                PIN
            } else {
                DOT
            };
            let mut element = div()
                .id(("pin", index))
                .absolute()
                .left(px(left - across / 2.0))
                .top(px(top - across / 2.0))
                .size(px(across))
                .rounded_full()
                .border_2()
                .border_color(theme.background)
                .bg(fill)
                .shadow_md()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.background)
                .children(pin.label.clone());
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
                element = element.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    move |_, window, cx| {
                        cx.stop_propagation();
                        sender.send(msg.clone(), window, cx);
                    },
                );
            }
            Some(element.into_any_element())
        })
        .collect()
}

/// The provider's credit in the bottom corner, and what went wrong
/// fetching its tiles, if anything did.
fn credit(map: &Map, cx: Cx) -> Div {
    let theme = cx.theme();
    let problem = tiles::failure(map.provider);
    div()
        .absolute()
        .bottom_0()
        .right_0()
        .px_1()
        .rounded_tl(px(4.))
        .bg(theme.background.opacity(0.8))
        .text_color(theme.muted_foreground)
        .text_size(px(10.))
        .flex()
        .gap_2()
        .children(problem.map(|problem| div().text_color(theme.danger).child(problem)))
        .child(map.provider.source().attribution)
}

/// Zoom in and out, in the top corner.
fn zoom_buttons(
    key: &str,
    frame: Frame,
    most: f64,
    moved: Option<Data>,
    sender: &Sender,
    cx: Cx,
) -> Div {
    let theme = cx.theme();
    let button = |label: &'static str, steps: f64| {
        let (key, moved, sender) = (key.to_owned(), moved.clone(), sender.clone());
        div()
            .id(label)
            .size(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(theme.foreground)
            .hover(|style| style.bg(theme.secondary_hover))
            .child(label)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| {
                // Around the middle of the box.
                let middle = view::middle(&key);
                let current = view::current(&key, frame);
                let zoomed = view::zoom(&key, current, steps, middle, most);
                tell(moved.as_ref(), zoomed, &sender, window, cx);
                window.refresh();
            })
    };
    div()
        .absolute()
        .top_2()
        .right_2()
        .flex()
        .flex_col()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .shadow_sm()
        .overflow_hidden()
        .child(button("+", 1.0))
        .child(div().h(px(1.)).bg(theme.border))
        .child(button("−", -1.0))
}

/// Pressing, the wheel, a pinch and a double click.
fn interact(
    element: Stateful<Div>,
    key: &str,
    frame: Frame,
    most: f64,
    moved: Option<Data>,
    sender: &Sender,
) -> Stateful<Div> {
    let (press_key, wheel_key, pinch_key) = (key.to_owned(), key.to_owned(), key.to_owned());
    let (wheel_moved, wheel_sender) = (moved.clone(), sender.clone());
    let (pinch_moved, pinch_sender) = (moved, sender.clone());
    element
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            // The map takes the press: a window dragged by its background
            // does not move.
            cx.stop_propagation();
            if event.click_count == 2 {
                let current = view::current(&press_key, frame);
                view::zoom(&press_key, current, 1.0, event.position, most);
                window.refresh();
                return;
            }
            view::press(
                &press_key,
                view::current(&press_key, frame),
                event.position,
                window,
            );
        })
        .on_scroll(move |event, window, cx| {
            cx.stop_propagation();
            let delta = event.delta.pixel_delta(px(40.));
            let steps = -f64::from(delta.y) * SCROLL_ZOOM;
            if steps == 0.0 {
                return;
            }
            let current = view::current(&wheel_key, frame);
            let zoomed = view::zoom(&wheel_key, current, steps, event.position, most);
            tell(wheel_moved.as_ref(), zoomed, &wheel_sender, window, cx);
            window.refresh();
        })
        .on_pinch(move |event, window, cx| {
            cx.stop_propagation();
            let steps = f64::from(1.0 + event.delta).max(0.01).log2();
            let current = view::current(&pinch_key, frame);
            let zoomed = view::zoom(&pinch_key, current, steps, event.position, most);
            tell(pinch_moved.as_ref(), zoomed, &pinch_sender, window, cx);
            window.refresh();
        })
}

/// A canvas over the box that keeps its bounds and, while the map is the
/// one pressed, follows the pointer across the whole window.
fn follow(key: &str, moved: Option<Data>, sender: &Sender) -> impl IntoElement {
    let key = key.to_owned();
    let sender = sender.clone();
    canvas(
        {
            let key = key.clone();
            move |bounds, window, _| {
                if view::laid(&key, bounds, window.scale_factor()) {
                    window.refresh();
                }
            }
        },
        move |_, (), window, _| {
            if !view::dragging(&key, window) {
                return;
            }
            let moving = key.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
                if phase != DispatchPhase::Capture || !view::dragging(&moving, window) {
                    return;
                }
                if event.pressed_button.is_none() {
                    view::release(&moving);
                    return;
                }
                if view::drag_to(event.position).is_some() {
                    window.refresh();
                }
            });
            let (released, moved, sender) = (key.clone(), moved.clone(), sender.clone());
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                    return;
                }
                if !view::dragging(&released, window) {
                    return;
                }
                if view::release(&released)
                    && let Some(frame) = view::moved(&released)
                {
                    tell(moved.as_ref(), frame, &sender, window, cx);
                }
            });
        },
    )
    .absolute()
    .size_full()
}

/// Sends `move` with where the map now looks.
fn tell(moved: Option<&Data>, frame: Frame, sender: &Sender, window: &mut Window, cx: &mut App) {
    let Some(msg) = moved else {
        return;
    };
    let [lat, lon] = frame.middle();
    let round = |value: f64, places: i32| {
        let scale = 10f64.powi(places);
        (value * scale).round() / scale
    };
    let fields = BTreeMap::from([
        ("lat".to_owned(), Data::Number(round(lat, 6))),
        ("lon".to_owned(), Data::Number(round(lon, 6))),
        ("zoom".to_owned(), Data::Number(round(frame.zoom, 2))),
    ]);
    sender.send(with_fields(msg, fields), window, cx);
}
