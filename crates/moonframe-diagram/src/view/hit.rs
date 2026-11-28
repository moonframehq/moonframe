//! What is under a point: a handle or port of the one chosen node, a node
//! (the topmost), or an edge (near its line or on its label).

use {
    crate::{
        geometry::{Point, Rect},
        layout::Route,
    },
    std::collections::{HashMap, HashSet},
};

/// A resize handle's corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Corner {
    // keep-sorted start
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
    // keep-sorted end
}

impl Corner {
    pub(crate) const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    /// Where on a box it sits, as fractions across and down.
    pub(crate) fn at(self, r: &Rect) -> Point {
        let (fx, fy) = match self {
            Self::TopLeft => (0.0, 0.0),
            Self::TopRight => (1.0, 0.0),
            Self::BottomLeft => (0.0, 1.0),
            Self::BottomRight => (1.0, 1.0),
        };
        Point::new(r.x + r.w * fx, r.y + r.h * fy)
    }

    /// The box `r` with this corner dragged by (dx, dy), never smaller than
    /// `least` either way.
    pub(crate) fn drag(self, r: &Rect, dx: f32, dy: f32, least: f32) -> Rect {
        let west = matches!(self, Self::TopLeft | Self::BottomLeft);
        let north = matches!(self, Self::TopLeft | Self::TopRight);
        let (mut x0, mut y0, mut x1, mut y1) = (r.x, r.y, r.right(), r.bottom());
        if west {
            x0 = (x0 + dx).min(x1 - least);
        } else {
            x1 = (x1 + dx).max(x0 + least);
        }
        if north {
            y0 = (y0 + dy).min(y1 - least);
        } else {
            y1 = (y1 + dy).max(y0 + least);
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

/// The four places on a box's sides where connectors start, a little out
/// from the edge so a press on the node near it still moves the node.
pub(crate) fn ports(r: &Rect, out: f32) -> [Point; 4] {
    let c = r.center();
    [
        Point::new(c.x, r.y - out),
        Point::new(r.right() + out, c.y),
        Point::new(c.x, r.bottom() + out),
        Point::new(r.x - out, c.y),
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Hit {
    // keep-sorted start
    Edge(String),
    Handle(String, Corner),
    Node(String),
    Nothing,
    Port(String),
    // keep-sorted end
}

/// What is under `p`. `reach` is how near, in diagram pixels, counts as on
/// a line or a handle; `handles` is the node whose handles and ports show.
pub(crate) fn hit(
    order: &[String],
    rects: &HashMap<String, Rect>,
    routes: &HashMap<String, Route>,
    p: Point,
    reach: f32,
    handles: Option<&str>,
    port_out: f32,
) -> Hit {
    if let Some(id) = handles
        && let Some(r) = rects.get(id)
    {
        for corner in Corner::ALL {
            if corner.at(r).distance(p) <= reach * 1.2 {
                return Hit::Handle(id.to_owned(), corner);
            }
        }
        if ports(r, port_out)
            .iter()
            .any(|q| q.distance(p) <= reach * 1.2)
        {
            return Hit::Port(id.to_owned());
        }
    }
    for id in order.iter().rev() {
        if rects.get(id).is_some_and(|r| r.contains(p)) {
            return Hit::Node(id.clone());
        }
    }
    let mut best: Option<(f32, &String)> = None;
    for (id, route) in routes {
        let near = route.label.map_or(f32::INFINITY, |l| l.distance(p) * 0.7);
        let line = route
            .points
            .windows(2)
            .map(|w| to_segment(p, w[0], w[1]))
            .fold(near, f32::min);
        if line <= reach && best.is_none_or(|(d, _)| line < d) {
            best = Some((line, id));
        }
    }
    best.map_or(Hit::Nothing, |(_, id)| Hit::Edge(id.clone()))
}

/// The nodes whose boxes share any area with `area`.
pub(crate) fn within(
    rects: &HashMap<String, Rect>,
    area: &Rect,
    order: &[String],
    also: &HashSet<String>,
) -> Vec<String> {
    order
        .iter()
        .filter(|id| also.contains(*id) || rects.get(*id).is_some_and(|r| r.overlaps(area)))
        .cloned()
        .collect()
}

fn to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx * dx + dy * dy;
    if length <= f32::EPSILON {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0);
    p.distance(Point::new(a.x + dx * t, a.y + dy * t))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_corner_drags_its_own_sides_and_keeps_a_least_size() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        assert_eq!(
            Corner::BottomRight.drag(&r, 10.0, 5.0, 12.0),
            Rect::new(0.0, 0.0, 110.0, 55.0)
        );
        assert_eq!(
            Corner::TopLeft.drag(&r, 10.0, 5.0, 12.0),
            Rect::new(10.0, 5.0, 90.0, 45.0)
        );
        assert_eq!(Corner::TopLeft.drag(&r, 500.0, 0.0, 12.0).w, 12.0);
    }

    #[test]
    fn the_topmost_node_wins_then_edges_near_their_line() {
        let rects = HashMap::from([
            ("a".to_string(), Rect::new(0.0, 0.0, 100.0, 100.0)),
            ("b".to_string(), Rect::new(50.0, 50.0, 100.0, 100.0)),
        ]);
        let order = vec!["a".to_string(), "b".to_string()];
        let route = Route {
            points: vec![Point::new(200.0, 0.0), Point::new(200.0, 100.0)],
            label: None,
        };
        let routes = HashMap::from([("e".to_string(), route)]);
        assert_eq!(
            hit(
                &order,
                &rects,
                &routes,
                Point::new(60.0, 60.0),
                6.0,
                None,
                12.0
            ),
            Hit::Node("b".into())
        );
        assert_eq!(
            hit(
                &order,
                &rects,
                &routes,
                Point::new(203.0, 50.0),
                6.0,
                None,
                12.0
            ),
            Hit::Edge("e".into())
        );
        assert_eq!(
            hit(
                &order,
                &rects,
                &routes,
                Point::new(300.0, 50.0),
                6.0,
                None,
                12.0
            ),
            Hit::Nothing
        );
    }
}
