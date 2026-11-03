//! Where each `ui.map` looks: what its view gives, or where the user
//! dragged and zoomed it to, kept by window and place across frames.

use {
    gpui_kit::*,
    moonframe_core::{MAX_LATITUDE, Map, unworld, world},
    std::{cell::RefCell, collections::HashMap},
};

/// Pixels left around the pins and routes a map frames.
const MARGIN: f64 = 48.0;

/// What the view gave, which, once it changes, wins over where the user
/// left the map.
#[derive(Clone, Debug, PartialEq)]
struct Given {
    center: Option<[f64; 2]>,
    zoom: Option<f32>,
    /// The pins and routes, which a map without `center` frames.
    places: Vec<[f64; 2]>,
}

impl Given {
    fn of(map: &Map) -> Self {
        Self {
            center: map.center,
            zoom: map.zoom,
            places: map
                .pins
                .iter()
                .map(|pin| [pin.lat, pin.lon])
                .chain(
                    map.routes
                        .iter()
                        .flat_map(|route| route.places().iter().copied()),
                )
                .collect(),
        }
    }
}

/// Where one map looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Frame {
    /// The middle of the box on the Web Mercator square, 0 to 1.
    pub(super) center: (f64, f64),
    pub(super) zoom: f64,
    /// The box, in pixels.
    pub(super) size: (f64, f64),
}

impl Frame {
    /// The world's width in pixels at this zoom.
    pub(super) fn span(&self) -> f64 {
        256.0 * self.zoom.exp2()
    }

    /// Where `lat`, `lon` lands in the box, in pixels from its corner.
    pub(super) fn at(&self, lat: f64, lon: f64) -> (f32, f32) {
        let (x, y) = world(lat, lon);
        self.at_world(near(x, self.center.0), y)
    }

    /// Unwrap a route one segment at a time. Wrapping every corner around
    /// the center independently would draw a line across the whole map
    /// when a segment crosses the opposite meridian.
    pub(super) fn route(&self, places: &[[f64; 2]]) -> Vec<(f32, f32)> {
        let mut previous = self.center.0;
        places
            .iter()
            .map(|&[lat, lon]| {
                let (x, y) = world(lat, lon);
                let x = near(x, previous);
                previous = x;
                self.at_world(x, y)
            })
            .collect()
    }

    /// World copies of a pin or unwrapped route that intersect the box,
    /// including its marker/stroke padding. Low zooms can show several.
    pub(super) fn copies(
        &self,
        left: f32,
        right: f32,
        padding: f32,
    ) -> std::ops::RangeInclusive<i64> {
        let span = self.span();
        let first = ((-f64::from(padding) - f64::from(right)) / span).ceil() as i64;
        let last = ((self.size.0 + f64::from(padding) - f64::from(left)) / span).floor() as i64;
        first..=last
    }

    fn at_world(&self, x: f64, y: f64) -> (f32, f32) {
        let span = self.span();
        let left = (x - self.center.0) * span + self.size.0 / 2.0;
        let top = (y - self.center.1) * span + self.size.1 / 2.0;
        (left as f32, top as f32)
    }

    /// The point of the world square under `(left, top)` in the box.
    fn world_at(&self, left: f64, top: f64) -> (f64, f64) {
        let span = self.span();
        (
            self.center.0 + (left - self.size.0 / 2.0) / span,
            self.center.1 + (top - self.size.1 / 2.0) / span,
        )
    }

    /// `[lat, lon]` at the middle.
    pub(super) fn middle(&self) -> [f64; 2] {
        unworld(self.center.0.rem_euclid(1.0), self.center.1)
    }

    /// Moved by `(dx, dy)` pixels of the pointer: the map follows it.
    fn dragged(self, dx: f64, dy: f64) -> Self {
        let span = self.span();
        let (_, top) = world(MAX_LATITUDE, 0.0);
        let center = (
            self.center.0 - dx / span,
            (self.center.1 - dy / span).clamp(top, 1.0 - top),
        );
        Self { center, ..self }
    }

    /// Zoomed by `steps` (one doubles the scale), keeping the place under
    /// `(left, top)` where it is.
    fn zoomed(self, steps: f64, left: f64, top: f64, most: f64) -> Self {
        let zoom = (self.zoom + steps).clamp(0.0, most);
        let before = self.world_at(left, top);
        let mut after = Self { zoom, ..self };
        let moved = after.world_at(left, top);
        after.center = (
            after.center.0 + before.0 - moved.0,
            after.center.1 + before.1 - moved.1,
        );
        after
    }
}

