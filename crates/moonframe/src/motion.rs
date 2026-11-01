//! Pane animations. The view only says whether a pane is visible; this
//! remembers what each pane last was and when that changed, so a change
//! plays the pane's stylesheet transition instead of jumping. The same for
//! an element's named `state`: its look eases from one state's to the next.

use {
    gpui_kit::{Hsla, Pixels, Rgba, ScrollHandle, Size},
    moonframe_core::{Curve, Node, Transition, TransitionKind},
    std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        rc::Rc,
        time::{Duration, Instant},
    },
};

mod slow;

pub(crate) use slow::{is_slow, now, set_slow};

pub(crate) struct Motion {
    panes: Recent<Track>,
    /// What each container with keyed children drew in the frame being
    /// drawn, or the last one until the next begins.
    keyed: Keyed,
    /// Each animated keyed child's size at rest, measured as it is drawn,
    /// for how far it grows or shrinks.
    sizes: Recent<Rc<Cell<Option<Size<Pixels>>>>>,
    /// Elements with a named state, each between two looks: drawn while
    /// styles resolve, which only borrow the painter.
    pub(crate) tweens: RefCell<Tweens>,
    /// Where each `ui.list` is scrolled, by path.
    scrolls: Recent<ScrollHandle>,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            panes: Recent::default(),
            keyed: Keyed::default(),
            sizes: Recent::default(),
            tweens: RefCell::default(),
            scrolls: Recent::keeping(FORGET_SCROLL),
        }
    }
}

/// Entries by path, each stamped when last used; those not used for
/// [`FORGET`] go as the map grows, so a long list's rows, each with a
/// track of its own, do not pile up for the app's life.
struct Recent<T> {
    entries: HashMap<String, (T, Instant)>,
    /// How long an unused entry is kept.
    keep: Duration,
}

impl<T> Default for Recent<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            keep: FORGET,
        }
    }
}

impl<T> Recent<T> {
    /// Kept for `keep` unused instead.
    fn keeping(keep: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            keep,
        }
    }

    /// The entry at `key`, made with `make` if there is none, used `now`.
    fn get_or(&mut self, key: &str, now: Instant, make: impl FnOnce() -> T) -> &mut T {
        if !self.entries.contains_key(key) {
            self.sweep(now);
        }
        let (value, seen) = self
            .entries
            .entry(key.to_owned())
            .or_insert_with(|| (make(), now));
        *seen = now;
        value
    }

    fn insert(&mut self, key: &str, value: T, now: Instant) {
        self.sweep(now);
        self.entries.insert(key.to_owned(), (value, now));
    }

    fn remove(&mut self, key: &str) {
        self.entries.remove(key);
    }

    /// Every 64 new entries, the ones unused for too long go.
    fn sweep(&mut self, now: Instant) {
        if self.entries.len().is_multiple_of(64) {
            let keep = self.keep;
            self.entries
                .retain(|_, (_, seen)| now.saturating_duration_since(*seen) < keep);
        }
    }
}

/// The animated part of a look, in the theme's colors.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Snapshot {
    pub(crate) background: Option<Hsla>,
    pub(crate) color: Option<Hsla>,
    pub(crate) border_color: Option<Hsla>,
    pub(crate) opacity: Option<f32>,
    pub(crate) width: Option<f32>,
    pub(crate) height: Option<f32>,
}

impl Snapshot {
    /// `self` moved `amount` (0 to 1) toward `to`. What one of them lacks
    /// jumps to `to`, as there is nothing to ease from or toward.
    fn toward(&self, to: &Self, amount: f32) -> Self {
        let number = |from: Option<f32>, to: Option<f32>| match (from, to) {
            (Some(from), Some(to)) => Some(from + (to - from) * amount),
            (_, to) => to,
        };
        let color = |from: Option<Hsla>, to: Option<Hsla>| match (from, to) {
            (Some(from), Some(to)) => Some(mix(from, to, amount)),
            (_, to) => to,
        };
        Self {
            background: color(self.background, to.background),
            color: color(self.color, to.color),
            border_color: color(self.border_color, to.border_color),
            opacity: match (self.opacity, to.opacity) {
                (None, None) => None,
                (from, to) => number(Some(from.unwrap_or(1.0)), Some(to.unwrap_or(1.0))),
            },
            width: number(self.width, to.width),
            height: number(self.height, to.height),
        }
    }
}

