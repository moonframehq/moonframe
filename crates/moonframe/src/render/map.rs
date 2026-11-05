//! `ui.map`: tiles from the map's provider, its routes painted over them,
//! then its pins, the provider's credit and zoom buttons. Dragging pans
//! it; the wheel, a pinch, a two-finger double tap, a double click and the
//! buttons zoom it; two fingers turn it. Its tiles are drawn as sprites
//! with north up, and through [`rotated`] turned; everything over them is
//! placed by [`view::Frame`] and stays upright.

#[cfg(feature = "map-benchmark")]
mod benchmark;
#[cfg(not(feature = "map-benchmark"))]
mod benchmark {
    pub(super) const fn cell(_: bool) {}
}
mod disk;
mod layer;
mod lines;
mod memory;
mod pins;
mod rotated;
mod routing;
mod stand_in;
mod tiles;
mod view;
mod wake;

use {
    super::{
        Cx, Draw, Painter, Place,
        pointer::Sender,
        styling::{LookExt as _, StyleExt as _},
    },
    crate::{
        color::ThemeColor,
        gestures::{Gestures, gestures},
    },
    gpui_kit::{component::ActiveTheme, *},
    layer::Grid,
    moonframe_core::{Data, Map, Part, with_fields},
    std::{
        cell::RefCell,
        collections::{BTreeMap, HashMap},
        time::Instant,
    },
    view::Frame,
};

/// One breath of a pulsing pin's halo, before `animation_speed`.
const PULSE: std::time::Duration = std::time::Duration::from_millis(2200);

/// Zoom steps per notch of a mouse wheel.
const WHEEL_ZOOM: f64 = 1.0 / 3.0;

/// How much a pixel of trackpad scroll changes the scale: Apple Maps' figure.
const SCROLL_SCALE: f64 = 0.01;

thread_local! {
    /// Each map's live view, by key, in windows that keep the rest still.
    static LIVE: RefCell<HashMap<String, EntityId>> = RefCell::new(HashMap::new());
}

/// Draws the map `key` again, and of its window as little as it can.
fn redraw(key: &str, window: &mut Window, cx: &mut App) {
    match live_view(key) {
        Some(view) => cx.notify(view),
        None => window.refresh(),
    }
}

/// The live view of the map `key`, if it has one.
fn live_view(key: &str) -> Option<EntityId> {
    LIVE.with_borrow(|live| live.get(key).copied())
}

