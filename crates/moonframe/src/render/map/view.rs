//! Where each `ui.map` looks: what its view gives, or where the user
//! dragged, zoomed and turned it to, kept by window and place across frames.
//! Turned, the box's own axes no longer run along the world's: every step
//! between pixels in the box and places in the world goes through
//! [`Frame::turn`] or [`Frame::unturn`], so pins, routes, drags and zooms
//! all agree with the tiles.

use {
    gpui_kit::*,
    moonframe_core::{MAX_LATITUDE, Map, unworld, world},
    std::{
        cell::RefCell,
        collections::HashMap,
        time::{Duration, Instant},
    },
};

/// Pixels left around the pins and routes a map frames.
const MARGIN: f64 = 48.0;
/// How many screens away a glide still goes. Farther, the map jumps: a
/// slide that long would blur past and fetch tiles nobody sees.
const GLIDE_SCREENS: f64 = 8.0;
/// Degrees two fingers turn before the map follows: a pinch always turns a
/// little, and that must not tip the map.
const TURN_FROM: f64 = 6.0;
/// Degrees from north a turn settles back to north when the fingers lift.
const NORTH_WITHIN: f64 = 4.0;

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
    /// The compass direction up the box, in degrees clockwise from north,
    /// from 0 up to 360.
    pub(super) bearing: f64,
}

impl Frame {
    /// Whether the two frames look the same, to within half a pixel.
    fn same_as(&self, other: &Self) -> bool {
        let span = self.span();
        (self.zoom - other.zoom).abs() < 0.001
            && self.turn_to(other).abs() < 0.01
            && (near(self.center.0, other.center.0) - other.center.0).abs() * span < 0.5
            && (self.center.1 - other.center.1).abs() * span < 0.5
    }

    /// The short way round from this bearing to `to`'s, in degrees.
    fn turn_to(&self, to: &Self) -> f64 {
        (to.bearing - self.bearing + 540.0).rem_euclid(360.0) - 180.0
    }

    /// Whether a glide from here reaches `to`: not too many screens away,
    /// measured at the wider of the two zooms.
    fn glides_to(&self, to: &Self) -> bool {
        let span = 256.0 * self.zoom.min(to.zoom).exp2();
        let across = (near(self.center.0, to.center.0) - to.center.0)
            .abs()
            .max((self.center.1 - to.center.1).abs());
        across * span <= GLIDE_SCREENS * to.size.0.max(to.size.1)
    }

