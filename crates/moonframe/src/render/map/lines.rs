//! A map's routes: drawn points as they are, a `via` route along the
//! streets its router found (nothing until then, so a route never jumps
//! from a guess to the real way; straight and faint between its stops if
//! the router fails), each over a casing so it reads on any tile; and
//! telling the mod how long each routed one takes, once.

use {
    super::{
        Frame,
        routing::{self, Ask, Routed, Way},
    },
    crate::{color::ThemeColor, render::pointer::Sender},
    gpui_kit::{component::ActiveTheme, *},
    moonframe_core::{Data, Map, with_fields},
    std::{
        cell::RefCell,
        collections::{BTreeMap, HashMap},
        sync::Arc,
    },
};

/// How faint a route is drawn straight when its router failed.
const PENDING: f32 = 0.45;

/// One line to paint, in pixels from the box's corner.
struct Line {
    corners: Vec<(f32, f32)>,
    color: Option<Hsla>,
    stroke: f32,
    dashed: bool,
    /// Straight between stops: its router failed.
    pending: bool,
}

/// What the map's routes need painted.
pub(super) struct Lines {
    frame: Frame,
    lines: Vec<Line>,
}

/// The routed routes: whether any is still asked, and those found, with
/// their index and what they were asked by.
pub(super) struct Ways {
    pub(super) waiting: bool,
    pub(super) found: Vec<(usize, Ask, Arc<Way>)>,
}

/// The map's routes as lines in `frame`, asking the router for those
/// given as stops.
pub(super) fn lines(map: &Map, frame: Frame, cx: &App) -> (Lines, Ways) {
    let theme = cx.theme();
    let mut ways = Ways {
        waiting: false,
        found: Vec::new(),
    };
    let lines = map
        .routes
        .iter()
        .enumerate()
        .map(|(index, route)| {
            let (places, pending) = match Ask::of(route) {
                None => (route.points.clone(), false),
                Some(ask) => match routing::way(&ask) {
                    Routed::Found(way) => {
                        let points = way.points.clone();
                        ways.found.push((index, ask, way));
                        (points, false)
                    },
                    // Not drawn until its way arrives.
                    Routed::Waiting => {
                        ways.waiting = true;
                        (Vec::new(), false)
                    },
                    Routed::Failed(_) => (route.via.clone(), true),
                },
            };
            Line {
                corners: frame.route(&places),
                color: route.color.map(|color| color.hsla(theme)),
                stroke: route.stroke,
                dashed: route.dashed || pending,
                pending,
            }
        })
        .collect();
    (Lines { frame, lines }, ways)
}

impl Lines {
    /// A canvas over the box painting each line, in its color or
    /// `fallback`, over a `casing`.
    pub(super) fn paint(self, fallback: Hsla, casing: Hsla) -> Canvas<()> {
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                for line in &self.lines {
                    if line.corners.len() < 2 {
                        continue;
                    }
                    let corner = |pick: fn(f32, f32) -> f32, start: f32| {
                        line.corners.iter().fold((start, start), |(x, y), corner| {
                            (pick(x, corner.0), pick(y, corner.1))
                        })
                    };
                    let low = corner(f32::min, f32::INFINITY);
                    let high = corner(f32::max, f32::NEG_INFINITY);
                    let faint = if line.pending {
                        PENDING
                    } else {
                        1.0
                    };
                    let color = line.color.unwrap_or(fallback);
                    let stroke = line.stroke;
                    for copy in self.frame.copies(low, high, stroke + 3.0) {
                        let (dx, dy) = self.frame.copy_offset(copy);
                        let at = |&(x, y): &(f32, f32)| {
                            point(bounds.origin.x + px(x + dx), bounds.origin.y + px(y + dy))
                        };
                        for (width, paint, dash) in [
                            (stroke + 3.0, casing.opacity(0.85 * faint), false),
                            (stroke, color.opacity(faint), line.dashed),
                        ] {
                            let mut path = PathBuilder::stroke(px(width));
                            if dash {
                                path = path.dash_array(&[px(stroke * 2.0), px(stroke * 1.5)]);
                            }
                            path.move_to(at(&line.corners[0]));
                            for corner in &line.corners[1..] {
                                path.line_to(at(corner));
                            }
                            if let Ok(path) = path.build() {
                                window.paint_path(path, paint);
                            }
                        }
                    }
                }
            },
        )
        .absolute()
        .size_full()
    }
}

