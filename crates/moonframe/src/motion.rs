//! Pane animations. The view only says whether a pane is visible; this
//! remembers what each pane last was and when that changed, so a change
//! plays the pane's stylesheet transition instead of jumping.

use {
    gpui_kit::{Pixels, Size},
    moonframe_core::{Node, Transition, TransitionKind},
    std::{
        cell::Cell,
        collections::HashMap,
        rc::Rc,
        time::{Duration, Instant},
    },
};

#[derive(Default)]
pub(crate) struct Motion {
    panes: HashMap<String, Track>,
    /// What each container with keyed children drew in the frame being
    /// drawn, or the last one until the next begins.
    keyed: Keyed,
    /// Each animated keyed child's size at rest, measured as it is drawn,
    /// for how far it grows or shrinks.
    sizes: HashMap<String, Rc<Cell<Option<Size<Pixels>>>>>,
}

/// The keyed children containers drew in one frame, by container path.
#[derive(Default)]
pub(crate) struct Keyed {
    pub(crate) containers: HashMap<String, Drawn>,
}

/// One container's keyed children: every key, and a copy of each animated
/// child, so one that goes can still be drawn while it leaves.
#[derive(Default)]
pub(crate) struct Drawn {
    pub(crate) keys: Vec<String>,
    pub(crate) kept: Vec<Kept>,
}

pub(crate) struct Kept {
    pub(crate) key: String,
    /// Where it was among its siblings.
    pub(crate) index: usize,
    pub(crate) node: Node,
    pub(crate) transition: Transition,
}

/// One pane's last visibility, and when it started moving toward it.
struct Track {
    visible: bool,
    started: Option<Instant>,
}

/// How much of a pane shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shown {
    /// At rest, shown or hidden.
    Settled(bool),
    /// Mid-transition: from 0 (hidden) to 1 (shown), already eased.
    Moving(f32),
}

impl Shown {
    /// Hidden and at rest: nothing to draw.
    pub(crate) fn hidden(self) -> bool {
        self == Self::Settled(false)
    }

    /// How much shows while it moves.
    pub(crate) fn moving(self) -> Option<f32> {
        match self {
            Self::Moving(amount) => Some(amount),
            Self::Settled(_) => None,
        }
    }
}

impl Track {
    /// Linear progress from 0 to 1, or `None` once the move is over.
    fn progress(&self, now: Instant, duration: Duration) -> Option<f32> {
        let elapsed = now.saturating_duration_since(self.started?);
        let time = elapsed.as_secs_f32() / duration.as_secs_f32();
        (time < 1.0).then_some(time)
    }
}

impl Motion {
    /// Starts a frame: what keyed containers drew in the last one, which
    /// the new frame compares against and records anew.
    pub(crate) fn begin_frame(&mut self) -> Keyed {
        std::mem::take(&mut self.keyed)
    }

    /// Records what a keyed container drew this frame.
    pub(crate) fn drew(&mut self, container: String, drawn: Drawn) {
        self.keyed.containers.insert(container, drawn);
    }

    /// The pane at `key` has just appeared: it starts hidden, so being
    /// shown plays its way in.
    pub(crate) fn enter(&mut self, key: &str) {
        self.panes.insert(key.to_owned(), Track {
            visible: false,
            started: None,
        });
    }

    /// The pane at `key` is gone for good.
    pub(crate) fn forget(&mut self, key: &str) {
        self.panes.remove(key);
        self.sizes.remove(key);
    }

    /// Where the size of the keyed child at `key` is kept.
    pub(crate) fn size(&mut self, key: &str) -> Rc<Cell<Option<Size<Pixels>>>> {
        self.sizes.entry(key.to_owned()).or_default().clone()
    }

