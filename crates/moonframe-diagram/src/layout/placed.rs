//! A flow whose nodes have places of their own (an editor's): they stay
//! where they are, a node without one is put beside a node it is linked
//! to, and edges are routed between them around the other nodes.

use {
    super::{Layout, Measure, Options, Route, layered::route::simplify, node_size},
    crate::{
        geometry::{Point, Rect, bounds},
        model::Diagram,
    },
    std::collections::HashMap,
};

/// How far a bend must keep from a node it goes around.
const CLEARANCE: f32 = 14.0;
/// The most boxes in the way a route tries to go around.
const MOST_DETOURS: usize = 6;
/// The side of a cell of the obstacles' grid.
const CELL: f32 = 160.0;
/// What passing through one box costs: more than any length.
const BLOCKED: f32 = 10_000.0;
/// The least length of the last leg, so an arrowhead fits.
const LAST_LEG: f32 = 14.0;

pub(super) fn layout(diagram: &Diagram, measure: &dyn Measure, options: &Options) -> Layout {
    let mut out = Layout::default();
    for node in &diagram.nodes {
        if let Some(at) = node.at {
            out.nodes.insert(node.id.clone(), at);
        }
    }
    // Unplaced nodes go beside a placed neighbour, else under everything.
    for node in diagram.nodes.iter().filter(|n| n.at.is_none()) {
        let size = node_size(node, measure, options);
        let beside = diagram.edges.iter().find_map(|e| {
            if e.to == node.id {
                out.nodes.get(&e.from).map(|r| Point::new(r.right() + options.node_gap * 2.0, r.y))
            } else if e.from == node.id {
                out.nodes.get(&e.to).map(|r| Point::new(r.x - options.node_gap * 2.0 - size.w, r.y))
            } else {
                None
            }
        });
        let start = beside.unwrap_or_else(|| {
            let all = bounds(out.nodes.values()).unwrap_or_default();
            Point::new(all.x, all.bottom() + options.rank_gap)
        });
        let mut rect = Rect::new(start.x, start.y, size.w, size.h);
        while out.nodes.values().any(|r| r.inflate(options.node_gap / 2.0).overlaps(&rect)) {
            rect.y += size.h + options.node_gap;
        }
        out.nodes.insert(node.id.clone(), rect);
    }
    let rects: Vec<Rect> = out.nodes.values().copied().collect();
    let obstacles = Obstacles::new(&rects);
    for edge in &diagram.edges {
        let (Some(a), Some(b)) = (out.nodes.get(&edge.from), out.nodes.get(&edge.to)) else {
            continue;
        };
        let route = if edge.from == edge.to {
            self_loop(a)
        } else {
            route_through(a, b, &obstacles)
        };
        out.edges.insert(edge.id.clone(), route);
    }
    out
}

fn self_loop(r: &Rect) -> Route {
    let (x, cy) = (r.right(), r.center().y);
    let points = vec![Point::new(x, cy - 8.0), Point::new(x + 20.0, cy - 8.0), Point::new(x + 20.0, cy + 8.0), Point::new(x, cy + 8.0)];
    Route { label: Some(Point::new(x + 24.0, cy)), points }
}

/// Routes every edge again from `rects` (by node id), as the view does
/// while nodes are dragged: only edges touching `moved` are worked out.
pub fn reroute(
    edges: impl IntoIterator<Item = (String, String, String)>,
    rects: &HashMap<String, Rect>,
    moved: &dyn Fn(&str) -> bool,
    routes: &mut HashMap<String, Route>,
) {
    let all: Vec<Rect> = rects.values().copied().collect();
    let obstacles = Obstacles::new(&all);
    for (id, from, to) in edges {
        if !(moved(&from) || moved(&to)) {
            continue;
        }
        let (Some(a), Some(b)) = (rects.get(&from), rects.get(&to)) else {
            continue;
        };
        let route = if from == to { self_loop(a) } else { route_through(a, b, &obstacles) };
        routes.insert(id, route);
    }
}

/// An orthogonal route from box `a` to box `b`, avoiding `obstacles` (which
/// may hold `a` and `b`): the cheapest of a few shapes (one bend, two
/// bends with their middle leg moved clear of what is in the way), scored
/// by what they cross, then length and bends.
pub fn route_between(a: &Rect, b: &Rect, obstacles: &[Rect]) -> Route {
    route_through(a, b, &Obstacles::new(obstacles))
}

