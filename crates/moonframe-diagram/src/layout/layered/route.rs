//! Edges of a layered flow as orthogonal elbows: out of a node's side
//! facing the next rank (several edges spread along it, in the order of
//! where they go), straight through each stand-in, and across each gap on
//! a track of their own, so parallel bends don't run on top of each other.

use {
    super::{Bands, Chain, Item, Turn},
    crate::{
        geometry::Point,
        layout::{Layout, Route, placed},
        model::{Diagram, NodeKind},
    },
    std::collections::HashMap,
};

/// The most room between two tracks in one gap.
const TRACK_STEP: f32 = 6.0;
/// How far off its line a stand-in may be and still be put on it.
const STRAIGHTEN: f32 = 8.0;
/// The least length of the leg into a node that holds the label.
const LABEL_LEG: f32 = 24.0;
/// How far a self loop stands out from its node.
const LOOP_OUT: f32 = 20.0;

#[allow(clippy::too_many_arguments)]
pub(super) fn edges(
    diagram: &Diagram,
    items: &[Item],
    chains: &[Chain],
    loops: &[(usize, usize)],
    across: &[f32],
    bands: &Bands,
    turn: &Turn,
    out: &mut Layout,
) {
    let real = diagram.nodes.len();
    // Where each chain leaves its first item and reaches its last.
    let mut leave = vec![0.0f32; chains.len()];
    let mut reach = vec![0.0f32; chains.len()];
    let mut outgoing: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut incoming: HashMap<usize, Vec<usize>> = HashMap::new();
    for (c, chain) in chains.iter().enumerate() {
        outgoing.entry(chain.items[0]).or_default().push(c);
        incoming
            .entry(*chain.items.last().unwrap_or(&0))
            .or_default()
            .push(c);
    }
    let spread =
        |v: usize, list: &mut Vec<usize>, next: &dyn Fn(usize) -> usize, ports: &mut [f32]| {
            list.sort_by(|&a, &b| across[next(a)].total_cmp(&across[next(b)]));
            let centred =
                v >= real || matches!(diagram.nodes[v].kind, NodeKind::Diamond | NodeKind::Circle);
            let width = items[v].across;
            for (k, &c) in list.iter().enumerate() {
                ports[c] = if centred {
                    across[v]
                } else {
                    across[v] - width / 2.0 + width * (k + 1) as f32 / (list.len() + 1) as f32
                };
            }
        };
    for (&v, list) in &mut outgoing {
        spread(v, list, &|c| chains[c].items[1], &mut leave);
    }
    for (&v, list) in &mut incoming {
        spread(
            v,
            list,
            &|c| chains[c].items[chains[c].items.len() - 2],
            &mut reach,
        );
    }

    // Each chain's corners in the layered frame, with its bends' gaps
    // noted so tracks can be shared out.
    struct Bend {
        chain: usize,
        corner: usize,
        rank: usize,
        from: f32,
    }
    let mut paths: Vec<Vec<(f32, f32)>> = Vec::with_capacity(chains.len());
    let mut bends: Vec<Bend> = Vec::new();
    for (c, chain) in chains.iter().enumerate() {
        let first = chain.items[0];
        let last = *chain.items.last().unwrap_or(&first);
        let mut stops = vec![(
            leave[c],
            bands.middle(items[first].rank) + items[first].along / 2.0,
        )];
        for &d in &chain.items[1..chain.items.len() - 1] {
            stops.push((across[d], bands.middle(items[d].rank)));
        }
        stops.push((
            reach[c],
            bands.middle(items[last].rank) - items[last].along / 2.0,
        ));
        // A stand-in a few pixels off the line before it is put on it, so
        // a long edge runs straight instead of stepping at each gap.
        for k in 1..stops.len().saturating_sub(1) {
            if (stops[k].0 - stops[k - 1].0).abs() < STRAIGHTEN {
                stops[k].0 = stops[k - 1].0;
            }
        }
        let mut path = vec![stops[0]];
        for (k, w) in stops.windows(2).enumerate() {
            let (p, q) = (w[0], w[1]);
            if (p.0 - q.0).abs() >= 0.5 {
                let rank = items[chain.items[k]].rank;
                let mid = bands.channel(rank);
                bends.push(Bend {
                    chain: c,
                    corner: path.len(),
                    rank,
                    from: p.0,
                });
                path.push((p.0, mid));
                path.push((q.0, mid));
            }
            path.push(q);
        }
        paths.push(path);
    }
    // Tracks: the bends in one gap, spread around its middle.
    let mut by_gap: HashMap<usize, Vec<usize>> = HashMap::new();
    for (b, bend) in bends.iter().enumerate() {
        by_gap.entry(bend.rank).or_default().push(b);
    }
    for (rank, mut list) in by_gap {
        if list.len() < 2 {
            continue;
        }
        list.sort_by(|&a, &b| bends[a].from.total_cmp(&bends[b].from));
        let room = (bands.start.get(rank + 1).copied().unwrap_or(bands.total)
            - bands.start[rank]
            - bands.depth[rank])
            * 0.6;
        let step = TRACK_STEP.min(room / list.len() as f32);
        let middle = (list.len() - 1) as f32 / 2.0;
        for (i, &b) in list.iter().enumerate() {
            let shift = (i as f32 - middle) * step;
            let bend = &bends[b];
            paths[bend.chain][bend.corner].1 += shift;
            paths[bend.chain][bend.corner + 1].1 += shift;
        }
    }

    for (c, chain) in chains.iter().enumerate() {
        let edge = &diagram.edges[chain.edge];
        let mut points: Vec<Point> = paths[c].iter().map(|&(a, b)| turn.point(a, b)).collect();
        if chain.reversed {
            points.reverse();
        }
        let placed_end = [&edge.from, &edge.to]
            .iter()
            .any(|id| diagram.node(id).is_some_and(|n| n.at.is_some()));
        let route = if placed_end {
            // A node with a place of its own isn't where the layers put
            // it: route straight to where it is.
            let (a, b) = (out.nodes[&edge.from], out.nodes[&edge.to]);
            placed::route_between(&a, &b, &[])
        } else {
            let points = simplify(points);
            Route {
                label: label_place(&points, chain.reversed),
                points,
            }
        };
        out.edges.insert(edge.id.clone(), route);
    }
    for &(e, v) in loops {
        let edge = &diagram.edges[e];
        let r = out.nodes[&diagram.nodes[v].id];
        let (x, cy) = (r.right(), r.center().y);
        let points = vec![
            Point::new(x, cy - 8.0),
            Point::new(x + LOOP_OUT, cy - 8.0),
            Point::new(x + LOOP_OUT, cy + 8.0),
            Point::new(x, cy + 8.0),
        ];
        out.edges.insert(edge.id.clone(), Route {
            label: Some(Point::new(x + LOOP_OUT + 4.0, cy)),
            points,
        });
    }
}

/// Where an edge's label goes: on the leg into its target (the leg out of
/// its source, for an edge drawn against the ranks), which is in that
/// node's own column, so labels of edges bending in one gap don't meet;
/// the middle of the route when that leg is too short.
fn label_place(points: &[Point], reversed: bool) -> Option<Point> {
    let leg = if reversed {
        points.get(..2)
    } else {
        points.get(points.len().saturating_sub(2)..)
    };
    match leg {
        Some([a, b]) if a.distance(*b) >= LABEL_LEG => {
            Some(Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0))
        },
        _ => Route::middle(points),
    }
}

/// `points` without repeats or corners that don't turn.
pub(crate) fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if out.last().is_some_and(|q| q.distance(p) < 0.01) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let straight = ((a.x - b.x).abs() < 0.01 && (b.x - p.x).abs() < 0.01)
                || ((a.y - b.y).abs() < 0.01 && (b.y - p.y).abs() < 0.01);
            if straight {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}