impl<'w> Draw<'w> for Map {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        #[cfg(feature = "map-benchmark")]
        benchmark::start(cx);
        let look = painter.look(Part::Map, &self.look, false, place);
        let theme = cx.theme();
        let key = format!("{:?}/{}", painter.window, place.path);
        let fallback = (
            f64::from(self.width.unwrap_or(0.0)),
            f64::from(self.height.unwrap_or(0.0)),
        );
        let live = painter.live_view(place);
        LIVE.with_borrow_mut(|views| match live {
            Some(view) => {
                views.insert(key.clone(), view);
            },
            None => {
                views.remove(&key);
            },
        });
        let stylesheet = painter.workspace.stylesheet();
        let (frame, scale) = view::frame(self, &key, fallback, painter.now, |pace| {
            stylesheet.scaled(pace)
        });
        let most = view::most_zoom(self);
        let sender = Sender {
            app: cx.entity().downgrade(),
            owner: place.owner.to_owned(),
            to: self.to.clone(),
        };
        let grid = Grid::new(view::origin(&key), scale);
        memory::begin(painter.window);
        let ahead = view::ahead(&key, painter.now, |zoom| {
            tiles::level(self.provider, zoom, scale)
        });
        let level = view::level(
            &key,
            frame.zoom,
            painter.now,
            stylesheet.scaled(layer::FADE),
            |zoom| tiles::level(self.provider, zoom, scale),
        );
        if view::gliding(&key) || level.1.is_some() {
            match live {
                Some(view) => painter.live_again(view),
                None => painter.animating = true,
            }
        }
        let (layer, tiles_missing) = if frame.turned() {
            rotated::layer(self, frame, scale, &key)
        } else {
            layer::tile_layer(self, frame, level, ahead, scale, grid)
        };
        // Its tiles set off now, not when the map arrives.
        if let Some(heading) = view::heading(&key) {
            layer::ask_for(self, heading, scale);
        }
        // Decoded tiles are bounded per window rather than kept for the run.
        let layer = match painter.window {
            Some(window) => layer.image_cache(memory::TileCache(window)),
            None => layer,
        };
        let (lines, ways) = lines::lines(self, frame, cx);
        let missing = tiles_missing || ways.waiting;
        #[cfg(feature = "map-benchmark")]
        benchmark::frame(level.0, tiles_missing);
        let target = live.unwrap_or_else(|| cx.entity_id());
        if missing {
            wake::arrival(target, cx);
        }
        // Only a map with a pulsing pin asks for frame after frame.
        let pulse = if self.pins.iter().any(|pin| pin.pulse) {
            painter.quiet_phase(PULSE)
        } else {
            None
        };
        if pulse.is_some() {
            wake::pulse(target, cx);
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
            .children(pins::pins(self, frame, &sender, pulse, cx))
            .child(lines::announce(
                &key,
                painter.frame,
                self,
                ways.found,
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
            .child(follow(&key, frame.turned(), self.moved.clone(), &sender))
            .child(trackpad(&key, frame, most, self.moved.clone(), &sender));
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
        painter.in_live_view(element.into_any_element(), place)
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
                let zoomed = view::glide_zoom(&key, frame, steps, middle, most);
                tell(&key, moved.as_ref(), zoomed, &sender, window, cx);
                redraw(&key, window, cx);
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

/// What only a trackpad does: two fingers turning the map, and a
/// two-finger double tap zooming it in.
fn trackpad(key: &str, frame: Frame, most: f64, moved: Option<Data>, sender: &Sender) -> Gestures {
    let (turn_key, turn_moved, turn_sender) = (key.to_owned(), moved.clone(), sender.clone());
    let (zoom_key, zoom_sender) = (key.to_owned(), sender.clone());
    gestures()
        .on_rotate(move |event, window, cx| {
            let degrees = f64::from(event.degrees);
            if let Some(turned) = view::turn(&turn_key, frame, degrees, event.phase) {
                tell(
                    &turn_key,
                    turn_moved.as_ref(),
                    turned,
                    &turn_sender,
                    window,
                    cx,
                );
                redraw(&turn_key, window, cx);
            }
        })
        .on_smart_zoom(move |event, window, cx| {
            let zoomed = view::glide_zoom(&zoom_key, frame, 1.0, event.position, most);
            tell(&zoom_key, moved.as_ref(), zoomed, &zoom_sender, window, cx);
            redraw(&zoom_key, window, cx);
        })
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
    let (press_moved, press_sender) = (moved.clone(), sender.clone());
    let (wheel_moved, wheel_sender) = (moved.clone(), sender.clone());
    let (pinch_moved, pinch_sender) = (moved, sender.clone());
    element
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            // The map takes the press: a window dragged by its background
            // does not move.
            cx.stop_propagation();
            if event.click_count == 2 {
                let zoomed = view::glide_zoom(&press_key, frame, 1.0, event.position, most);
                tell(
                    &press_key,
                    press_moved.as_ref(),
                    zoomed,
                    &press_sender,
                    window,
                    cx,
                );
                redraw(&press_key, window, cx);
                return;
            }
            view::press(
                &press_key,
                view::current(&press_key, frame),
                event.position,
                window,
            );
            // The next paint installs the listeners that follow this press.
            redraw(&press_key, window, cx);
        })
        .on_scroll(move |event, window, cx| {
            cx.stop_propagation();
            let steps = scroll_steps(event.delta);
            if steps == 0.0 {
                return;
            }
            let current = view::current(&wheel_key, frame);
            let zoomed = view::zoom(&wheel_key, current, steps, event.position, most);
            tell(
                &wheel_key,
                wheel_moved.as_ref(),
                zoomed,
                &wheel_sender,
                window,
                cx,
            );
            redraw(&wheel_key, window, cx);
        })
        .on_pinch(move |event, window, cx| {
            cx.stop_propagation();
            let delta = f64::from(event.delta);
            let zoomed = view::pinch(&pinch_key, frame, delta, event.phase, event.position, most);
            tell(
                &pinch_key,
                pinch_moved.as_ref(),
                zoomed,
                &pinch_sender,
                window,
                cx,
            );
            redraw(&pinch_key, window, cx);
        })
}

/// The zoom steps a wheel or trackpad scroll of `delta` makes.
fn scroll_steps(delta: ScrollDelta) -> f64 {
    match delta {
        ScrollDelta::Lines(lines) => -f64::from(lines.y) * WHEEL_ZOOM,
        ScrollDelta::Pixels(pixels) => {
            let pixels = -f64::from(pixels.y);
            (1.0 + pixels.abs() * SCROLL_SCALE).log2().copysign(pixels)
        },
    }
}

/// A canvas over the box that keeps its bounds and, while the map is the
/// one pressed, follows the pointer across the whole window.
fn follow(key: &str, turned: bool, moved: Option<Data>, sender: &Sender) -> impl IntoElement {
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
            memory::settle(window, cx);
            memory::evict(window, cx);
            if !turned {
                rotated::release(&key, window, cx);
            }
            if !view::dragging(&key, window) {
                return;
            }
            let moving = key.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || !view::dragging(&moving, window) {
                    return;
                }
                if event.pressed_button.is_none() {
                    view::release(&moving);
                    return;
                }
                if view::drag_to(event.position).is_some() {
                    redraw(&moving, window, cx);
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
                    tell(&released, moved.as_ref(), frame, &sender, window, cx);
                }
            });
        },
    )
    .absolute()
    .size_full()
}

