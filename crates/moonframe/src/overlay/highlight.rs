//! The inspector's highlight: rounded outlines around the hovered and the
//! selected element. The hover outline glides from element to element, and
//! a selection pulses once as it lands. Motion is skipped when the system
//! asks for reduced motion.

use {
    crate::inspect::Placed,
    gpui_kit::*,
    std::time::{Duration, Instant},
};

/// How long the hover outline takes to reach a newly hovered element.
const GLIDE: Duration = Duration::from_millis(140);
/// How long a selection's ring takes to spread and fade.
const PULSE: Duration = Duration::from_millis(650);
/// How far an outline sits outside its element, to leave its edges visible.
const OUTSET: f32 = 2.;
/// How far a selection's ring spreads.
const SPREAD: f32 = 10.;

/// Where the outlines were last drawn, to move them on from there.
#[derive(Default)]
pub(crate) struct Highlight {
    hover: Option<Glide>,
    selected: Option<Pulse>,
}

/// The hover outline, on its way from one element to another.
struct Glide {
    path: String,
    from: Shape,
    to: Shape,
    since: Instant,
    /// Nothing was hovered before: it fades in where it is.
    fresh: bool,
}

/// The selection, with when it was picked.
struct Pulse {
    path: String,
    since: Instant,
}

/// Where an element is drawn and how round its corners are.
#[derive(Clone, Copy)]
struct Shape {
    bounds: Bounds<Pixels>,
    corners: Corners<Pixels>,
}

/// What a frame paints: the outlines' targets and their colour.
pub(super) struct Targets<'a> {
    pub(super) hovered: Option<&'a str>,
    pub(super) selected: Option<&'a str>,
    pub(super) accent: Hsla,
    /// No motion: everything lands at once.
    pub(super) still: bool,
}

impl Highlight {
    /// Paints the outlines over `recorded`, the frame's placed nodes, and
    /// asks for another frame while any of them is still moving.
    pub(super) fn paint(&mut self, recorded: &[Placed], targets: &Targets, window: &mut Window) {
        let now = Instant::now();
        let progress = |since: Instant, length: Duration| {
            if targets.still {
                1.
            } else {
                (now.saturating_duration_since(since).as_secs_f32() / length.as_secs_f32()).min(1.)
            }
        };
        let mut moving = false;

        let selected = targets
            .selected
            .and_then(|path| shape_of(recorded, path).map(|shape| (path, shape)));
        self.selected = selected.map(|(path, _)| match self.selected.take() {
            Some(pulse) if pulse.path == path => pulse,
            _ => Pulse {
                path: path.to_owned(),
                since: now,
            },
        });
        if let (Some((_, shape)), Some(pulse)) = (selected, &self.selected) {
            let t = progress(pulse.since, PULSE);
            moving |= t < 1.;
            paint_selected(shape, ease_out(t), targets.accent, window);
        }

        let hovered = targets
            .hovered
            .and_then(|path| shape_of(recorded, path).map(|shape| (path, shape)));
        self.hover = match (self.hover.take(), hovered) {
            (_, None) => None,
            (Some(glide), Some((path, shape))) if glide.path == path => {
                Some(Glide { to: shape, ..glide })
            },
            (Some(glide), Some((path, shape))) => {
                let from = glide.shown(ease_out(progress(glide.since, GLIDE)));
                Some(Glide {
                    path: path.to_owned(),
                    from,
                    to: shape,
                    since: now,
                    fresh: false,
                })
            },
            (None, Some((path, shape))) => Some(Glide {
                path: path.to_owned(),
                from: shape,
                to: shape,
                since: now,
                fresh: true,
            }),
        };
        if let Some(glide) = &self.hover {
            let t = progress(glide.since, GLIDE);
            moving |= t < 1.;
            let eased = ease_out(t);
            let alpha = if glide.fresh {
                eased
            } else {
                1.
            };
            paint_hovered(glide.shown(eased), alpha, targets.accent, window);
        }

        if moving {
            window.request_animation_frame();
        }
    }
}