/// [`route_between`] with the obstacles already indexed, for many routes
/// among the same boxes.
pub fn route_through(a: &Rect, b: &Rect, obstacles: &Obstacles) -> Route {
    let (ac, bc) = (a.center(), b.center());
    // Each leg leaves and enters by the side facing where it goes.
    let side_x = |r: &Rect, toward: f32| if toward >= r.center().x { r.right() } else { r.x };
    let side_y = |r: &Rect, toward: f32| if toward >= r.center().y { r.bottom() } else { r.y };

    let side_to_side = |mx: f32| {
        let (sx, ex) = (side_x(a, mx), side_x(b, mx));
        simplify(vec![Point::new(sx, ac.y), Point::new(mx, ac.y), Point::new(mx, bc.y), Point::new(ex, bc.y)])
    };
    let end_to_end = |my: f32| {
        let (sy, ey) = (side_y(a, my), side_y(b, my));
        simplify(vec![Point::new(ac.x, sy), Point::new(ac.x, my), Point::new(bc.x, my), Point::new(bc.x, ey)])
    };
    // The plain shapes: two bends with the middle leg halfway, either way,
    // and one bend either way.
    let plain = [
        side_to_side((side_x(a, bc.x) + side_x(b, ac.x)) / 2.0),
        end_to_end((side_y(a, bc.y) + side_y(b, ac.y)) / 2.0),
        simplify(vec![Point::new(side_x(a, bc.x), ac.y), Point::new(bc.x, ac.y), Point::new(bc.x, side_y(b, ac.y))]),
        simplify(vec![Point::new(ac.x, side_y(a, bc.y)), Point::new(ac.x, bc.y), Point::new(side_x(b, ac.x), bc.y)]),
    ];
    let cheapest = |routes: Vec<Vec<Point>>| {
        routes
            .into_iter()
            .map(|r| (cost(&r, obstacles), r))
            .min_by(|p, q| p.0.total_cmp(&q.0))
            .unwrap_or_default()
    };
    let (mut best_cost, mut best) = cheapest(plain.to_vec());
    if best_cost >= BLOCKED {
        // Something is in the way of every plain shape: try the middle leg
        // just clear of each box the plain shapes cross.
        let mut detours = Vec::new();
        let mut blocking: Vec<Rect> = plain.iter().flat_map(|r| r.windows(2).flat_map(|w| obstacles.crossing(w[0], w[1]))).collect();
        blocking.dedup();
        for o in blocking.iter().take(MOST_DETOURS) {
            detours.push(side_to_side(o.x - CLEARANCE));
            detours.push(side_to_side(o.right() + CLEARANCE));
            detours.push(end_to_end(o.y - CLEARANCE));
            detours.push(end_to_end(o.bottom() + CLEARANCE));
        }
        let (detour_cost, detour) = cheapest(detours);
        if detour_cost < best_cost {
            (best_cost, best) = (detour_cost, detour);
        }
    }
    let _ = best_cost;
    Route { label: Route::middle(&best), points: best }
}

/// What a route costs: each box it passes through costs far more than any
/// length; then its length, its bends, a last leg too short for an
/// arrowhead, and legs that double back.
fn cost(points: &[Point], obstacles: &Obstacles) -> f32 {
    let mut crossed = 0;
    let mut length = 0.0;
    for w in points.windows(2) {
        length += w[0].distance(w[1]);
        crossed += obstacles.crossing(w[0], w[1]).len();
    }
    let last = match points {
        [.., p, q] => p.distance(*q),
        _ => 0.0,
    };
    let short = if last < LAST_LEG { 400.0 } else { 0.0 };
    let first = match points {
        [p, q, ..] => p.distance(*q),
        _ => 0.0,
    };
    let stub = if first < LAST_LEG / 2.0 { 200.0 } else { 0.0 };
    crossed as f32 * BLOCKED + length + points.len() as f32 * 30.0 + short + stub
}

/// Boxes to route around, in a grid of cells so a leg only looks at the
/// boxes near it.
pub struct Obstacles {
    rects: Vec<Rect>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl Obstacles {
    pub fn new(rects: &[Rect]) -> Self {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, r) in rects.iter().enumerate() {
            for cell in cells_of(r.x, r.y, r.right(), r.bottom()) {
                cells.entry(cell).or_default().push(i as u32);
            }
        }
        Self { rects: rects.to_vec(), cells }
    }

    /// The boxes the level or upright leg from `a` to `b` passes through.
    pub fn crossing(&self, a: Point, b: Point) -> Vec<Rect> {
        let mut seen: Vec<u32> = cells_of(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
            .filter_map(|cell| self.cells.get(&cell))
            .flatten()
            .copied()
            .collect();
        seen.sort_unstable();
        seen.dedup();
        seen.into_iter()
            .map(|i| self.rects[i as usize])
            .filter(|r| r.crossed_by(a, b))
            .collect()
    }
}

fn cells_of(x0: f32, y0: f32, x1: f32, y1: f32) -> impl Iterator<Item = (i32, i32)> {
    let cell = |v: f32| (v / CELL).floor() as i32;
    let (cx0, cy0, cx1, cy1) = (cell(x0), cell(y0), cell(x1), cell(y1));
    (cx0..=cx1).flat_map(move |cx| (cy0..=cy1).map(move |cy| (cx, cy)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn side_by_side_boxes_join_straight() {
        let a = Rect::new(0.0, 0.0, 100.0, 40.0);
        let b = Rect::new(200.0, 0.0, 100.0, 40.0);
        let route = route_between(&a, &b, &[a, b]);
        assert_eq!(route.points, vec![Point::new(100.0, 20.0), Point::new(200.0, 20.0)]);
    }

    #[test]
    fn a_box_in_the_way_is_gone_around() {
        let a = Rect::new(0.0, 0.0, 100.0, 40.0);
        let b = Rect::new(400.0, 0.0, 100.0, 40.0);
        let wall = Rect::new(200.0, -10.0, 60.0, 60.0);
        let route = route_between(&a, &b, &[a, b, wall]);
        for w in route.points.windows(2) {
            assert!(!wall.crossed_by(w[0], w[1]), "{:?}", route.points);
        }
    }
}
