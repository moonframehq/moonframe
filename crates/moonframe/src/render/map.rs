//! `ui.map`: tiles from the map's provider, its routes painted over them,
//! then its pins, the provider's credit and zoom buttons. Dragging pans
//! it; the wheel, a pinch, a double click and the buttons zoom it.

mod disk;
mod layer;
mod lines;
mod memory;
mod pins;
mod routing;
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
    layer::Grid,
    moonframe_core::{Data, Map, Part, with_fields},
    std::{cell::Cell, collections::BTreeMap},
    view::Frame,
};

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
        let grid = Grid::new(view::origin(&key), scale);
        memory::begin(painter.window);
        let (layer, tiles_missing) = layer::tile_layer(self, frame, scale, grid);
        // Decoded tiles are bounded per window rather than kept for the run.
        let layer = match painter.window {
            Some(window) => layer.image_cache(memory::TileCache(window)),
            None => layer,
        };
        let (lines, ways) = lines::lines(self, frame, cx);
        let missing = tiles_missing || ways.waiting;
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
            .bg(layer::placeholder(self.provider))
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .cursor_grab()
            .child(layer)
            .child(lines.paint(route_color, theme.background))
            .children(pins::pins(self, frame, &sender, cx))
            .child(lines::announce(
                &key,
                ways.found,
                self.routed.clone(),
                &sender,
            ))
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
        move |_, (), window, cx| {
            memory::evict(window, cx);
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
