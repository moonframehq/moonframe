//! A flow in layers. Ranks run along the diagram's direction; an edge that
//! spans several ranks gets a stand-in node in each rank it crosses, so it
//! is ordered and placed like a node and then routed through them. Works in
//! its own frame (`across` the ranks, `along` them) and turns the result to
//! the diagram's direction at the end.

pub(super) mod route;

use {
    super::{Layout, Measure, Options, node_size, order, place, rank},
    crate::{
        geometry::{Point, Rect, Size},
        model::{Diagram, Direction},
    },
    std::collections::HashMap,
};

/// How wide a long edge's stand-in is across its rank: how far apart
/// parallel long edges run.
const DUMMY_WIDTH: f32 = 12.0;

/// One node of the layered graph: a diagram node, or a long edge's
/// stand-in in one rank.
pub(super) struct Item {
    pub(super) rank: usize,
    /// Its size across and along the ranks.
    pub(super) across: f32,
    pub(super) along: f32,
    pub(super) dummy: bool,
}

/// An edge as it is drawn: the items it passes through, in rank order, and
/// whether that order is the reverse of the edge's own.
pub(super) struct Chain {
    pub(super) edge: usize,
    pub(super) items: Vec<usize>,
    pub(super) reversed: bool,
}

/// The ranks' extent along the direction: where each band starts and how
/// deep it is.
pub(super) struct Bands {
    pub(super) start: Vec<f32>,
    pub(super) depth: Vec<f32>,
    pub(super) total: f32,
}

impl Bands {
    pub(super) fn middle(&self, rank: usize) -> f32 {
        self.start[rank] + self.depth[rank] / 2.0
    }

    /// The middle of the gap after `rank`.
    pub(super) fn channel(&self, rank: usize) -> f32 {
        let end = self.start[rank] + self.depth[rank];
        match self.start.get(rank + 1) {
            Some(next) => (end + next) / 2.0,
            None => end,
        }
    }
}

