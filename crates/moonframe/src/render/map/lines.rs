//! A map's routes: drawn points as they are, a `via` route along the
//! streets its router found (or, until then, straight and faint between
//! its stops), each over a casing so it reads on any tile; and telling
//! the mod how long each routed one takes, once.

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
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
};

/// How faint a route is while its way along streets is not known.
const PENDING: f32 = 0.45;

/// One line to paint, in pixels from the box's corner.
struct Line {
    corners: Vec<(f32, f32)>,
    color: Option<Hsla>,
    stroke: f32,
    dashed: bool,
    /// Straight between stops while the router answers.
    pending: bool,
}

/// What the map's routes need painted.
pub(super) struct Lines(Vec<Line>);

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
                    Routed::Waiting => {
                        ways.waiting = true;
                        (route.via.clone(), true)
                    },
                    Routed::Failed(_) => (route.via.clone(), true),
                },
            };
            Line {
                corners: places
                    .iter()
                    .map(|&[lat, lon]| frame.at(lat, lon))
                    .collect(),
                color: route.color.map(|color| color.hsla(theme)),
                stroke: route.stroke,
                dashed: route.dashed || pending,
                pending,
            }
        })
        .collect();
    (Lines(lines), ways)
}

impl Lines {
    /// A canvas over the box painting each line, in its color or
    /// `fallback`, over a `casing`.
    pub(super) fn paint(self, fallback: Hsla, casing: Hsla) -> Canvas<()> {
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                for line in &self.0 {
                    if line.corners.len() < 2 {
                        continue;
                    }
                    let at = |&(x, y): &(f32, f32)| {
                        point(bounds.origin.x + px(x), bounds.origin.y + px(y))
                    };
                    let points: Vec<Point<Pixels>> = line.corners.iter().map(at).collect();
                    let faint = if line.pending {
                        PENDING
                    } else {
                        1.0
                    };
                    let color = line.color.unwrap_or(fallback);
                    let stroke = line.stroke;
                    for (width, paint, dash) in [
                        (stroke + 3.0, casing.opacity(0.85 * faint), false),
                        (stroke, color.opacity(faint), line.dashed),
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
}

thread_local! {
    /// The routed routes each map has told its mod of: by map, index and
    /// what it was asked by, so a route is told again once its stops change.
    static TOLD: RefCell<HashSet<(String, usize, Ask)>> = RefCell::new(HashSet::new());
}

/// Tells the mod, with `routed`, how long each newly found way takes.
pub(super) fn announce(
    key: &str,
    found: Vec<(usize, Ask, Arc<Way>)>,
    routed: Option<Data>,
    sender: &Sender,
) -> impl IntoElement {
    let key = key.to_owned();
    let sender = sender.clone();
    canvas(
        move |_, window, cx| {
            let Some(msg) = routed else {
                return;
            };
            for (index, ask, way) in found {
                let new = TOLD.with_borrow_mut(|told| told.insert((key.clone(), index, ask)));
                if !new {
                    continue;
                }
                let fields = BTreeMap::from([
                    ("route".to_owned(), Data::Number((index + 1) as f64)),
                    ("seconds".to_owned(), Data::Number(way.seconds.round())),
                    ("meters".to_owned(), Data::Number(way.meters.round())),
                ]);
                let msg = with_fields(&msg, fields);
                let sender = sender.clone();
                window.defer(cx, move |window, cx| sender.send(msg, window, cx));
            }
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_0()
}
