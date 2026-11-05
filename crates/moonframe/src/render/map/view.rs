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

/// How often a moving map tells its view where it is. Telling runs the view
/// and reads its whole tree, which took as long as drawing the frame when
/// every event did it; nothing the map draws waits for it.
const TELL_EVERY: Duration = Duration::from_millis(100);
/// How long after the last step a zoom still counts as going on.
const STILL_ZOOMING: Duration = Duration::from_millis(300);
/// Zoom steps looked ahead at a time for where the next tile level starts.
const AHEAD_STEP: f64 = 0.1;
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
/// A pinch that last moved longer ago than this has stopped: letting go of
/// it does not coast. Apple Maps' figure, as the next three are.
const STILL_PINCHING: Duration = Duration::from_millis(150);
/// A change of scale smaller than this is the fingers resting.
const PINCH_MOVES: f64 = 0.01;
/// Zoom steps a pinch coasts on for each unit of its last rate of change:
/// Apple Maps adds a quarter of that rate per frame, fading over 30 frames.
const COAST_STEPS: f64 = 7.2;
/// A coast shorter than this, in zoom steps, is not worth a glide.
const SMALLEST_COAST: f64 = 0.01;

/// How a map gets somewhere on its own: Apple Maps' zoom takes a quarter of a
/// second, easing both ways, and a pinch let go coasts for half of one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Pace {
    /// To another place, however far the view sent it.
    Trip,
    /// Closer or further, staying over the same place.
    Zoom,
    /// On from a pinch that let go still moving.
    Coast,
}

impl Pace {
    fn length(self) -> Duration {
        match self {
            Self::Trip => Duration::from_millis(600),
            Self::Zoom => Duration::from_millis(250),
            Self::Coast => Duration::from_millis(500),
        }
    }

    /// How far along a glide is, 0 to 1, `progress` of the way through it.
    fn eased(self, progress: f64) -> f64 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Trip => 1.0 - (1.0 - progress).powi(3),
            Self::Zoom => progress * progress * (3.0 - 2.0 * progress),
            // Its speed falls as `1 - progress²`, from the fingers' own.
            Self::Coast => (3.0 * progress - progress.powi(3)) / 2.0,
        }
    }
}

/// A glide under way.
#[derive(Clone, Copy, Debug)]
struct Glide {
    from: Frame,
    start: Instant,
    pace: Pace,
}

/// A pinch, as it goes, for how fast it is moving when it ends.
#[derive(Clone, Copy, Debug)]
struct Pinch {
    /// The scale of the latest change, smoothed over the last few.
    rate: f64,
    /// When the fingers last really moved.
    moved: Instant,
}