/// Two colors mixed in RGB, which never swings round the hue circle.
fn mix(from: Hsla, to: Hsla, amount: f32) -> Hsla {
    let (from, to) = (Rgba::from(from), Rgba::from(to));
    let channel = |a: f32, b: f32| a + (b - a) * amount;
    Hsla::from(Rgba {
        r: channel(from.r, to.r),
        g: channel(from.g, to.g),
        b: channel(from.b, to.b),
        a: channel(from.a, to.a),
    })
}

/// Elements' moves between named states, by path.
#[derive(Default)]
pub(crate) struct Tweens {
    elements: HashMap<String, Tween>,
}

struct Tween {
    state: Option<String>,
    from: Snapshot,
    to: Snapshot,
    /// When the move begins (later than the change, by the transition's
    /// delay), and how it runs; `None` at rest.
    moving: Option<(Instant, Duration, Curve)>,
    /// The last frame that drew it, to forget elements that went.
    seen: Instant,
}

/// Elements not drawn for this long are forgotten, and start at rest.
const FORGET: Duration = Duration::from_secs(10);
/// Where a list not drawn for this long was scrolled is forgotten: kept
/// longer, so a tab left for a while opens where it was.
const FORGET_SCROLL: Duration = Duration::from_secs(600);

impl Tween {
    /// Where it is at `now`, and whether it is still moving.
    fn at(&mut self, now: Instant) -> (Snapshot, bool) {
        let Some((started, duration, curve)) = self.moving else {
            return (self.to, false);
        };
        let time = now.saturating_duration_since(started).as_secs_f32() / duration.as_secs_f32();
        if time >= 1.0 {
            self.moving = None;
            return (self.to, false);
        }
        (self.from.toward(&self.to, curve.at(time)), true)
    }
}

