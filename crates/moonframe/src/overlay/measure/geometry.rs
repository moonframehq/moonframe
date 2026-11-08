//! What the measurements measure: the gaps from an element to whatever is
//! next to it, its padding, the distance between two elements, and which of
//! their edges line up. Plain geometry over the bounds a frame recorded.

use gpui_kit::*;

/// Edges this close count as the same: layout lands on fractions.
const SAME: f32 = 0.5;

/// A measured stretch of empty space: a rectangle, measured along its
/// width when `across` is horizontal, else along its height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Span {
    pub(super) rect: Bounds<Pixels>,
    pub(super) horizontal: bool,
}

impl Span {
    /// What its label says, in whole pixels.
    pub(super) fn length(&self) -> f32 {
        let length = if self.horizontal {
            self.rect.size.width
        } else {
            self.rect.size.height
        };
        f32::from(length).round()
    }
}

/// A full-length line where two elements' edges (or centres) meet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Guide {
    /// A vertical line at x `at`, else a horizontal one at y `at`.
    pub(super) vertical: bool,
    pub(super) at: Pixels,
}

/// How far `subject` is from what is next to it on each side: the nearest
/// element beside it within its container, else the container's own edge.
/// The container is the smallest element around it, else the window.
pub(super) fn gaps(
    subject: Bounds<Pixels>,
    others: &[Bounds<Pixels>],
    window: Bounds<Pixels>,
) -> Vec<Span> {
    let container = others
        .iter()
        .filter(|other| within(subject, **other) && !same(subject, **other))
        .min_by(|a, b| area(**a).total_cmp(&area(**b)))
        .copied()
        .unwrap_or(window);
    let inside: Vec<Bounds<Pixels>> = others
        .iter()
        .filter(|other| within(**other, container) && !same(**other, container))
        .copied()
        .collect();
    let mut spans = Vec::new();
    // Right and left: the nearest edge among those level with it.
    let level = |other: &Bounds<Pixels>| {
        overlap(subject.top(), subject.bottom(), other.top(), other.bottom())
    };
    let beside = |other: &Bounds<Pixels>| {
        overlap(subject.left(), subject.right(), other.left(), other.right())
    };

    let right = inside
        .iter()
        .filter(|other| other.left() >= subject.right() - px(SAME))
        .filter_map(|other| level(other).map(|span| (other.left(), span)))
        .min_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)))
        .unwrap_or((container.right(), (subject.top(), subject.bottom())));
    spans.push(across(subject.right(), right.0, right.1));

    let left = inside
        .iter()
        .filter(|other| other.right() <= subject.left() + px(SAME))
        .filter_map(|other| level(other).map(|span| (other.right(), span)))
        .max_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)))
        .unwrap_or((container.left(), (subject.top(), subject.bottom())));
    spans.push(across(left.0, subject.left(), left.1));

    let below = inside
        .iter()
        .filter(|other| other.top() >= subject.bottom() - px(SAME))
        .filter_map(|other| beside(other).map(|span| (other.top(), span)))
        .min_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)))
        .unwrap_or((container.bottom(), (subject.left(), subject.right())));
    spans.push(down(subject.bottom(), below.0, below.1));

    let above = inside
        .iter()
        .filter(|other| other.bottom() <= subject.top() + px(SAME))
        .filter_map(|other| beside(other).map(|span| (other.bottom(), span)))
        .max_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)))
        .unwrap_or((container.top(), (subject.left(), subject.right())));
    spans.push(down(above.0, subject.top(), above.1));

    spans.retain(|span| span.length() >= 1.);
    spans
}