/// Tells the view through `move` where the map now looks and points: at once,
/// or with the latest place once it has been told recently.
fn tell(
    key: &str,
    moved: Option<&Data>,
    frame: Frame,
    sender: &Sender,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(msg) = moved else {
        return;
    };
    match view::tell(key, frame, Instant::now()) {
        view::Tell::Now => send_move(msg, frame, sender, window, cx),
        view::Tell::Held => {},
        view::Tell::After(wait) => {
            let (key, msg, sender) = (key.to_owned(), msg.clone(), sender.clone());
            window
                .spawn(cx, async move |cx| {
                    cx.background_executor().timer(wait).await;
                    let _ = cx.update(|window, cx| {
                        if let Some(frame) = view::late(&key, Instant::now()) {
                            send_move(&msg, frame, &sender, window, cx);
                        }
                    });
                })
                .detach();
        },
    }
}

/// Sends `move` with where the map looks and points.
fn send_move(msg: &Data, frame: Frame, sender: &Sender, window: &mut Window, cx: &mut App) {
    let ([lat, lon], zoom) = frame.told();
    let fields = BTreeMap::from([
        ("lat".to_owned(), Data::Number(lat)),
        ("lon".to_owned(), Data::Number(lon)),
        ("zoom".to_owned(), Data::Number(zoom)),
        (
            "bearing".to_owned(),
            Data::Number((frame.bearing * 100.0).round() / 100.0),
        ),
    ]);
    sender.send(with_fields(msg, fields), window, cx);
}

#[cfg(test)]
mod tests {
    use {
        super::scroll_steps,
        gpui_kit::{ScrollDelta, point, px},
    };

    #[test]
    fn a_trackpad_zooms_by_scale_and_a_wheel_by_notches() {
        let pixels = |y: f32| ScrollDelta::Pixels(point(px(0.), px(y)));
        // A hundred pixels doubles the scale, either way.
        assert!((scroll_steps(pixels(-100.)) - 1.0).abs() < 1e-9);
        assert!((scroll_steps(pixels(100.)) + 1.0).abs() < 1e-9);
        let notch = ScrollDelta::Lines(point(0., -3.));
        assert!((scroll_steps(notch) - 1.0).abs() < 1e-6);
        assert_eq!(scroll_steps(pixels(0.)), 0.0);
    }
}