impl Tweens {
    /// The look of the element at `path` at `now`: `target`, the look of
    /// its `state`, or on the way there from the last state's when the
    /// state changed and `transition` says how to move. Whether it moves.
    pub(crate) fn step(
        &mut self,
        path: &str,
        state: Option<&str>,
        target: Snapshot,
        transition: Option<&Transition>,
        now: Instant,
    ) -> (Snapshot, bool) {
        let Some(tween) = self.elements.get_mut(path) else {
            // First seen: where it is, not where it came from.
            if self.elements.len().is_multiple_of(64) {
                self.elements
                    .retain(|_, tween| now.saturating_duration_since(tween.seen) < FORGET);
            }
            self.elements.insert(path.to_owned(), Tween {
                state: state.map(str::to_owned),
                from: target,
                to: target,
                moving: None,
                seen: now,
            });
            return (target, false);
        };
        tween.seen = now;
        if tween.state.as_deref() != state {
            let (here, _) = tween.at(now);
            tween.state = state.map(str::to_owned);
            tween.from = here;
            tween.moving = transition
                .filter(|transition| transition.kind != TransitionKind::None)
                .filter(|transition| !transition.duration.is_zero())
                .map(|transition| {
                    // Until its delay is over it holds where it was.
                    (
                        now + transition.delay,
                        transition.duration,
                        transition.curve,
                    )
                });
        }
        // The theme may change mid-move: aim at the current look.
        tween.to = target;
        tween.at(now)
    }
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
    /// Shared from frame to frame while it does not change, so a child at
    /// rest is compared, not copied, each frame.
    pub(crate) node: Rc<Node>,
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
        self.panes.insert(
            key,
            Track {
                visible: false,
                started: None,
            },
            Instant::now(),
        );
    }

    /// The pane at `key` is gone for good.
    pub(crate) fn forget(&mut self, key: &str) {
        self.panes.remove(key);
        self.sizes.remove(key);
    }

    /// Where the list or scrolling frame at `path` is scrolled.
    pub(crate) fn scroll(&mut self, path: &str) -> ScrollHandle {
        self.scrolls
            .get_or(path, Instant::now(), ScrollHandle::default)
            .clone()
    }

    /// Where the size of the keyed child at `key` is kept.
    pub(crate) fn size(&mut self, key: &str) -> Rc<Cell<Option<Size<Pixels>>>> {
        self.sizes.get_or(key, Instant::now(), Rc::default).clone()
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
        let track = self.panes.get_or(key, now, || Track {
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
            // the far end: from the time at which the eased way back shows
            // what shows now, which for an asymmetric curve is not simply
            // the time left.
            let done = track.progress(now, transition.duration).unwrap_or(1.0);
            let shown_from_far_end = 1.0 - transition.curve.at(done);
            let head_start = transition
                .duration
                .mul_f32(time_at(&transition.curve, shown_from_far_end));
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

/// The time (0 to 1) at which `curve` reaches `progress`, by bisection:
/// curves only ever grow with time.
fn time_at(curve: &Curve, progress: f32) -> f32 {
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..32 {
        let middle = (low + high) / 2.0;
        if curve.at(middle) < progress {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
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
        delay: Duration::ZERO,
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
    fn turning_back_on_an_asymmetric_curve_does_not_jump() {
        let slide = Transition {
            curve: Curve::EASE_OUT,
            ..SLIDE
        };
        let mut motion = Motion::default();
        let start = Instant::now();
        let mut pane = |visible, ms| {
            motion.pane(
                "p",
                visible,
                Some(&slide),
                start + Duration::from_millis(ms),
            )
        };
        pane(false, 0);
        pane(true, 0);
        // Halfway in, in time: ease-out has it well over half shown.
        let shown = Curve::EASE_OUT.at(0.5);
        assert!(shown > 0.6);
        moving(pane(true, 50), shown);
        // Turned back on that frame: it starts from there, not from 1 - shown.
        moving(pane(false, 50), shown);
        // And then keeps hiding.
        let later = pane(false, 60).moving().unwrap_or(f32::NAN);
        assert!(later < shown, "{later} should be below {shown}");
    }

    fn snapshot(width: f32) -> Snapshot {
        Snapshot {
            width: Some(width),
            ..Snapshot::default()
        }
    }

    #[test]
    fn a_new_state_eases_from_the_old_look_and_settles() {
        let mut tweens = Tweens::default();
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let step = |tweens: &mut Tweens, state, width, ms| {
            tweens.step("drawer", Some(state), snapshot(width), Some(&SLIDE), at(ms))
        };
        // First seen: at rest, wherever it is.
        assert_eq!(
            step(&mut tweens, "closed", 40.0, 0),
            (snapshot(40.0), false)
        );
        assert_eq!(step(&mut tweens, "open", 240.0, 10), (snapshot(40.0), true));
        assert_eq!(
            step(&mut tweens, "open", 240.0, 60),
            (snapshot(140.0), true)
        );
        // Turned back halfway: it eases back from where it is.
        let (back, moving) = step(&mut tweens, "closed", 40.0, 60);
        assert_eq!((back, moving), (snapshot(140.0), true));
        assert_eq!(
            step(&mut tweens, "closed", 40.0, 110),
            (snapshot(90.0), true)
        );
        assert_eq!(
            step(&mut tweens, "closed", 40.0, 200),
            (snapshot(40.0), false)
        );
        // Without a transition, a change jumps.
        let jump = tweens.step("drawer", Some("open"), snapshot(240.0), None, at(300));
        assert_eq!(jump, (snapshot(240.0), false));
    }

    #[test]
    fn a_delayed_state_change_holds_then_moves() {
        let mut tweens = Tweens::default();
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let later = Transition {
            delay: Duration::from_millis(50),
            ..SLIDE
        };
        let mut step = |state, width, ms| {
            tweens.step("label", Some(state), snapshot(width), Some(&later), at(ms))
        };
        step("shown", 100.0, 0);
        // Hidden at 10ms: it holds, still moving, until 60ms, then eases.
        assert_eq!(step("hidden", 0.0, 10), (snapshot(100.0), true));
        assert_eq!(step("hidden", 0.0, 40), (snapshot(100.0), true));
        assert_eq!(step("hidden", 0.0, 110), (snapshot(50.0), true));
        assert_eq!(step("hidden", 0.0, 160), (snapshot(0.0), false));
        // Shown again at 200, then hidden while it still waits: it never
        // left 0, and waits again from there.
        assert_eq!(step("shown", 100.0, 200), (snapshot(0.0), true));
        assert_eq!(step("hidden", 0.0, 220), (snapshot(0.0), true));
        assert_eq!(step("shown", 100.0, 230), (snapshot(0.0), true));
        // Shown from 230: waits until 280, halfway at 330, there by 380.
        assert_eq!(step("shown", 100.0, 330), (snapshot(50.0), true));
        assert_eq!(step("shown", 100.0, 400), (snapshot(100.0), false));
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
