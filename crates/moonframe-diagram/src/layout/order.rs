//! The order of nodes within each rank: sweeps down and up the ranks, each
//! node moved to the average place of its neighbours in the rank before,
//! keeping the order with the fewest crossings seen. Members of one group
//! stay side by side. An order from the previous layout is kept unless a
//! sweep finds one with fewer crossings.

/// Sweeps tried before settling.
const SWEEPS: usize = 12;
/// Ranks with up to this many nodes also try swapping neighbours.
const TRANSPOSE_UP_TO: usize = 48;

/// The ranks' orders. `layers` lists each rank's nodes in their starting
/// order; `down[v]` and `up[v]` are v's neighbours in the next and the
/// previous rank; `group[v]` keeps v beside its group's other members.
pub(super) fn order(
    mut layers: Vec<Vec<usize>>,
    down: &[Vec<usize>],
    up: &[Vec<usize>],
    group: &[Option<usize>],
    keep_start: bool,
) -> Vec<Vec<usize>> {
    let n = down.len();
    let mut pos = vec![0usize; n];
    let place = |layers: &[Vec<usize>], pos: &mut [usize]| {
        for layer in layers {
            for (i, &v) in layer.iter().enumerate() {
                pos[v] = i;
            }
        }
    };
    if !keep_start {
        // A first order rank by rank, from the one before.
        place(&layers, &mut pos);
        for layer in layers.iter_mut().skip(1) {
            sort_by_neighbours(layer, up, &pos, group);
            for (i, &v) in layer.iter().enumerate() {
                pos[v] = i;
            }
        }
    }
    place(&layers, &mut pos);
    let mut best = layers.clone();
    let mut best_crossings = crossings(&layers, down, &pos);
    for sweep in 0..SWEEPS {
        if best_crossings == 0 {
            break;
        }
        if sweep % 2 == 0 {
            for layer in layers.iter_mut().skip(1) {
                sort_by_neighbours(layer, up, &pos, group);
                for (i, &v) in layer.iter().enumerate() {
                    pos[v] = i;
                }
            }
        } else {
            let last = layers.len().saturating_sub(1);
            for layer in layers[..last].iter_mut().rev() {
                sort_by_neighbours(layer, down, &pos, group);
                for (i, &v) in layer.iter().enumerate() {
                    pos[v] = i;
                }
            }
        }
        transpose(&mut layers, down, up, &mut pos, group);
        let now = crossings(&layers, down, &pos);
        if now < best_crossings {
            best_crossings = now;
            best = layers.clone();
        }
    }
    best
}

/// `layer` sorted by where its nodes' neighbours sit (`pos`); a node with
/// none keeps its place. Then each group's members are drawn together, at
/// the place of the group's first.
fn sort_by_neighbours(layer: &mut [usize], neighbours: &[Vec<usize>], pos: &[usize], group: &[Option<usize>]) {
    let mut keyed: Vec<(f32, usize, usize)> = layer
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let around = &neighbours[v];
            let key = if around.is_empty() {
                i as f32
            } else {
                around.iter().map(|&u| pos[u] as f32).sum::<f32>() / around.len() as f32
            };
            (key, i, v)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let sorted: Vec<usize> = keyed.into_iter().map(|(_, _, v)| v).collect();
    let clustered = cluster(&sorted, group);
    layer.copy_from_slice(&clustered);
}

/// `layer` with each group's members moved to just after the group's first.
fn cluster(layer: &[usize], group: &[Option<usize>]) -> Vec<usize> {
    let mut first: Vec<(usize, usize)> = Vec::new();
    for (i, &v) in layer.iter().enumerate() {
        if let Some(g) = group[v]
            && !first.iter().any(|&(h, _)| h == g)
        {
            first.push((g, i));
        }
    }
    let mut keyed: Vec<(usize, usize, usize)> = layer
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let anchor = group[v].and_then(|g| first.iter().find(|&&(h, _)| h == g)).map_or(i, |&(_, at)| at);
            (anchor, i, v)
        })
        .collect();
    keyed.sort_unstable();
    keyed.into_iter().map(|(_, _, v)| v).collect()
}