impl Glide {
    fn shown(&self, eased: f32) -> Shape {
        let mix = |from: Pixels, to: Pixels| from + (to - from) * eased;
        let (from, to) = (self.from.bounds, self.to.bounds);
        // Corners as drawn, so a pill's huge radius blends from its height.
        let (a, b) = (self.from.drawn(), self.to.drawn());
        Shape {
            bounds: Bounds {
                origin: point(
                    mix(from.origin.x, to.origin.x),
                    mix(from.origin.y, to.origin.y),
                ),
                size: size(
                    mix(from.size.width, to.size.width),
                    mix(from.size.height, to.size.height),
                ),
            },
            corners: Corners {
                top_left: mix(a.top_left, b.top_left),
                top_right: mix(a.top_right, b.top_right),
                bottom_right: mix(a.bottom_right, b.bottom_right),
                bottom_left: mix(a.bottom_left, b.bottom_left),
            },
        }
    }
}

impl Shape {
    /// The corners as they show: none rounder than half the element.
    fn drawn(&self) -> Corners<Pixels> {
        let most = self.bounds.size.width.min(self.bounds.size.height) / 2.;
        let clamp = |radius: Pixels| radius.min(most).max(px(0.));
        Corners {
            top_left: clamp(self.corners.top_left),
            top_right: clamp(self.corners.top_right),
            bottom_right: clamp(self.corners.bottom_right),
            bottom_left: clamp(self.corners.bottom_left),
        }
    }

    /// The same shape `by` further out, its corners following the
    /// element's: a square corner stays square.
    fn around(&self, by: Pixels) -> Shape {
        let grow = |radius: Pixels| {
            if radius > px(0.) {
                radius + by
            } else {
                radius
            }
        };
        let corners = self.drawn();
        Shape {
            bounds: self.bounds.dilate(by),
            corners: Corners {
                top_left: grow(corners.top_left),
                top_right: grow(corners.top_right),
                bottom_right: grow(corners.bottom_right),
                bottom_left: grow(corners.bottom_left),
            },
        }
    }
}

fn paint_hovered(shape: Shape, alpha: f32, accent: Hsla, window: &mut Window) {
    let outline = shape.around(px(OUTSET));
    window.paint_quad(quad(
        outline.bounds,
        outline.corners,
        accent.opacity(0.12 * alpha),
        px(1.5),
        accent.opacity(alpha),
        BorderStyle::Solid,
    ));
}

/// The selection, with its ring `eased` of the way through spreading.
fn paint_selected(shape: Shape, eased: f32, accent: Hsla, window: &mut Window) {
    let outline = shape.around(px(OUTSET));
    // A soft halo that stays, so the selection reads at a glance.
    let halo = shape.around(px(OUTSET + 3.));
    window.paint_quad(quad(
        halo.bounds,
        halo.corners,
        transparent_black(),
        px(3.),
        accent.opacity(0.18),
        BorderStyle::Solid,
    ));
    window.paint_quad(quad(
        outline.bounds,
        outline.corners,
        accent.opacity(0.08 + 0.12 * (1. - eased)),
        px(2.),
        accent,
        BorderStyle::Solid,
    ));
    if eased < 1. {
        let ring = shape.around(px(OUTSET + SPREAD * eased));
        window.paint_quad(quad(
            ring.bounds,
            ring.corners,
            transparent_black(),
            px(2.),
            accent.opacity(0.6 * (1. - eased)),
            BorderStyle::Solid,
        ));
    }
}

fn shape_of(recorded: &[Placed], path: &str) -> Option<Shape> {
    recorded
        .iter()
        .find(|placed| placed.path == path)
        // Fully clipped rows can remain recorded in a list's overscan.
        // Do not expand their empty bounds into a visible outline.
        .filter(|placed| !placed.visible.is_empty())
        .map(|placed| Shape {
            bounds: placed.visible,
            corners: placed.corners,
        })
}

fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}
