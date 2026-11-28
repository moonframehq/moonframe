//! A sequence diagram in lanes: participants side by side along the top,
//! each with its lifeline down, and the messages in rows, top to bottom,
//! each an arrow from one lifeline to another (or a loop back to its own).
//! Lanes are spread so every label fits between them.

use {
    super::{Layout, Measure, Options, Route},
    crate::{
        geometry::{Point, Rect},
        model::{Diagram, Node},
    },
    std::collections::HashMap,
};

/// Height of a message's row.
const ROW: f32 = 44.0;
/// How far a message to itself stands out from its lifeline.
const LOOP_OUT: f32 = 36.0;

pub(super) fn layout(diagram: &Diagram, measure: &dyn Measure, options: &Options) -> Layout {
    let mut out = Layout::default();
    // Participants: those listed, then any a message names that wasn't.
    let mut people: Vec<Node> = diagram.nodes.clone();
    for m in &diagram.messages {
        for id in [&m.from, &m.to] {
            if !people.iter().any(|p| &p.id == id) {
                people.push(Node::new(id.clone(), id.clone()));
            }
        }
    }
    let index: HashMap<&str, usize> = people
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.as_str(), i))
        .collect();
    let size = options.font_size;
    let head_h = size * 1.4 + options.padding.h * 2.0;
    let widths: Vec<f32> = people
        .iter()
        .map(|p| (measure.width(&p.label, size) + options.padding.w * 2.0).max(options.min_node.w))
        .collect();
    // The least distance between each pair of neighbouring lanes: their
    // boxes, and every label of a message crossing that gap.
    let lanes = people.len();
    let mut need = vec![0.0f32; lanes.saturating_sub(1)];
    for (i, w) in need.iter_mut().enumerate() {
        *w = (widths[i] + widths[i + 1]) / 2.0 + options.node_gap;
    }
    for m in &diagram.messages {
        let (Some(&a), Some(&b)) = (index.get(m.from.as_str()), index.get(m.to.as_str())) else {
            continue;
        };
        let label = measure.width(&m.label, size * 0.9) + 24.0;
        let (lo, hi) = (a.min(b), a.max(b));
        if lo == hi {
            if let Some(gap) = need.get_mut(lo) {
                *gap = gap.max(LOOP_OUT + label);
            }
        } else {
            // Share the label's room across the gaps it spans.
            let each = label / (hi - lo) as f32;
            for gap in &mut need[lo..hi] {
                *gap = gap.max(each);
            }
        }
    }
    let mut x = Vec::with_capacity(lanes);
    let mut at = widths.first().copied().unwrap_or(0.0) / 2.0;
    for i in 0..lanes {
        x.push(at);
        at += need.get(i).copied().unwrap_or(0.0);
    }
    for (i, p) in people.iter().enumerate() {
        out.nodes.insert(
            p.id.clone(),
            Rect::new(x[i] - widths[i] / 2.0, 0.0, widths[i], head_h),
        );
    }

    // Rows: notes before the message they precede, then the message.
    let mut y = head_h + ROW * 0.75;
    let mut notes = diagram.notes.iter().peekable();
    for (i, m) in diagram.messages.iter().enumerate() {
        while let Some(note) = notes.next_if(|n| n.before <= i) {
            y = place_note(note, &index, &x, measure, options, y, &mut out);
        }
        let (Some(&a), Some(&b)) = (index.get(m.from.as_str()), index.get(m.to.as_str())) else {
            continue;
        };
        if a == b {
            let points = vec![
                Point::new(x[a], y),
                Point::new(x[a] + LOOP_OUT, y),
                Point::new(x[a] + LOOP_OUT, y + ROW * 0.5),
                Point::new(x[a], y + ROW * 0.5),
            ];
            // Its label beside the loop, not over it.
            let half = measure.width(&m.label, size * 0.9) / 2.0;
            out.messages.push(Route {
                label: Some(Point::new(x[a] + LOOP_OUT + 8.0 + half, y + ROW * 0.25)),
                points,
            });
            y += ROW * 1.5;
        } else {
            let points = vec![Point::new(x[a], y), Point::new(x[b], y)];
            out.messages.push(Route {
                label: Some(Point::new((x[a] + x[b]) / 2.0, y - size * 0.8)),
                points,
            });
            y += ROW;
        }
    }
    for note in notes {
        y = place_note(note, &index, &x, measure, options, y, &mut out);
    }
    for (i, p) in people.iter().enumerate() {
        out.lifelines
            .push((p.id.clone(), Point::new(x[i], head_h), Point::new(x[i], y)));
    }
    out
}

/// A note's box over its participants at `y`; returns where the next row
/// starts.
fn place_note(
    note: &crate::model::Note,
    index: &HashMap<&str, usize>,
    x: &[f32],
    measure: &dyn Measure,
    options: &Options,
    y: f32,
    out: &mut Layout,
) -> f32 {
    let lanes: Vec<usize> = note
        .over
        .iter()
        .filter_map(|id| index.get(id.as_str()).copied())
        .collect();
    let (lo, hi) = match (lanes.iter().min(), lanes.iter().max()) {
        (Some(&lo), Some(&hi)) => (x[lo], x[hi]),
        _ => (
            x.first().copied().unwrap_or(0.0),
            x.first().copied().unwrap_or(0.0),
        ),
    };
    let text = measure.width(&note.text, options.font_size * 0.9) + options.padding.w * 2.0;
    let w = text.max(hi - lo + 40.0);
    let h = options.font_size * 1.4 + options.padding.h * 2.0;
    out.notes
        .push(Rect::new((lo + hi) / 2.0 - w / 2.0, y - h / 2.0, w, h));
    y + h + ROW * 0.4
}