/// The copy of a world x-coordinate closest to `reference`, including
/// when the user has panned several worlds away from the original one.
fn near(x: f64, reference: f64) -> f64 {
    x + (reference - x).round()
}

/// One map's state.
struct Kept {
    given: Given,
    /// Where the user left it, once they move it.
    moved: Option<Frame>,
    /// The box as last laid out, in window pixels.
    bounds: Bounds<Pixels>,
    /// The window's scale, for `@2x` tiles.
    scale: f32,
}

thread_local! {
    static KEPT: RefCell<HashMap<String, Kept>> = RefCell::new(HashMap::new());
}

/// The deepest zoom `map` shows: its provider's, a step beyond (tiles
/// enlarged), at most the deepest any map takes.
pub(super) fn most_zoom(map: &Map) -> f64 {
    (f64::from(map.provider.source().max_zoom) + 1.0).min(f64::from(moonframe_core::MAX_MAP_ZOOM))
}

/// Where the map kept as `key` looks this frame, in a box of `fallback`
/// pixels until it has been laid out.
pub(super) fn frame(map: &Map, key: &str, fallback: (f64, f64)) -> (Frame, f32) {
    let given = Given::of(map);
    KEPT.with_borrow_mut(|kept| {
        let entry = kept.entry(key.to_owned()).or_insert_with(|| Kept {
            given: given.clone(),
            moved: None,
            bounds: Bounds::default(),
            scale: 2.0,
        });
        // The view moved it: that wins over where the user left it.
        if entry.given != given {
            entry.given = given;
            entry.moved = None;
        }
        let laid = entry.bounds.size;
        let size = if laid.width > px(0.) && laid.height > px(0.) {
            (f64::from(laid.width), f64::from(laid.height))
        } else {
            fallback
        };
        let frame = match entry.moved {
            Some(moved) => Frame { size, ..moved },
            None => {
                let (framed, fit) = map.framing(size.0, size.1, MARGIN);
                let [lat, lon] = map.center.unwrap_or(framed);
                let zoom = map.zoom.map_or(fit, f64::from).clamp(0.0, most_zoom(map));
                Frame {
                    center: world(lat, lon),
                    zoom,
                    size,
                }
            },
        };
        (frame, entry.scale)
    })
}

/// Keeps the box `key` was laid out in; whether it changed size.
pub(super) fn laid(key: &str, bounds: Bounds<Pixels>, scale: f32) -> bool {
    KEPT.with_borrow_mut(|kept| {
        let Some(entry) = kept.get_mut(key) else {
            return false;
        };
        let resized = entry.bounds.size != bounds.size || entry.scale != scale;
        entry.bounds = bounds;
        entry.scale = scale;
        resized
    })
}

/// The press being followed: one at a time, as there is one pointer.
struct Drag {
    key: String,
    window: WindowId,
    start: Point<Pixels>,
    from: Frame,
}

thread_local! {
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
}

/// A press on the map `key`, showing `frame`, at `position`.
pub(super) fn press(key: &str, frame: Frame, position: Point<Pixels>, window: &Window) {
    DRAG.set(Some(Drag {
        key: key.to_owned(),
        window: window.window_handle().window_id(),
        start: position,
        from: frame,
    }));
}

/// Whether the map `key` holds the press in `window`.
pub(super) fn dragging(key: &str, window: &Window) -> bool {
    let id = window.window_handle().window_id();
    DRAG.with_borrow(|drag| {
        drag.as_ref()
            .is_some_and(|drag| drag.window == id && drag.key == key)
    })
}

/// Follows the pointer to `position`; the frame it moved to.
pub(super) fn drag_to(position: Point<Pixels>) -> Option<Frame> {
    let (key, frame) = DRAG.with_borrow(|drag| {
        let drag = drag.as_ref()?;
        let dx = f64::from(position.x - drag.start.x);
        let dy = f64::from(position.y - drag.start.y);
        Some((drag.key.clone(), drag.from.dragged(dx, dy)))
    })?;
    keep(&key, frame);
    Some(frame)
}

/// Ends the press on `key`; whether it moved the map.
pub(super) fn release(key: &str) -> bool {
    DRAG.with_borrow_mut(|slot| {
        if slot.as_ref().is_some_and(|drag| drag.key == key) {
            let drag = slot.take();
            return drag.is_some_and(|drag| {
                KEPT.with_borrow(|kept| {
                    kept.get(key)
                        .and_then(|kept| kept.moved)
                        .is_some_and(|moved| moved.center != drag.from.center)
                })
            });
        }
        false
    })
}