    /// How much of the pane at `key` shows at `now`, given what the view
    /// says it should be and how it moves.
    pub(crate) fn pane(
        &mut self,
        key: &str,
        visible: bool,
        transition: Option<&Transition>,
        now: Instant,
    ) -> Shown {
        let track = self.panes.entry(key.to_owned()).or_insert(Track {
            visible,
            started: None,
        });
        let Some(transition) = transition
            .filter(|transition| transition.kind != TransitionKind::None)
            .filter(|transition| !transition.duration.is_zero())
        else {
            *track = Track {
                visible,
                started: None,
            };
            return Shown::Settled(visible);
        };
        if track.visible != visible {
            // Turning back mid-move starts from where the pane is, not from
            // the far end.
            let done = track.progress(now, transition.duration).unwrap_or(1.0);
            let head_start = transition.duration.mul_f32(1.0 - done);
            track.visible = visible;
            track.started = Some(now.checked_sub(head_start).unwrap_or(now));
        }
        match track.progress(now, transition.duration) {
            Some(time) => {
                let eased = transition.curve.at(time);
                Shown::Moving(if visible {
                    eased
                } else {
                    1.0 - eased
                })
            },
            None => {
                track.started = None;
                Shown::Settled(visible)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, moonframe_core::Curve};

    /// Mid-move by `expected`, give or take float rounding.
    fn moving(shown: Shown, expected: f32) {
        let amount = shown.moving().unwrap_or(f32::NAN);
        assert!(
            (amount - expected).abs() < 1e-4,
            "{shown:?}, expected {expected}"
        );
    }

    const SLIDE: Transition = Transition {
        kind: TransitionKind::Slide,
        duration: Duration::from_millis(100),
        curve: Curve::Linear,
    };

    #[test]
    fn a_change_plays_and_then_settles() {
        let mut motion = Motion::default();
        let start = Instant::now();
        let mut pane = |visible, ms| {
            motion.pane(
                "p",
                visible,
                Some(&SLIDE),
                start + Duration::from_millis(ms),
            )
        };
        // The first frame is where the pane starts: no animation.
        assert_eq!(pane(true, 0), Shown::Settled(true));
        moving(pane(false, 10), 1.0);
        moving(pane(false, 60), 0.5);
        assert_eq!(pane(false, 110), Shown::Settled(false));
    }

    #[test]
    fn an_entering_pane_grows_from_nothing_and_a_forgotten_one_starts_over() {
        let mut motion = Motion::default();
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        motion.enter("tab");
        moving(motion.pane("tab", true, Some(&SLIDE), at(0)), 0.0);
        moving(motion.pane("tab", true, Some(&SLIDE), at(50)), 0.5);
        assert_eq!(
            motion.pane("tab", true, Some(&SLIDE), at(100)),
            Shown::Settled(true)
        );
        motion.size("tab").set(Some(Size::default()));
        motion.forget("tab");
        assert_eq!(motion.size("tab").get(), None);
        // Seen again without `enter`, it is simply there.
        assert_eq!(
            motion.pane("tab", true, Some(&SLIDE), at(200)),
            Shown::Settled(true)
        );
    }

    #[test]
    fn a_frame_hands_over_what_the_last_one_drew() {
        let mut motion = Motion::default();
        motion.drew("strip".to_owned(), Drawn {
            keys: vec!["a".to_owned()],
            kept: Vec::new(),
        });
        let last = motion.begin_frame();
        assert_eq!(last.containers["strip"].keys, ["a"]);
        assert!(motion.begin_frame().containers.is_empty());
    }

    #[test]
    fn turning_back_starts_from_where_it_is() {
        let mut motion = Motion::default();
        let start = Instant::now();
        let mut pane = |visible, ms| {
            motion.pane(
                "p",
                visible,
                Some(&SLIDE),
                start + Duration::from_millis(ms),
            )
        };
        pane(true, 0);
        pane(false, 0);
        // 30% hidden, then shown again: it is 70% shown and grows from there.
        moving(pane(true, 30), 0.7);
        assert_eq!(pane(true, 60), Shown::Settled(true));
    }

    #[test]
    fn no_transition_jumps() {
        let mut motion = Motion::default();
        let now = Instant::now();
        motion.pane("p", true, None, now);
        assert_eq!(motion.pane("p", false, None, now), Shown::Settled(false));
        let none = Transition {
            kind: TransitionKind::None,
            ..SLIDE
        };
        assert_eq!(
            motion.pane("p", true, Some(&none), now),
            Shown::Settled(true)
        );
    }
}