    /// `progress` of the way, 0 to 1, from here to `to`, easing out.
    fn toward(&self, to: &Self, progress: f64) -> Self {
        let eased = 1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3);
        let from = near(self.center.0, to.center.0);
        Self {
            center: (
                from + (to.center.0 - from) * eased,
                self.center.1 + (to.center.1 - self.center.1) * eased,
            ),
            zoom: self.zoom + (to.zoom - self.zoom) * eased,
            size: to.size,
            bearing: (self.bearing + self.turn_to(to) * eased).rem_euclid(360.0),
        }
    }

    /// Whether north is anywhere but straight up.
    pub(super) fn turned(&self) -> bool {
        self.bearing.min(360.0 - self.bearing) > 0.001
    }

    /// `(x, y)` along the world's axes, as seen along the box's.
    fn turn(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let (sin, cos) = self.bearing.to_radians().sin_cos();
        (x * cos + y * sin, y * cos - x * sin)
    }

    /// `(x, y)` along the box's axes, as seen along the world's.
    pub(super) fn unturn(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let (sin, cos) = self.bearing.to_radians().sin_cos();
        (x * cos - y * sin, y * cos + x * sin)
    }

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
    /// `low` and `high` are its corners in the box. Copies lie a world
    /// apart along the box's turned east, so each axis bounds them.
    pub(super) fn copies(
        &self,
        low: (f32, f32),
        high: (f32, f32),
        padding: f32,
    ) -> std::ops::RangeInclusive<i64> {
        let (step_x, step_y) = self.turn((self.span(), 0.0));
        let padding = f64::from(padding);
        let reach = |low: f32, high: f32, size: f64, step: f64| {
            let (low, high) = (f64::from(low), f64::from(high));
            if step.abs() < 1e-9 {
                let inside = high >= -padding && low <= size + padding;
                return if inside {
                    (f64::NEG_INFINITY, f64::INFINITY)
                } else {
                    (1.0, 0.0)
                };
            }
            let (a, b) = ((-padding - high) / step, (size + padding - low) / step);
            (a.min(b), a.max(b))
        };
        let (x_from, x_to) = reach(low.0, high.0, self.size.0, step_x);
        let (y_from, y_to) = reach(low.1, high.1, self.size.1, step_y);
        x_from.max(y_from).ceil() as i64..=x_to.min(y_to).floor() as i64
    }

    /// How far world copy `copy` of something lies from it, in the box.
    pub(super) fn copy_offset(&self, copy: i64) -> (f32, f32) {
        let (x, y) = self.turn((copy as f64 * self.span(), 0.0));
        (x as f32, y as f32)
    }

    fn at_world(&self, x: f64, y: f64) -> (f32, f32) {
        let span = self.span();
        let (left, top) = self.turn(((x - self.center.0) * span, (y - self.center.1) * span));
        (
            (left + self.size.0 / 2.0) as f32,
            (top + self.size.1 / 2.0) as f32,
        )
    }

    /// The point of the world square under `(left, top)` in the box.
    pub(super) fn world_at(&self, left: f64, top: f64) -> (f64, f64) {
        let span = self.span();
        let (x, y) = self.unturn((left - self.size.0 / 2.0, top - self.size.1 / 2.0));
        (self.center.0 + x / span, self.center.1 + y / span)
    }

    /// The least and most of the world square the box shows, x and y.
    pub(super) fn world_box(&self) -> ((f64, f64), (f64, f64)) {
        let (width, height) = self.size;
        let corners = [(0.0, 0.0), (width, 0.0), (0.0, height), (width, height)]
            .map(|(left, top)| self.world_at(left, top));
        corners
            .iter()
            .fold((corners[0], corners[0]), |(low, high), &(x, y)| {
                ((low.0.min(x), low.1.min(y)), (high.0.max(x), high.1.max(y)))
            })
    }

    /// `[lat, lon]` at the middle.
    pub(super) fn middle(&self) -> [f64; 2] {
        unworld(self.center.0.rem_euclid(1.0), self.center.1)
    }

    /// Moved by `(dx, dy)` pixels of the pointer: the map follows it.
    fn dragged(self, dx: f64, dy: f64) -> Self {
        let span = self.span();
        let (dx, dy) = self.unturn((dx, dy));
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
    /// What the last frame showed.
    drawn: Option<Frame>,
    /// Where a glide under way left from, and when.
    glide: Option<(Frame, Instant)>,
    /// The box as last laid out, in window pixels.
    bounds: Bounds<Pixels>,
    /// The window's scale, for `@2x` tiles.
    scale: f32,
    /// Where the map points, kept apart from `moved`: a view that follows
    /// `move` with `center` must not undo a turn it did not echo.
    bearing: f64,
    /// The `bearing` the view last gave.
    given_bearing: Option<f32>,
    /// Two fingers turning it, if they are.
    turn: Option<Turn>,
}

/// One two-finger turn, from the first touch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Turn {
    /// How far the fingers have gone, while the map has not followed.
    waited: f64,
    /// The map follows: past [`TURN_FROM`].
    following: bool,
}