/// Swaps of neighbours that lower the crossings, in small ranks.
fn transpose(
    layers: &mut [Vec<usize>],
    down: &[Vec<usize>],
    up: &[Vec<usize>],
    pos: &mut [usize],
    group: &[Option<usize>],
) {
    for _ in 0..4 {
        let mut better = false;
        for layer in layers.iter_mut() {
            if layer.len() > TRANSPOSE_UP_TO {
                continue;
            }
            for i in 0..layer.len().saturating_sub(1) {
                let (a, b) = (layer[i], layer[i + 1]);
                if group[a] != group[b] {
                    continue;
                }
                let now = pair_crossings(a, b, down, pos) + pair_crossings(a, b, up, pos);
                let swapped = pair_crossings(b, a, down, pos) + pair_crossings(b, a, up, pos);
                if swapped < now {
                    layer.swap(i, i + 1);
                    pos[a] = i + 1;
                    pos[b] = i;
                    better = true;
                }
            }
        }
        if !better {
            break;
        }
    }
}

/// Crossings between the edges of `a` and those of `b` (a left of b) into
/// one neighbouring rank.
fn pair_crossings(a: usize, b: usize, neighbours: &[Vec<usize>], pos: &[usize]) -> usize {
    let mut count = 0;
    for &x in &neighbours[a] {
        for &y in &neighbours[b] {
            if pos[y] < pos[x] {
                count += 1;
            }
        }
    }
    count
}

/// Every crossing between neighbouring ranks: for each pair of ranks, the
/// inversions among the edges' ends once sorted by their starts, counted
/// with a Fenwick tree.
pub(super) fn crossings(layers: &[Vec<usize>], down: &[Vec<usize>], pos: &[usize]) -> usize {
    let mut total = 0;
    for r in 0..layers.len().saturating_sub(1) {
        let mut ends: Vec<(usize, usize)> = Vec::new();
        for &v in &layers[r] {
            for &w in &down[v] {
                ends.push((pos[v], pos[w]));
            }
        }
        ends.sort_unstable();
        let size = layers[r + 1].len() + 1;
        let mut tree = vec![0usize; size + 1];
        for (seen, &(_, end)) in ends.iter().enumerate() {
            // Ends already seen that lie to the right of this one cross it.
            let mut at = end + 1;
            let mut not_greater = 0;
            while at > 0 {
                not_greater += tree[at];
                at -= at.isolate_lowest_one();
            }
            total += seen - not_greater;
            let mut at = end + 1;
            while at <= size {
                tree[at] += 1;
                at += at.isolate_lowest_one();
            }
        }
    }
    total
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn neighbours(n: usize, edges: &[(usize, usize)]) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
        let mut down = vec![Vec::new(); n];
        let mut up = vec![Vec::new(); n];
        for &(a, b) in edges {
            down[a].push(b);
            up[b].push(a);
        }
        (down, up)
    }

    #[test]
    fn a_crossing_is_undone() {
        // 0 -> 3 and 1 -> 2 cross with ranks [0, 1], [2, 3].
        let (down, up) = neighbours(4, &[(0, 3), (1, 2)]);
        let layers = vec![vec![0, 1], vec![2, 3]];
        let pos = [0, 1, 0, 1];
        assert_eq!(crossings(&layers, &down, &pos), 1);
        let done = order(layers, &down, &up, &[None; 4], true);
        let mut pos = [0; 4];
        for layer in &done {
            for (i, &v) in layer.iter().enumerate() {
                pos[v] = i;
            }
        }
        assert_eq!(crossings(&done, &down, &pos), 0);
    }

    #[test]
    fn a_groups_members_stay_together() {
        assert_eq!(cluster(&[0, 1, 2, 3], &[Some(0), None, Some(0), None]), vec![0, 2, 1, 3]);
    }
}