/// The space between `subject`'s edges and what it holds: the bounds of
/// everything inside it, as top, right, bottom and left spans. Nothing
/// when it holds nothing smaller than itself.
pub(super) fn padding(subject: Bounds<Pixels>, others: &[Bounds<Pixels>]) -> Vec<Span> {
    let content = others
        .iter()
        .filter(|other| within(**other, subject) && !same(**other, subject))
        .copied()
        .reduce(|a, b| a.union(&b));
    let Some(content) = content else {
        return Vec::new();
    };
    let mut spans = vec![
        down(
            subject.top(),
            content.top(),
            (subject.left(), subject.right()),
        ),
        across(
            content.right(),
            subject.right(),
            (content.top(), content.bottom()),
        ),
        down(
            content.bottom(),
            subject.bottom(),
            (subject.left(), subject.right()),
        ),
        across(
            subject.left(),
            content.left(),
            (content.top(), content.bottom()),
        ),
    ];
    spans.retain(|span| span.length() >= 1.);
    spans
}

/// The distances between `a` and `b`: from the one inside to the other's
/// inner edges, else across the space between them, sideways and up or down.
pub(super) fn between(a: Bounds<Pixels>, b: Bounds<Pixels>) -> Vec<Span> {
    if within(b, a) && !same(a, b) {
        return gaps(b, &[a], a);
    }
    if within(a, b) {
        return gaps(a, &[b], b);
    }
    let mut spans = Vec::new();
    // Level with each other: across the middle of where they meet; else
    // from the middle of `a`.
    let level = overlap(a.top(), a.bottom(), b.top(), b.bottom())
        .unwrap_or((a.center().y, a.center().y + px(1.)));
    if b.left() >= a.right() {
        spans.push(across(a.right(), b.left(), level));
    } else if b.right() <= a.left() {
        spans.push(across(b.right(), a.left(), level));
    }
    let beside = overlap(a.left(), a.right(), b.left(), b.right())
        .unwrap_or((a.center().x, a.center().x + px(1.)));
    if b.top() >= a.bottom() {
        spans.push(down(a.bottom(), b.top(), beside));
    } else if b.bottom() <= a.top() {
        spans.push(down(b.bottom(), a.top(), beside));
    }
    spans.retain(|span| span.length() >= 1.);
    spans
}

/// Where an edge or centre of `a` lines up with one of `b`'s.
pub(super) fn aligned(a: Bounds<Pixels>, b: Bounds<Pixels>) -> Vec<Guide> {
    let xs = |r: Bounds<Pixels>| [r.left(), r.center().x, r.right()];
    let ys = |r: Bounds<Pixels>| [r.top(), r.center().y, r.bottom()];
    let mut guides: Vec<Guide> = Vec::new();
    let mut add = |vertical: bool, ours: [Pixels; 3], theirs: [Pixels; 3]| {
        for at in ours {
            let meets = theirs
                .iter()
                .any(|other| (f32::from(*other) - f32::from(at)).abs() <= SAME);
            let known = guides.iter().any(|guide| {
                guide.vertical == vertical && (f32::from(guide.at) - f32::from(at)).abs() <= SAME
            });
            if meets && !known {
                guides.push(Guide { vertical, at });
            }
        }
    };
    add(true, xs(a), xs(b));
    add(false, ys(a), ys(b));
    guides
}

/// Empty space from x `from` to `to`, over the rows `level` covers.
fn across(from: Pixels, to: Pixels, level: (Pixels, Pixels)) -> Span {
    Span {
        rect: Bounds::from_corners(point(from, level.0), point(to.max(from), level.1)),
        horizontal: true,
    }
}

/// Empty space from y `from` to `to`, over the columns `beside` covers.
fn down(from: Pixels, to: Pixels, beside: (Pixels, Pixels)) -> Span {
    Span {
        rect: Bounds::from_corners(point(beside.0, from), point(beside.1, to.max(from))),
        horizontal: false,
    }
}

/// Where two ranges overlap, if they do.
fn overlap(a0: Pixels, a1: Pixels, b0: Pixels, b1: Pixels) -> Option<(Pixels, Pixels)> {
    let (start, end) = (a0.max(b0), a1.min(b1));
    (end - start > px(SAME)).then_some((start, end))
}