impl Turn {
    /// The fingers turned `degrees` more: what the map turns by.
    fn by(&mut self, degrees: f64) -> f64 {
        if self.following {
            return degrees;
        }
        self.waited += degrees;
        self.following = self.waited.abs() >= TURN_FROM;
        0.0
    }
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
pub(super) fn frame(
    map: &Map,
    key: &str,
    fallback: (f64, f64),
    now: Instant,
    glide: Duration,
) -> (Frame, f32) {
    let given = Given::of(map);
    KEPT.with_borrow_mut(|kept| {
        let entry = kept.entry(key.to_owned()).or_insert_with(|| Kept {
            given: given.clone(),
            moved: None,
            drawn: None,
            glide: None,
            bounds: Bounds::default(),
            scale: 2.0,
            bearing: compass(map.bearing),
            given_bearing: map.bearing,
            turn: None,
        });
        // The view moved it: that wins over where the user left it.
        let mut sent = false;
        if entry.given != given {
            sent = entry.given.center != given.center || entry.given.zoom != given.zoom;
            entry.given = given;
            entry.moved = None;
        }
        // While the fingers turn it they lead, and the view's echo of their
        // last move trails behind: it would pull the map back.
        if entry.given_bearing != map.bearing {
            entry.given_bearing = map.bearing;
            if map.bearing.is_some() && !entry.turn.is_some_and(|turn| turn.following) {
                entry.bearing = compass(map.bearing);
                sent = true;
            }
        }
        let bearing = entry.bearing;
        let laid = entry.bounds.size;
        let size = if laid.width > px(0.) && laid.height > px(0.) {
            (f64::from(laid.width), f64::from(laid.height))
        } else {
            fallback
        };
        let frame = match entry.moved {
            Some(moved) => Frame {
                size,
                bearing,
                ..moved
            },
            None => {
                let (framed, fit) = map.framing(size.0, size.1, MARGIN, bearing);
                let [lat, lon] = map.center.unwrap_or(framed);
                let zoom = map.zoom.map_or(fit, f64::from).clamp(0.0, most_zoom(map));
                Frame {
                    center: world(lat, lon),
                    zoom,
                    size,
                    bearing,
                }
            },
        };
        if sent
            && map.glide
            && !glide.is_zero()
            && let Some(from) = entry.drawn
        {
            entry.glide = (!from.same_as(&frame) && from.glides_to(&frame)).then_some((from, now));
        }
        let shown = match entry.glide {
            Some((from, start)) => {
                let progress = now.saturating_duration_since(start).as_secs_f64()
                    / glide.as_secs_f64().max(f64::EPSILON);
                if progress >= 1.0 {
                    entry.glide = None;
                    frame
                } else {
                    from.toward(&frame, progress)
                }
            },
            None => frame,
        };
        entry.drawn = Some(shown);
        (shown, entry.scale)
    })
}

/// Whether the map `key` is on its way somewhere, and wants another frame.
pub(super) fn gliding(key: &str) -> bool {
    KEPT.with_borrow(|kept| kept.get(key).is_some_and(|kept| kept.glide.is_some()))
}

/// `bearing` as a frame holds it, from 0 up to 360.
fn compass(bearing: Option<f32>) -> f64 {
    f64::from(bearing.unwrap_or(0.0)).rem_euclid(360.0)
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

/// Two fingers turned `degrees` clockwise over the map `key`, in `phase`;
/// the frame to tell the view about, if the map moved. The map turns with
/// them, about its middle, once they mean it; lifted close to north, it
/// settles there.
pub(super) fn turn(key: &str, drawn: Frame, degrees: f64, phase: TouchPhase) -> Option<Frame> {
    let frame = current(key, drawn);
    let (bearing, moved) = KEPT.with_borrow_mut(|kept| {
        let entry = kept.get_mut(key)?;
        let mut turn = match phase {
            TouchPhase::Started => Turn::default(),
            _ => entry.turn.unwrap_or_default(),
        };
        // Turning the map clockwise brings what lay left of north up.
        let bearing = (entry.bearing - turn.by(degrees)).rem_euclid(360.0);
        let ended = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);
        let bearing = if ended && turn.following {
            settled(bearing)
        } else {
            bearing
        };
        let moved = bearing != entry.bearing || (ended && turn.following);
        entry.bearing = bearing;
        entry.turn = (!ended).then_some(turn);
        Some((bearing, moved))
    })?;
    if !moved {
        return None;
    }
    // Held where it is: a map framing its pins would refit as it turns.
    let turned = Frame { bearing, ..frame };
    keep(key, turned);
    Some(turned)
}

/// A turn's last `bearing`, close to north becoming north.
fn settled(bearing: f64) -> f64 {
    if bearing.min(360.0 - bearing) < NORTH_WITHIN {
        0.0
    } else {
        bearing
    }
}

/// Where the user left the map `key`, if they moved it.
pub(super) fn moved(key: &str) -> Option<Frame> {
    KEPT.with_borrow(|kept| {
        let kept = kept.get(key)?;
        kept.moved.map(|moved| Frame {
            bearing: kept.bearing,
            ..moved
        })
    })
}

/// Where the map `key` looks now: where the user left it, else `drawn`,
/// as the last frame drew it, turned as it now is. Events between two
/// frames see each other.
pub(super) fn current(key: &str, drawn: Frame) -> Frame {
    moved(key).unwrap_or_else(|| Frame {
        bearing: KEPT.with_borrow(|kept| kept.get(key).map_or(drawn.bearing, |kept| kept.bearing)),
        ..drawn
    })
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
            // The user took over.
            entry.glide = None;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{Frame, Turn, settled, world};

    #[test]
    fn zooming_keeps_the_place_under_the_pointer() {
        let frame = Frame {
            center: world(37.79, -122.40),
            zoom: 14.0,
            size: (800.0, 600.0),
            bearing: 0.0,
        };
        let under = frame.world_at(100.0, 50.0);
        let zoomed = frame.zoomed(1.5, 100.0, 50.0, 20.0);
        let after = zoomed.world_at(100.0, 50.0);
        assert!((under.0 - after.0).abs() < 1e-12 && (under.1 - after.1).abs() < 1e-12);
        assert_eq!(zoomed.zoom, 15.5);
    }

    #[test]
    fn a_glide_eases_to_its_end_and_refuses_long_trips() {
        let from = Frame {
            center: world(37.79, -122.40),
            zoom: 14.0,
            size: (800.0, 600.0),
            bearing: 0.0,
        };
        let to = Frame {
            center: world(37.80, -122.41),
            zoom: 15.0,
            ..from
        };
        assert_eq!(from.toward(&to, 0.0), from);
        assert!(from.toward(&to, 1.0).same_as(&to));
        let half = from.toward(&to, 0.5);
        assert!(half.zoom > 14.5 && half.zoom < 15.0);
        assert!(from.glides_to(&to));
        let far = Frame {
            center: world(40.71, -74.0),
            ..to
        };
        assert!(!from.glides_to(&far));
        assert!(from.same_as(&from.toward(&from, 0.5)));
    }

    #[test]
    fn a_glide_turns_the_short_way_round_past_north() {
        let from = Frame {
            center: world(37.79, -122.40),
            zoom: 14.0,
            size: (800.0, 600.0),
            bearing: 350.0,
        };
        let to = Frame {
            bearing: 10.0,
            ..from
        };
        assert!(!from.same_as(&to));
        assert!(from.toward(&to, 1.0).same_as(&to));
        let half = from.toward(&to, 0.2).bearing;
        assert!(!(10.0..350.0).contains(&half), "{half}");
    }

    #[test]
    fn a_glide_takes_the_short_way_over_the_dateline() {
        let from = Frame {
            center: world(0.0, 179.99),
            zoom: 10.0,
            size: (800.0, 600.0),
            bearing: 0.0,
        };
        let to = Frame {
            center: world(0.0, -179.99),
            ..from
        };
        assert!(from.glides_to(&to));
        let half = from.toward(&to, 0.2);
        assert!((half.center.0 - to.center.0).abs() < 1e-3);
    }

    #[test]
    fn a_place_lands_where_the_frame_says() {
        let frame = Frame {
            center: world(37.79, -122.40),
            zoom: 14.0,
            size: (800.0, 600.0),
            bearing: 0.0,
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
            bearing: 0.0,
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
            bearing: 0.0,
        };
        let route = frame.route(&[[0.0, 179.0], [0.0, -179.0]]);
        assert!(route[0].0 > 500.0 && route[1].0 > 512.0);
        assert!(route[1].0 - route[0].0 < 3.0);
        let copies: Vec<_> = frame.copies(route[0], route[1], 4.0).collect();
        assert_eq!(copies, [-1, 0]);
        // At zoom zero, all three visible copies of a pin are drawn.
        let wide = Frame {
            zoom: 0.0,
            size: (800.0, 400.0),
            ..frame
        };
        let (x, _) = wide.at(0.0, 0.0);
        let at = wide.at(0.0, 0.0);
        assert_eq!(at.0, x);
        assert_eq!(wide.copies(at, at, 14.0).collect::<Vec<_>>(), [-1, 0, 1]);
    }

    fn turned(bearing: f64) -> Frame {
        Frame {
            center: world(37.79, -122.40),
            zoom: 14.0,
            size: (800.0, 600.0),
            bearing,
        }
    }

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    #[test]
    fn east_up_puts_north_on_the_left() {
        let frame = turned(90.0);
        let (left, top) = frame.at(37.80, -122.40);
        assert!(left < 300.0 && (top - 300.0).abs() < 0.01, "{left}, {top}");
        let (left, top) = frame.at(37.79, -122.39);
        assert!((left - 400.0).abs() < 0.01 && top < 200.0, "{left}, {top}");
    }

    #[test]
    fn a_place_and_its_pixel_agree_at_any_bearing() {
        for bearing in [0.0, 17.0, 90.0, 133.5, 270.0, 359.0] {
            let frame = turned(bearing);
            let (left, top) = frame.at(37.80, -122.41);
            let (x, y) = frame.world_at(f64::from(left), f64::from(top));
            let (want_x, want_y) = world(37.80, -122.41);
            assert!((x - want_x).abs() < 1e-7 && (y - want_y).abs() < 1e-7);
        }
    }

    #[test]
    fn a_drag_keeps_the_place_under_the_pointer_when_turned() {
        for bearing in [0.0, 30.0, 200.0] {
            let frame = turned(bearing);
            let under = frame.world_at(100.0, 50.0);
            let dragged = frame.dragged(35.0, -20.0);
            let after = dragged.world_at(135.0, 30.0);
            assert!((under.0 - after.0).abs() < 1e-9 && (under.1 - after.1).abs() < 1e-9);
        }
    }

    #[test]
    fn zooming_keeps_the_place_under_the_pointer_when_turned() {
        let frame = turned(75.0);
        let under = frame.world_at(100.0, 50.0);
        let after = frame.zoomed(1.5, 100.0, 50.0, 20.0).world_at(100.0, 50.0);
        assert!((under.0 - after.0).abs() < 1e-12 && (under.1 - after.1).abs() < 1e-12);
    }

    #[test]
    fn the_world_box_covers_every_corner_of_a_turned_box() {
        let frame = turned(45.0);
        let (low, high) = frame.world_box();
        for corner in [(0.0, 0.0), (800.0, 0.0), (0.0, 600.0), (800.0, 600.0)] {
            let (x, y) = frame.world_at(corner.0, corner.1);
            assert!(low.0 <= x && x <= high.0 && low.1 <= y && y <= high.1);
        }
        let north = turned(0.0).world_box();
        assert!(high.0 - low.0 > north.1.0 - north.0.0);
    }

    #[test]
    fn world_copies_of_a_place_lie_along_the_turned_east() {
        // A quarter turn: copies repeat up and down the box, not across it.
        let frame = Frame {
            center: world(0.0, 0.0),
            zoom: 0.0,
            size: (300.0, 800.0),
            bearing: 90.0,
        };
        let at = frame.at(0.0, 0.0);
        assert_eq!(frame.copies(at, at, 14.0).collect::<Vec<_>>(), [-1, 0, 1]);
        let (x, y) = frame.copy_offset(1);
        assert!(close((f64::from(x), f64::from(y)), (0.0, -frame.span())));
        // Off the box's side, no copy along its length brings it in.
        let aside = (at.0 + 1000.0, at.1);
        assert_eq!(frame.copies(aside, aside, 14.0).count(), 0);
    }

    #[test]
    fn a_pinch_s_small_turn_does_not_tip_the_map() {
        let mut turn = Turn::default();
        assert_eq!(turn.by(3.0), 0.0);
        assert_eq!(turn.by(-2.0), 0.0);
        assert!(!turn.following);
        assert_eq!(turn.by(5.5), 0.0);
        assert!(turn.following);
        assert_eq!(turn.by(1.5), 1.5);
    }

    #[test]
    fn a_turn_left_close_to_north_settles_there() {
        assert_eq!(settled(3.0), 0.0);
        assert_eq!(settled(358.0), 0.0);
        assert_eq!(settled(10.0), 10.0);
        assert_eq!(settled(350.0), 350.0);
    }
}
