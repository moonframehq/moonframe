//! A two-finger swipe, out of the scrolls it is made of. macOS sends a
//! swipe event only for three fingers, and only where the system's
//! trackpad settings leave those to apps; by default they switch spaces and
//! two fingers swipe between pages, which reaches an app as plain scrolling.

use super::SwipeDirection;

/// How far the fingers go across, in points, to count as a swipe.
const ACROSS: f64 = 120.0;
/// How many times farther across than up or down they go.
const STRAIGHT: f64 = 2.0;

/// The scroll under way, from its first touch.
#[derive(Default)]
pub(super) struct Scrolls {
    across: f64,
    down: f64,
}

/// How far through a scroll an event is.
#[derive(Clone, Copy)]
pub(super) enum Stroke {
    Began,
    Changed,
    Ended,
    Cancelled,
}

impl Scrolls {
    /// Takes one event of a scroll, `dx` and `dy` the way the fingers
    /// went, with `dx` positive to the right: the swipe it finishes, if
    /// it finishes one.
    pub(super) fn feed(&mut self, stroke: Stroke, dx: f64, dy: f64) -> Option<SwipeDirection> {
        if matches!(stroke, Stroke::Began) {
            *self = Self::default();
        }
        self.across += dx;
        self.down += dy;
        let (across, down) = (self.across, self.down);
        if !matches!(stroke, Stroke::Ended | Stroke::Cancelled) {
            return None;
        }
        *self = Self::default();
        let swiped = matches!(stroke, Stroke::Ended)
            && across.abs() >= ACROSS
            && across.abs() >= down.abs() * STRAIGHT;
        swiped.then_some(if across > 0.0 {
            SwipeDirection::Back
        } else {
            SwipeDirection::Forward
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Scrolls, Stroke, SwipeDirection};

    fn stroke(steps: &[(f64, f64)], end: Stroke) -> Option<SwipeDirection> {
        let mut scrolls = Scrolls::default();
        assert_eq!(scrolls.feed(Stroke::Began, 0.0, 0.0), None);
        for &(dx, dy) in steps {
            assert_eq!(scrolls.feed(Stroke::Changed, dx, dy), None);
        }
        scrolls.feed(end, 0.0, 0.0)
    }

    #[test]
    fn a_long_straight_stroke_is_a_swipe_when_the_fingers_lift() {
        assert_eq!(
            stroke(&[(70.0, 2.0), (70.0, -3.0)], Stroke::Ended),
            Some(SwipeDirection::Back)
        );
        assert_eq!(
            stroke(&[(-130.0, 0.0)], Stroke::Ended),
            Some(SwipeDirection::Forward)
        );
    }

    #[test]
    fn short_slanted_and_abandoned_strokes_are_scrolls() {
        assert_eq!(stroke(&[(100.0, 0.0)], Stroke::Ended), None);
        assert_eq!(stroke(&[(200.0, 150.0)], Stroke::Ended), None);
        assert_eq!(stroke(&[(0.0, 400.0)], Stroke::Ended), None);
        assert_eq!(stroke(&[(300.0, 0.0)], Stroke::Cancelled), None);
    }

    #[test]
    fn each_stroke_counts_from_its_own_start() {
        let mut scrolls = Scrolls::default();
        scrolls.feed(Stroke::Changed, 500.0, 0.0);
        scrolls.feed(Stroke::Began, 0.0, 0.0);
        scrolls.feed(Stroke::Changed, 10.0, 0.0);
        assert_eq!(scrolls.feed(Stroke::Ended, 0.0, 0.0), None);
    }
}