thread_local! {
    /// What each map's routes last told their mod, by map and route index:
    /// a route tells again whenever its stops or mode change, back and
    /// forth too, as the mod may have dropped what it was told.
    static TOLD: RefCell<HashMap<String, Announcements>> = RefCell::new(HashMap::new());
}

#[derive(Default)]
struct Announcements {
    frame: u64,
    routes: HashMap<usize, Ask>,
}

impl Announcements {
    fn take(
        &mut self,
        frame: u64,
        asked: &[Option<Ask>],
        found: Vec<(usize, Ask, Arc<Way>)>,
    ) -> Vec<(usize, Arc<Way>)> {
        // A remounted map may belong to a mod whose state started over.
        if frame > 0 && self.frame + 1 < frame {
            self.routes.clear();
        }
        self.frame = frame;
        // Track the inputs, including drawn, removed and pending routes,
        // rather than only replacing the last successfully announced way.
        self.routes
            .retain(|index, previous| asked.get(*index).and_then(Option::as_ref) == Some(previous));
        found
            .into_iter()
            .filter_map(|(index, ask, way)| {
                let new = self.routes.insert(index, ask.clone()).as_ref() != Some(&ask);
                new.then_some((index, way))
            })
            .collect()
    }
}

/// Tells the mod, with `routed`, how long each newly found way takes.
pub(super) fn announce(
    key: &str,
    frame: u64,
    map: &Map,
    found: Vec<(usize, Ask, Arc<Way>)>,
    sender: &Sender,
) -> impl IntoElement {
    let key = key.to_owned();
    let asked: Vec<_> = map.routes.iter().map(Ask::of).collect();
    let routed = map.routed.clone();
    let sender = sender.clone();
    canvas(
        move |_, window, cx| {
            let Some(msg) = routed else {
                TOLD.with_borrow_mut(|told| told.remove(&key));
                return;
            };
            let newly_found = TOLD
                .with_borrow_mut(|told| told.entry(key).or_default().take(frame, &asked, found));
            for (index, way) in newly_found {
                let fields = BTreeMap::from([
                    ("route".to_owned(), Data::Number((index + 1) as f64)),
                    ("seconds".to_owned(), Data::Number(way.seconds.round())),
                    ("meters".to_owned(), Data::Number(way.meters.round())),
                ]);
                let msg = with_fields(&msg, fields);
                let sender = sender.clone();
                // After the frame: the window is still drawing this one, and a
                // deferred update would find it busy and be dropped.
                window
                    .spawn(cx, async move |cx| {
                        let _ = cx.update(|window, cx| sender.send(msg, window, cx));
                    })
                    .detach();
            }
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_0()
}

#[cfg(test)]
mod tests {
    use {
        super::{Announcements, Arc, Ask, Way},
        moonframe_core::{Route, TravelMode},
    };

    #[test]
    fn returning_to_a_route_announces_again_even_if_its_replacement_never_arrived() {
        let route = Route {
            points: Vec::new(),
            via: vec![[0.0, 0.0], [0.0, 1.0]],
            mode: TravelMode::Walk,
            color: None,
            stroke: 4.0,
            dashed: false,
        };
        let walk = Ask::of(&route);
        let bike = Ask::of(&Route {
            mode: TravelMode::Bike,
            ..route
        });
        let way = Arc::new(Way {
            points: Vec::new(),
            seconds: 60.0,
            meters: 100.0,
        });
        let found = || {
            walk.clone()
                .map(|ask| (0, ask, way.clone()))
                .into_iter()
                .collect()
        };
        let asked = [walk.clone()];
        let mut told = Announcements::default();
        assert_eq!(told.take(1, &asked, found()).len(), 1);
        assert!(told.take(2, &asked, found()).is_empty());
        // Walk -> Drawn -> Walk; then Walk -> pending Bike -> Walk.
        for (index, replacement) in [None, bike].into_iter().enumerate() {
            let frame = 3 + 2 * index as u64;
            assert!(told.take(frame, &[replacement], Vec::new()).is_empty());
            assert_eq!(told.take(frame + 1, &asked, found()).len(), 1);
        }
        // Removing the route entirely also invalidates its announcement.
        assert!(told.take(7, &[], Vec::new()).is_empty());
        assert_eq!(told.take(8, &asked, found()).len(), 1);
        // A map absent for a frame is newly mounted when it returns.
        assert_eq!(told.take(10, &asked, found()).len(), 1);
    }
}
