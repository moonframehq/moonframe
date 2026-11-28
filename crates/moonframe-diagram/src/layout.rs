//! Where everything goes. [`layout`] picks how by what the diagram is:
//!
//! - a sequence diagram: participants side by side, messages in rows
//!   ([`sequence`]);
//! - a flow with any node placed (an editor's): those places, the others
//!   put beside a node they link to, and edges routed around the nodes
//!   ([`placed`]);
//! - any other flow: layers ([`layered`]): ranks along the direction,
//!   crossings reduced, nodes lined up with their neighbours, edges as
//!   orthogonal elbows through the gaps.
//!
//! A layout given the previous one keeps each node's rank and order where
//! it can, so a diagram that grows (an agent adding nodes) keeps its shape.

mod groups;
mod layered;
mod order;
mod place;
pub mod placed;
mod rank;
mod sequence;
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

use {
    crate::{
        geometry::{Point, Rect, Size, bounds},
        model::{Diagram, DiagramKind, Node, NodeKind},
    },
    std::collections::{BTreeMap, HashMap},
};

/// How wide text is: the view measures with the real font; [`Approx`]
/// guesses, for a layout with no GUI.
pub trait Measure: Send + Sync {
    /// The width of `text` on one line at `size` pixels.
    fn width(&self, text: &str, size: f32) -> f32;
}

/// Text as wide as an average proportional font draws it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Approx;

impl Measure for Approx {
    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().count() as f32 * size * 0.56
    }
}

/// Widths measured beforehand (the view shapes each label once, on its own
/// thread), falling back to [`Approx`] for text it did not measure.
#[derive(Clone, Debug, Default)]
pub struct Widths(pub HashMap<String, f32>);

impl Measure for Widths {
    fn width(&self, text: &str, size: f32) -> f32 {
        self.0
            .get(text)
            .copied()
            .unwrap_or_else(|| Approx.width(text, size))
    }
}

/// Sizes and gaps, in diagram pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub font_size: f32,
    /// Between neighbours in a rank.
    pub node_gap: f32,
    /// Between ranks.
    pub rank_gap: f32,
    /// Around a label inside its node.
    pub padding: Size,
    /// The least size of a node with an outline.
    pub min_node: Size,
    /// Around a group's nodes, and the height of its title.
    pub group_padding: f32,
    pub group_header: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            node_gap: 32.0,
            rank_gap: 56.0,
            padding: Size::new(16.0, 10.0),
            min_node: Size::new(72.0, 40.0),
            group_padding: 16.0,
            group_header: 22.0,
        }
    }
}

/// An edge's way: corners from its start to its end, and where its label
/// sits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Route {
    pub points: Vec<Point>,
    pub label: Option<Point>,
}

impl Route {
    /// Where the route is half its length along: a label's place.
    pub fn middle(points: &[Point]) -> Option<Point> {
        let total: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
        let mut left = total / 2.0;
        for w in points.windows(2) {
            let step = w[0].distance(w[1]);
            if step >= left && step > 0.0 {
                let t = left / step;
                return Some(Point::new(
                    w[0].x + (w[1].x - w[0].x) * t,
                    w[0].y + (w[1].y - w[0].y) * t,
                ));
            }
            left -= step;
        }
        points.first().copied()
    }
}

/// Where every part of a diagram goes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub nodes: BTreeMap<String, Rect>,
    /// A flow's edges, by id.
    pub edges: BTreeMap<String, Route>,
    pub groups: BTreeMap<String, Rect>,
    /// A sequence diagram's messages, in order.
    pub messages: Vec<Route>,
    /// A sequence diagram's notes, in order.
    pub notes: Vec<Rect>,
    /// A sequence diagram's lifelines: participant, top, bottom.
    pub lifelines: Vec<(String, Point, Point)>,
    /// Everything drawn.
    pub bounds: Rect,
    /// Each laid-out node's rank and place in it, for the next layout.
    pub(crate) hints: HashMap<String, (usize, f32)>,
}

/// Lays `diagram` out, keeping what `previous` decided where it can.
pub fn layout(
    diagram: &Diagram,
    measure: &dyn Measure,
    options: &Options,
    previous: Option<&Layout>,
) -> Layout {
    let mut out = match diagram.kind {
        DiagramKind::Sequence => sequence::layout(diagram, measure, options),
        DiagramKind::Flow if diagram.nodes.iter().any(|n| n.at.is_some()) => {
            placed::layout(diagram, measure, options)
        },
        DiagramKind::Flow => layered::layout(diagram, measure, options, previous),
    };
    groups::place(diagram, options, &mut out);
    // Everything drawn: boxes, and every corner of every line.
    let dots: Vec<Rect> = out
        .edges
        .values()
        .chain(out.messages.iter())
        .flat_map(|r| r.points.iter())
        .chain(out.lifelines.iter().flat_map(|(_, a, b)| [a, b]))
        .map(|p| Rect::new(p.x, p.y, 0.0, 0.0))
        .collect();
    out.bounds = bounds(
        out.nodes
            .values()
            .chain(out.groups.values())
            .chain(out.notes.iter())
            .chain(dots.iter()),
    )
    .unwrap_or_default();
    out
}

/// The size `node` takes: its own when placed, else what its label needs.
pub fn node_size(node: &Node, measure: &dyn Measure, options: &Options) -> Size {
    if let Some(at) = node.at {
        return at.size();
    }
    let size = options.font_size;
    let line = size * 1.4;
    let mut text_w = measure.width(&node.label, size);
    let mut text_h = line;
    if let Some(sub) = &node.sublabel {
        text_w = text_w.max(measure.width(sub, size * 0.86));
        text_h += line * 0.86;
    }
    let pad = options.padding;
    let (w, h) = (text_w + pad.w * 2.0, text_h + pad.h * 2.0);
    let min = options.min_node;
    match node.kind {
        NodeKind::Text => Size::new(text_w + 4.0, text_h),
        // A diamond's label sits in its middle half.
        NodeKind::Diamond => Size::new((w * 1.5).max(min.w * 1.3), (h * 1.6).max(min.h * 1.5)),
        NodeKind::Circle => {
            let d = w.max(h).max(min.h) * 1.1;
            Size::new(d, d)
        },
        NodeKind::Cylinder => Size::new(w.max(min.w), (h + 16.0).max(min.h + 16.0)),
        NodeKind::Note => Size::new(w.max(min.w * 1.4), h.max(min.h * 1.4)),
        NodeKind::Pill => Size::new((w + h * 0.5).max(min.w), h.max(min.h)),
        NodeKind::Box | NodeKind::Rounded => Size::new(w.max(min.w), h.max(min.h)),
    }
}
