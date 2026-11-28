//! What laying a diagram out costs, found without laying it out: how many
//! items the layout places. A layered flow places every node and a
//! stand-in for each rank a long edge crosses, so a few edges between far
//! ranks cost more than many short ones. Ranking is linear, so a caller
//! laying out where a frame waits (a markdown fence) can ask this first,
//! with the same earlier layout it will lay out from: that layout's ranks
//! hold, and can stretch edges a fresh ranking would keep short.

use {
    super::{Layout, rank},
    crate::model::{Diagram, DiagramKind},
    std::collections::HashMap,
};

/// The items laying `diagram` out from `previous` places: nodes, groups,
/// lines, notes, and a layered flow's stand-ins for its long edges.
pub fn cost(diagram: &Diagram, previous: Option<&Layout>) -> usize {
    let plain = diagram.nodes.len()
        + diagram.groups.len()
        + diagram.edges.len()
        + diagram.messages.len()
        + diagram.notes.len();
    let layered = diagram.kind == DiagramKind::Flow && diagram.nodes.iter().all(|n| n.at.is_none());
    if layered {
        plain.saturating_add(stand_ins(diagram, previous))
    } else {
        plain
    }
}

/// The stand-ins a layered flow puts in for its edges' ranks crossed, as
/// [`super::layered`] ranks them, keeping `previous`'s ranks.
fn stand_ins(diagram: &Diagram, previous: Option<&Layout>) -> usize {
    let n = diagram.nodes.len();
    let index: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let plain: Vec<(usize, usize)> = diagram
        .edges
        .iter()
        .filter_map(|e| Some((*index.get(e.from.as_str())?, *index.get(e.to.as_str())?)))
        .filter(|(a, b)| a != b)
        .collect();
    let back = rank::reversed(n, &plain);
    let directed: Vec<(usize, usize)> = plain
        .iter()
        .zip(&back)
        .map(|(&(a, b), &r)| {
            if r {
                (b, a)
            } else {
                (a, b)
            }
        })
        .collect();
    let hints: Vec<Option<usize>> = diagram
        .nodes
        .iter()
        .map(|n| previous.and_then(|p| p.hints.get(&n.id)).map(|&(r, _)| r))
        .collect();
    let ranks = rank::ranks(n, &directed, &hints);
    directed
        .iter()
        .map(|&(a, b)| ranks[b].abs_diff(ranks[a]).saturating_sub(1))
        .fold(0usize, usize::saturating_add)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {
        super::cost,
        crate::{
            layout::{Approx, Options, layout},
            model::{Diagram, Edge, Node},
        },
    };

    fn chain(nodes: usize) -> Diagram {
        let mut diagram = Diagram::default();
        for i in 0..nodes {
            diagram.nodes.push(Node::new(format!("n{i}"), "x"));
        }
        for i in 1..nodes {
            diagram.edges.push(Edge::new(
                format!("e{i}"),
                format!("n{}", i - 1),
                format!("n{i}"),
            ));
        }
        diagram
    }

    #[test]
    fn short_edges_cost_their_items_and_long_ones_their_ranks() {
        let short = chain(10);
        assert_eq!(cost(&short, None), 10 + 9);
        // The same chain with edges from its first node to its last, each
        // crossing the eight ranks between.
        let mut long = chain(10);
        for i in 0..5 {
            long.edges.push(Edge::new(format!("long{i}"), "n0", "n9"));
        }
        assert_eq!(cost(&long, None), 10 + 14 + 5 * 8);
    }

    #[test]
    fn an_earlier_layout_s_ranks_count_as_they_will_be_kept() {
        // `x --> t` first: `x` takes rank 0. A chain then grows into `t`;
        // laid out fresh `x` would move down beside `t`, but the earlier
        // layout keeps it at the top, so its edge crosses the whole chain.
        let mut first = Diagram::default();
        first.nodes.push(Node::new("x", "x"));
        first.nodes.push(Node::new("t", "t"));
        first.edges.push(Edge::new("xt", "x", "t"));
        let before = layout(&first, &Approx, &Options::default(), None);
        let mut grown = chain(10);
        grown.nodes.push(Node::new("x", "x"));
        grown.nodes.push(Node::new("t", "t"));
        grown.edges.push(Edge::new("tail", "n9", "t"));
        grown.edges.push(Edge::new("xt", "x", "t"));
        let fresh = cost(&grown, None);
        let kept = cost(&grown, Some(&before));
        assert_eq!(fresh, 12 + 11);
        assert!(kept >= fresh + 9, "{kept} after {fresh}");
    }
}