/// The way a scroll or a pinch is zooming.
#[derive(Clone, Copy, Debug)]
struct Trend {
    /// In, or out.
    inward: bool,
    /// What stays put, from the box's corner.
    around: (f64, f64),
    most: f64,
    at: Instant,
}

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

    /// Whether `to` looks at the same place, to within a pixel.
    fn stays_over(&self, to: &Self) -> bool {
        let span = 256.0 * self.zoom.max(to.zoom).exp2();
        let across = (near(self.center.0, to.center.0) - to.center.0)
            .abs()
            .max((self.center.1 - to.center.1).abs());
        across * span < 1.0
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

    /// `[lat, lon]` and zoom as `move` tells them, rounded.
    pub(super) fn told(&self) -> ([f64; 2], f64) {
        let round = |value: f64, places: i32| {
            let scale = 10f64.powi(places);
            (value * scale).round() / scale
        };
        let [lat, lon] = self.middle();
        ([round(lat, 6), round(lon, 6)], round(self.zoom, 2))
    }

    /// Whether `given` is only the view handing back what `move` told it.
    fn echoed_by(&self, given: &Given) -> bool {
        let (center, zoom) = self.told();
        given.center == Some(center) && given.zoom == Some(zoom as f32)
    }

    /// `eased` of the way, 0 to 1, from here to `to`. What stays put on the
    /// screen stays put all the way: the center moves with the scale, not
    /// with the zoom, which is its logarithm.
    fn toward(&self, to: &Self, eased: f64) -> Self {
        let from = near(self.center.0, to.center.0);
        let zoom = self.zoom + (to.zoom - self.zoom) * eased;
        let scaled = 1.0 - (self.zoom - to.zoom).exp2();
        let along = if scaled.abs() < 1e-9 {
            eased
        } else {
            (1.0 - (self.zoom - zoom).exp2()) / scaled
        };
        Self {
            center: (
                from + (to.center.0 - from) * along,
                self.center.1 + (to.center.1 - self.center.1) * along,
            ),
            zoom,
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
    /// The glide under way.
    glide: Option<Glide>,
    /// Where that glide ends.
    heading: Option<Frame>,
    /// The pinch under way.
    pinch: Option<Pinch>,
    /// The way the map last zoomed by scroll or pinch.
    trend: Option<Trend>,
    /// The last place `move` told the view, and when.
    told: Option<(Frame, Instant)>,
    /// The latest place held back, to tell once [`TELL_EVERY`] has passed.
    untold: Option<Frame>,
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
    scaled: impl Fn(Duration) -> Duration,
) -> (Frame, f32) {
    let given = Given::of(map);
    KEPT.with_borrow_mut(|kept| {
        let entry = kept.entry(key.to_owned()).or_insert_with(|| Kept {
            given: given.clone(),
            moved: None,
            drawn: None,
            glide: None,
            heading: None,
            pinch: None,
            trend: None,
            told: None,
            untold: None,
            bounds: Bounds::default(),
            scale: 2.0,
            bearing: compass(map.bearing),
            given_bearing: map.bearing,
            turn: None,
        });
        // The view moved it: that wins over where the user left it.
        let mut sent = false;
        // Handing back what `move` told it is no order: it would pull a
        // gesture back to where it was when it was told.
        if entry.given != given {
            if !entry.told.is_some_and(|(told, _)| told.echoed_by(&given)) {
                sent = entry.given.center != given.center || entry.given.zoom != given.zoom;
                entry.moved = None;
                entry.told = None;
                entry.untold = None;
            }
            entry.given = given;
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
            && let Some(from) = entry.drawn
        {
            let pace = if from.stays_over(&frame) {
                Pace::Zoom
            } else {
                Pace::Trip
            };
            entry.glide = (!from.same_as(&frame) && from.glides_to(&frame)).then_some(Glide {
                from,
                start: now,
                pace,
            });
        }
        let shown = match entry.glide {
            Some(glide) => {
                let progress = now.saturating_duration_since(glide.start).as_secs_f64()
                    / scaled(glide.pace.length()).as_secs_f64().max(f64::EPSILON);
                if progress >= 1.0 {
                    entry.glide = None;
                    frame
                } else {
                    glide.from.toward(&frame, glide.pace.eased(progress))
                }
            },
            None => frame,
        };
        entry.heading = entry.glide.is_some().then_some(frame);
        entry.drawn = Some(shown);
        (shown, entry.scale)
    })
}

/// Whether the map `key` is on its way somewhere, and wants another frame.
pub(super) fn gliding(key: &str) -> bool {
    KEPT.with_borrow(|kept| kept.get(key).is_some_and(|kept| kept.glide.is_some()))
}

/// Where the map `key` is gliding to, if it is: its tiles can be on their way
/// before it arrives.
pub(super) fn heading(key: &str) -> Option<Frame> {
    KEPT.with_borrow(|kept| kept.get(key).and_then(|kept| kept.heading))
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
    let origin = KEPT.with_borrow_mut(|kept| {
        let entry = kept.get_mut(key)?;
        let origin = entry.bounds.origin;
        let around = (
            f64::from(position.x - origin.x),
            f64::from(position.y - origin.y),
        );
        entry.trend = Some(Trend {
            inward: steps > 0.0,
            around,
            most,
            at: Instant::now(),
        });
        Some(origin)
    });
    let origin = origin.unwrap_or_default();
    let left = f64::from(position.x - origin.x);
    let top = f64::from(position.y - origin.y);
    let zoomed = frame.zoomed(steps, left, top, most);
    keep(key, zoomed);
    zoomed
}

/// Where the map `key`, still zooming at `now`, reaches the next tile level,
/// as `level` tells them apart: its tiles can set off before it gets there.
pub(super) fn ahead(key: &str, now: Instant, level: impl Fn(f64) -> u8) -> Option<Frame> {
    let (frame, trend) = KEPT.with_borrow(|kept| {
        let entry = kept.get(key)?;
        Some((entry.moved?, entry.trend?))
    })?;
    if now.saturating_duration_since(trend.at) > STILL_ZOOMING {
        return None;
    }
    let (left, top) = trend.around;
    let step = if trend.inward {
        AHEAD_STEP
    } else {
        -AHEAD_STEP
    };
    let here = level(frame.zoom);
    (1..=(1.0 / AHEAD_STEP) as u32 + 1)
        .map(|steps| frame.zoomed(step * f64::from(steps), left, top, trend.most))
        .find(|there| level(there.zoom) != here)
}

/// Zooms the map `key` by `steps` around `position` in the window, gliding
/// there. A second click on the way adds to where the first was going.
pub(super) fn glide_zoom(
    key: &str,
    drawn: Frame,
    steps: f64,
    position: Point<Pixels>,
    most: f64,
) -> Frame {
    let target = moved(key).unwrap_or(drawn);
    let from = shown(key, drawn);
    let zoomed = zoom(key, target, steps, position, most);
    glide(key, from, Pace::Zoom);
    zoomed
}

/// Sets the map `key` off from `from` to where it was just kept.
fn glide(key: &str, from: Frame, pace: Pace) {
    KEPT.with_borrow_mut(|kept| {
        if let Some(entry) = kept.get_mut(key) {
            entry.glide = Some(Glide {
                from,
                start: Instant::now(),
                pace,
            });
        }
    });
}

/// A pinch's `delta`, in `phase`, around `position` over the map `key`: the
/// frame to tell the view about. Let go while still moving it coasts on, as
/// Apple Maps' does.
pub(super) fn pinch(
    key: &str,
    drawn: Frame,
    delta: f64,
    phase: TouchPhase,
    position: Point<Pixels>,
    most: f64,
) -> Frame {
    let scale = (1.0 + delta).max(0.01);
    let now = Instant::now();
    let rate = KEPT.with_borrow_mut(|kept| {
        let entry = kept.get_mut(key)?;
        let pinch = entry
            .pinch
            .filter(|_| !matches!(phase, TouchPhase::Started))
            .map_or(
                Pinch {
                    rate: scale,
                    moved: now,
                },
                |pinch| pinch.after(scale, delta, now),
            );
        let ended = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);
        entry.pinch = (!ended).then_some(pinch);
        (ended && now.duration_since(pinch.moved) < STILL_PINCHING).then_some(pinch.rate)
    });
    let frame = current(key, drawn);
    let zoomed = zoom(key, frame, scale.log2(), position, most);
    let coast = rate.map_or(0.0, |rate| (rate - 1.0) * COAST_STEPS);
    if coast.abs() < SMALLEST_COAST {
        return zoomed;
    }
    let coasting = zoom(key, zoomed, coast, position, most);
    glide(key, zoomed, Pace::Coast);
    coasting
}

impl Pinch {
    /// After another `scale`, a change of `delta`, at `now`: recent changes
    /// are averaged, as one alone says little of the fingers' speed.
    fn after(self, scale: f64, delta: f64, now: Instant) -> Self {
        let recent = now.duration_since(self.moved) <= STILL_PINCHING;
        Self {
            rate: if recent {
                (scale + 2.0 * self.rate) / 3.0
            } else {
                scale
            },
            moved: if delta.abs() > PINCH_MOVES {
                now
            } else {
                self.moved
            },
        }
    }
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

/// What to do about telling the view that the map moved to a place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Tell {
    /// Tell it now.
    Now,
    /// Held back: ask [`late`] after this long.
    After(Duration),
    /// Held back, with [`late`] already due.
    Held,
}

/// Whether the map `key`, moved to `frame`, tells its view `now`.
pub(super) fn tell(key: &str, frame: Frame, now: Instant) -> Tell {
    KEPT.with_borrow_mut(|kept| {
        let Some(entry) = kept.get_mut(key) else {
            return Tell::Now;
        };
        if entry.untold.is_some() {
            entry.untold = Some(frame);
            return Tell::Held;
        }
        let since = entry.told.map(|(_, at)| now.saturating_duration_since(at));
        match since.filter(|since| *since < TELL_EVERY) {
            Some(since) => {
                entry.untold = Some(frame);
                Tell::After(TELL_EVERY - since)
            },
            None => {
                entry.told = Some((frame, now));
                Tell::Now
            },
        }
    })
}

/// The latest place held back from the view of the map `key`, now told.
/// `None` if the view moved the map since: its order stands.
pub(super) fn late(key: &str, now: Instant) -> Option<Frame> {
    KEPT.with_borrow_mut(|kept| {
        let entry = kept.get_mut(key)?;
        let frame = entry.untold.take()?;
        entry.told = Some((frame, now));
        Some(frame)
    })
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
/// frames see each other. A glide stops where it shows: the fingers take
/// the map from there, not from where it was going.
pub(super) fn current(key: &str, drawn: Frame) -> Frame {
    if gliding(key) {
        return shown(key, drawn);
    }
    moved(key).unwrap_or_else(|| Frame {
        bearing: KEPT.with_borrow(|kept| kept.get(key).map_or(drawn.bearing, |kept| kept.bearing)),
        ..drawn
    })
}

/// What the last frame showed of the map `key`, turned as it now is.
fn shown(key: &str, drawn: Frame) -> Frame {
    KEPT.with_borrow(|kept| {
        let kept = kept.get(key);
        Frame {
            bearing: kept.map_or(drawn.bearing, |kept| kept.bearing),
            ..kept.and_then(|kept| kept.drawn).unwrap_or(drawn)
        }
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
#[path = "view_tests.rs"]
mod tests;