pub(super) fn layout(diagram: &Diagram, measure: &dyn Measure, options: &Options, previous: Option<&Layout>) -> Layout {
    let vertical = diagram.direction.vertical();
    let index: HashMap<&str, usize> = diagram.nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let real = diagram.nodes.len();
    let sizes: Vec<Size> = diagram.nodes.iter().map(|n| node_size(n, measure, options)).collect();

    // Edges between known nodes; self loops apart.
    let mut pairs = Vec::new();
    let mut loops = Vec::new();
    for (i, edge) in diagram.edges.iter().enumerate() {
        let (Some(&a), Some(&b)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str())) else {
            continue;
        };
        if a == b {
            loops.push((i, a));
        } else {
            pairs.push((i, a, b));
        }
    }
    let plain: Vec<(usize, usize)> = pairs.iter().map(|&(_, a, b)| (a, b)).collect();
    let back = rank::reversed(real, &plain);
    let directed: Vec<(usize, usize)> = plain.iter().zip(&back).map(|(&(a, b), &r)| if r { (b, a) } else { (a, b) }).collect();
    let hints: Vec<Option<(usize, f32)>> = diagram
        .nodes
        .iter()
        .map(|n| previous.and_then(|p| p.hints.get(&n.id).copied()))
        .collect();
    let rank_hints: Vec<Option<usize>> = hints.iter().map(|h| h.map(|(r, _)| r)).collect();
    let ranks = rank::ranks(real, &directed, &rank_hints);

    // Items: the nodes, then each long edge's stand-ins.
    let mut items: Vec<Item> = (0..real)
        .map(|v| {
            let s = sizes[v];
            let (across, along) = if vertical { (s.w, s.h) } else { (s.h, s.w) };
            Item { rank: ranks[v], across, along, dummy: false }
        })
        .collect();
    let mut chains = Vec::new();
    for (&(edge, _, _), (&(a, b), &reversed)) in pairs.iter().zip(directed.iter().zip(&back)) {
        let mut chain = vec![a];
        for r in ranks[a] + 1..ranks[b] {
            items.push(Item { rank: r, across: DUMMY_WIDTH, along: 0.0, dummy: true });
            chain.push(items.len() - 1);
        }
        chain.push(b);
        chains.push(Chain { edge, items: chain, reversed });
    }
    let count = items.len();
    let mut down = vec![Vec::new(); count];
    let mut up = vec![Vec::new(); count];
    for chain in &chains {
        for w in chain.items.windows(2) {
            down[w[0]].push(w[1]);
            up[w[1]].push(w[0]);
        }
    }

    // A first order: the previous layout's where there is one, else the
    // diagram's own order (stand-ins after their rank's nodes).
    let ranks_count = items.iter().map(|i| i.rank + 1).max().unwrap_or(0);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); ranks_count];
    for (v, item) in items.iter().enumerate() {
        layers[item.rank].push(v);
    }
    let hinted = hints.iter().filter(|h| h.is_some()).count();
    let keep = hinted > 0 && hinted * 2 >= real;
    if keep {
        for layer in &mut layers {
            layer.sort_by(|&a, &b| {
                let key = |v: usize| if v < real { hints[v].map_or(f32::MAX, |(_, o)| o) } else { f32::MAX };
                key(a).total_cmp(&key(b)).then(a.cmp(&b))
            });
        }
    }
    let group_ids: HashMap<&str, usize> = diagram.groups.iter().enumerate().map(|(i, g)| (g.id.as_str(), i)).collect();
    let group: Vec<Option<usize>> = (0..count)
        .map(|v| {
            (v < real)
                .then(|| diagram.nodes[v].group.as_deref().and_then(|g| group_ids.get(g).copied()))
                .flatten()
        })
        .collect();
    let layers = order::order(layers, &down, &up, &group, keep);

    // Across: centres; along: bands as deep as their deepest node.
    let widths: Vec<f32> = items.iter().map(|i| i.across).collect();
    let weight = |a: usize, b: usize| match (items[a].dummy, items[b].dummy) {
        (true, true) => 8.0,
        (true, false) | (false, true) => 2.0,
        (false, false) => 1.0,
    };
    let across = place::across(&layers, &widths, options.node_gap, &down, &up, weight);
    let labelled = diagram.edges.iter().any(|e| e.label.as_deref().is_some_and(|l| !l.is_empty()));
    let gap = options.rank_gap + if labelled { options.font_size * 1.4 } else { 0.0 };
    let mut start = Vec::with_capacity(ranks_count);
    let mut depth = Vec::with_capacity(ranks_count);
    let mut at = 0.0;
    for layer in &layers {
        let deep = layer.iter().map(|&v| items[v].along).fold(0.0, f32::max);
        start.push(at);
        depth.push(deep);
        at += deep + gap;
    }
    let bands = Bands { total: (at - gap).max(0.0), start, depth };

    // Turned to the diagram's direction.
    let turn = Turn { direction: diagram.direction, total: bands.total };
    let mut out = Layout::default();
    for (v, node) in diagram.nodes.iter().enumerate() {
        let center = turn.point(across[v], bands.middle(items[v].rank));
        let rect = node.at.unwrap_or_else(|| Rect::centered(center, sizes[v]));
        out.nodes.insert(node.id.clone(), rect);
    }
    for (r, layer) in layers.iter().enumerate() {
        for (i, &v) in layer.iter().enumerate() {
            if v < real {
                out.hints.insert(diagram.nodes[v].id.clone(), (r, i as f32));
            }
        }
    }
    route::edges(diagram, &items, &chains, &loops, &across, &bands, &turn, &mut out);
    out
}

/// From the layered frame (across, along) to the diagram's.
pub(super) struct Turn {
    pub(super) direction: Direction,
    pub(super) total: f32,
}

impl Turn {
    pub(super) fn point(&self, across: f32, along: f32) -> Point {
        match self.direction {
            Direction::Down => Point::new(across, along),
            Direction::Up => Point::new(across, self.total - along),
            Direction::Right => Point::new(along, across),
            Direction::Left => Point::new(self.total - along, across),
        }
    }
}