/// Zooms the map `key` by `steps` around `position` in the window.
pub(super) fn zoom(
    key: &str,
    frame: Frame,
    steps: f64,
    position: Point<Pixels>,
    most: f64,
) -> Frame {
    let origin = KEPT.with_borrow(|kept| {
        kept.get(key)
            .map_or_else(Point::default, |kept| kept.bounds.origin)
    });
    let left = f64::from(position.x - origin.x);
    let top = f64::from(position.y - origin.y);
    let zoomed = frame.zoomed(steps, left, top, most);
    keep(key, zoomed);
    zoomed
}

/// Where the user left the map `key`, if they moved it.
pub(super) fn moved(key: &str) -> Option<Frame> {
    KEPT.with_borrow(|kept| kept.get(key).and_then(|kept| kept.moved))
}

/// Where the map `key` looks now: where the user left it, else `drawn`,
/// as the last frame drew it. Events between two frames see each other.
pub(super) fn current(key: &str, drawn: Frame) -> Frame {
    moved(key).unwrap_or(drawn)
}

/// The corner of the map `key`'s box in the window, as last laid out.
pub(super) fn origin(key: &str) -> Point<Pixels> {
    KEPT.with_borrow(|kept| {
        kept.get(key)
            .map_or_else(Point::default, |kept| kept.bounds.origin)
    })
}

/// The middle of the map `key`'s box, in the window.
pub(super) fn middle(key: &str) -> Point<Pixels> {
    KEPT.with_borrow(|kept| {
        kept.get(key)
            .map_or_else(Point::default, |kept| kept.bounds.center())
    })
}

fn keep(key: &str, frame: Frame) {
    KEPT.with_borrow_mut(|kept| {
        if let Some(entry) = kept.get_mut(key) {
            entry.moved = Some(frame);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{Frame, world};

    #[test]
    fn zooming_keeps_the_place_under_the_pointer() {
        let frame = Frame {
            center: world(37.79, -122.40),
            zoom: 14.0,
            size: (800.0, 600.0),
        };
        let under = frame.world_at(100.0, 50.0);
        let zoomed = frame.zoomed(1.5, 100.0, 50.0, 20.0);
        let after = zoomed.world_at(100.0, 50.0);
        assert!((under.0 - after.0).abs() < 1e-12 && (under.1 - after.1).abs() < 1e-12);
        assert_eq!(zoomed.zoom, 15.5);
    }

    #[test]
    fn a_place_lands_where_the_frame_says() {
        let frame = Frame {
            center: world(37.79, -122.40),
            zoom: 14.0,
            size: (800.0, 600.0),
        };
        let (left, top) = frame.at(37.79, -122.40);
        assert!((left - 400.0).abs() < 1e-3 && (top - 300.0).abs() < 1e-3);
        let [lat, lon] = frame.middle();
        assert!((lat - 37.79).abs() < 1e-9 && (lon + 122.40).abs() < 1e-9);
    }

    #[test]
    fn overlays_follow_tiles_across_the_dateline_and_repeated_worlds() {
        let frame = Frame {
            center: world(0.0, 179.9),
            zoom: 10.0,
            size: (800.0, 600.0),
        };
        let (x, y) = frame.at(0.0, -179.9);
        let expected = 400.0 + 0.2 / 360.0 * frame.span();
        assert!((f64::from(x) - expected).abs() < 0.001);
        assert_eq!(y, 300.0);
        for worlds in [-3.0, 2.0] {
            let panned = Frame {
                center: (frame.center.0 + worlds, frame.center.1),
                ..frame
            };
            assert_eq!(panned.at(0.0, -179.9), (x, y));
        }
        let route = frame.route(&[[0.0, 179.9], [0.0, -179.9]]);
        assert_eq!(route[0], (400.0, 300.0));
        assert_eq!(route[1], (x, y));
    }

    #[test]
    fn a_dateline_route_draws_at_both_edges_without_crossing_greenwich() {
        let frame = Frame {
            center: world(0.0, 0.0),
            zoom: 1.0,
            size: (512.0, 400.0),
        };
        let route = frame.route(&[[0.0, 179.0], [0.0, -179.0]]);
        assert!(route[0].0 > 500.0 && route[1].0 > 512.0);
        assert!(route[1].0 - route[0].0 < 3.0);
        let copies: Vec<_> = frame.copies(route[0].0, route[1].0, 4.0).collect();
        assert_eq!(copies, [-1, 0]);
        // At zoom zero, all three visible copies of a pin are drawn.
        let wide = Frame {
            zoom: 0.0,
            size: (800.0, 400.0),
            ..frame
        };
        let (x, _) = wide.at(0.0, 0.0);
        assert_eq!(wide.copies(x, x, 14.0).collect::<Vec<_>>(), [-1, 0, 1]);
    }
}
