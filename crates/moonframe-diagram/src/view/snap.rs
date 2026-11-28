//! Snapping a dragged selection to the nodes around it: its left, centre
//! or right lines up with another node's within a few screen pixels, and
//! the same for top, middle and bottom. Each snap shows a guide line along
//! the two nodes it lined up.

use crate::geometry::{Point, Rect};

/// How far the snap moved the selection, and the guides to draw.
pub(crate) struct Snapped {
    pub dx: f32,
    pub dy: f32,
    pub guides: Vec<(Point, Point)>,
}

/// Snaps `moving` (the dragged selection's box, already moved) to `others`
/// within `reach` diagram pixels.
pub(crate) fn snap<'a>(moving: Rect, others: impl IntoIterator<Item = &'a Rect>, reach: f32) -> Snapped {
    let xs = |r: &Rect| [r.x, r.x + r.w / 2.0, r.right()];
    let ys = |r: &Rect| [r.y, r.y + r.h / 2.0, r.bottom()];
    // The nearest line up on each axis: (distance, shift, the other node).
    let mut best_x: Option<(f32, f32, Rect)> = None;
    let mut best_y: Option<(f32, f32, Rect)> = None;
    for other in others {
        for a in xs(&moving) {
            for b in xs(other) {
                let d = (b - a).abs();
                if d <= reach && best_x.is_none_or(|(e, _, _)| d < e) {
                    best_x = Some((d, b - a, *other));
                }
            }
        }
        for a in ys(&moving) {
            for b in ys(other) {
                let d = (b - a).abs();
                if d <= reach && best_y.is_none_or(|(e, _, _)| d < e) {
                    best_y = Some((d, b - a, *other));
                }
            }
        }
    }
    let dx = best_x.map_or(0.0, |(_, s, _)| s);
    let dy = best_y.map_or(0.0, |(_, s, _)| s);
    let moved = moving.translate(dx, dy);
    let mut guides = Vec::new();
    if let Some((_, _, other)) = best_x {
        // The line both share, from the top of the higher to the bottom of
        // the lower.
        let x = xs(&moved).into_iter().find(|a| xs(&other).iter().any(|b| (a - b).abs() < 0.01)).unwrap_or(moved.x);
        guides.push((Point::new(x, moved.y.min(other.y)), Point::new(x, moved.bottom().max(other.bottom()))));
    }
    if let Some((_, _, other)) = best_y {
        let y = ys(&moved).into_iter().find(|a| ys(&other).iter().any(|b| (a - b).abs() < 0.01)).unwrap_or(moved.y);
        guides.push((Point::new(moved.x.min(other.x), y), Point::new(moved.right().max(other.right()), y)));
    }
    Snapped { dx, dy, guides }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_near_centre_snaps_and_shows_a_guide() {
        let other = Rect::new(0.0, 0.0, 100.0, 40.0);
        let moving = Rect::new(3.0, 200.0, 100.0, 40.0);
        let snapped = snap(moving, [&other], 6.0);
        assert_eq!(snapped.dx, -3.0);
        assert_eq!(snapped.dy, 0.0);
        assert_eq!(snapped.guides.len(), 1);
    }

    #[test]
    fn nothing_near_moves_nothing() {
        let snapped = snap(Rect::new(500.0, 500.0, 10.0, 10.0), [&Rect::new(0.0, 0.0, 10.0, 10.0)], 6.0);
        assert_eq!((snapped.dx, snapped.dy, snapped.guides.len()), (0.0, 0.0, 0));
    }
}