/// `inner` lies inside `outer`, edges included.
fn within(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    let slack = px(SAME);
    inner.left() >= outer.left() - slack
        && inner.top() >= outer.top() - slack
        && inner.right() <= outer.right() + slack
        && inner.bottom() <= outer.bottom() + slack
}

/// The same box: a wrapper and what it wraps.
fn same(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    within(a, b) && within(b, a)
}

fn area(bounds: Bounds<Pixels>) -> f32 {
    f32::from(bounds.size.width) * f32::from(bounds.size.height)
}

#[cfg(test)]
mod tests {
    use {
        super::{Guide, Span, aligned, between, gaps, padding},
        gpui_kit::{Bounds, Pixels, point, px, size},
    };

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }

    fn lengths(spans: &[Span]) -> Vec<(bool, f32)> {
        spans
            .iter()
            .map(|span| (span.horizontal, span.length()))
            .collect()
    }

    // The mockup's panel: a header, the picked card, one under it and one
    // beside it, in a panel of its own.
    const PANEL: (f32, f32, f32, f32) = (0., 0., 420., 236.);

    fn panel() -> Vec<Bounds<Pixels>> {
        let (x, y, w, h) = PANEL;
        vec![
            rect(x, y, w, h),
            rect(16., 12., 120., 26.),
            rect(16., 52., 260., 72.),
            rect(16., 140., 260., 72.),
            rect(292., 52., 112., 160.),
        ]
    }

    #[test]
    fn gaps_reach_the_nearest_neighbour_or_the_container() {
        let others = panel();
        let window = rect(0., 0., 1000., 800.);
        let spans = gaps(rect(16., 52., 260., 72.), &others, window);
        // Right to the card beside it, left to the panel, down to the card
        // under it, up to the header.
        assert_eq!(lengths(&spans), vec![
            (true, 16.),
            (true, 16.),
            (false, 16.),
            (false, 14.)
        ]);
    }

    #[test]
    fn a_wrapper_of_the_same_size_is_not_the_container() {
        let mut others = panel();
        others.push(rect(16., 52., 260., 72.));
        let spans = gaps(
            rect(16., 52., 260., 72.),
            &others,
            rect(0., 0., 1000., 800.),
        );
        assert_eq!(spans.len(), 4);
    }

    #[test]
    fn padding_is_the_space_around_what_it_holds() {
        let card = rect(16., 52., 260., 72.);
        let others = vec![card, rect(28., 64., 120., 20.), rect(28., 88., 100., 24.)];
        assert_eq!(lengths(&padding(card, &others)), vec![
            (false, 12.),
            (true, 128.),
            (false, 12.),
            (true, 12.)
        ]);
        assert!(padding(card, &[card]).is_empty());
    }

    #[test]
    fn between_two_measures_the_space_between_them() {
        let a = rect(16., 52., 260., 72.);
        let beside = rect(292., 52., 112., 160.);
        assert_eq!(lengths(&between(a, beside)), vec![(true, 16.)]);
        let under = rect(16., 140., 260., 72.);
        assert_eq!(lengths(&between(a, under)), vec![(false, 16.)]);
        // Diagonal: both ways.
        assert_eq!(lengths(&between(a, rect(300., 150., 20., 20.))).len(), 2);
        // One inside the other: to its inner edges.
        let around = rect(0., 0., 420., 236.);
        assert_eq!(between(a, around).len(), 4);
    }

    #[test]
    fn guides_show_where_edges_line_up() {
        let a = rect(16., 52., 260., 72.);
        let guides = aligned(a, rect(292., 52., 112., 160.));
        assert_eq!(guides, vec![Guide {
            vertical: false,
            at: px(52.)
        }]);
        assert!(aligned(a, rect(300., 300., 7., 7.)).is_empty());
    }
}
