//! Ranks: which layer each node sits in. Cycles are broken first by
//! turning back the edges a depth-first walk finds going back up; then
//! every node sits at least one rank under each node pointing at it.

/// Which edges to turn round so the graph has no cycle: those a
/// depth-first walk from each node in turn finds pointing at a node still
/// on its path. `edges` are (from, to) over `n` nodes; self loops are left
/// alone (they never take part in ranking).
pub(super) fn reversed(n: usize, edges: &[(usize, usize)]) -> Vec<bool> {
    let mut out = vec![false; edges.len()];
    let mut adjacent: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, &(a, b)) in edges.iter().enumerate() {
        if a != b {
            adjacent[a].push(i);
        }
    }
    // 0: not seen, 1: on the path, 2: done.
    let mut mark = vec![0u8; n];
    for root in 0..n {
        if mark[root] != 0 {
            continue;
        }
        // An explicit stack of (node, next edge to look at), so deep
        // graphs don't overflow the thread's stack.
        let mut stack = vec![(root, 0usize)];
        mark[root] = 1;
        while let Some(&mut (node, ref mut next)) = stack.last_mut() {
            if let Some(&edge) = adjacent[node].get(*next) {
                *next += 1;
                let to = edges[edge].1;
                match mark[to] {
                    0 => {
                        mark[to] = 1;
                        stack.push((to, 0));
                    },
                    1 => out[edge] = true,
                    _ => {},
                }
            } else {
                mark[node] = 2;
                stack.pop();
            }
        }
    }
    out
}

/// Each node's rank, from 0, over an acyclic graph: at least one more than
/// every node pointing at it, and at least its `hint` (where it was in the
/// previous layout). A node only pointing at others, with no hint, moves
/// down to sit just over the nearest of them, so its edges stay short.
pub(super) fn ranks(n: usize, edges: &[(usize, usize)], hints: &[Option<usize>]) -> Vec<usize> {
    let mut incoming = vec![0usize; n];
    let mut out_edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(a, b) in edges {
        if a != b {
            incoming[b] += 1;
            out_edges[a].push(b);
            in_edges[b].push(a);
        }
    }
    // Kahn's order: each node after all that point at it.
    let mut order = Vec::with_capacity(n);
    let mut ready: Vec<usize> = (0..n).filter(|&v| incoming[v] == 0).rev().collect();
    while let Some(v) = ready.pop() {
        order.push(v);
        for &w in &out_edges[v] {
            incoming[w] -= 1;
            if incoming[w] == 0 {
                ready.push(w);
            }
        }
    }
    let mut rank = vec![0usize; n];
    for &v in &order {
        let least = in_edges[v].iter().map(|&u| rank[u] + 1).max().unwrap_or(0);
        rank[v] = least.max(hints.get(v).copied().flatten().unwrap_or(0));
    }
    // Sources pulled down toward what they point at.
    for &v in order.iter().rev() {
        if in_edges[v].is_empty()
            && hints.get(v).copied().flatten().is_none()
            && let Some(nearest) = out_edges[v].iter().map(|&w| rank[w]).min()
        {
            rank[v] = nearest.saturating_sub(1).max(rank[v]);
        }
    }
    // Ranks with nobody in them (hints can leave gaps) close up.
    let mut used: Vec<usize> = rank.clone();
    used.sort_unstable();
    used.dedup();
    rank.iter()
        .map(|r| used.binary_search(r).unwrap_or(0))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_cycle_loses_one_edge_and_ranks_follow_the_arrows() {
        let edges = [(0, 1), (1, 2), (2, 0)];
        let back = reversed(3, &edges);
        assert_eq!(back, vec![false, false, true]);
        let acyclic: Vec<_> = edges
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
        assert_eq!(ranks(3, &acyclic, &[None; 3]), vec![0, 1, 2]);
    }

    #[test]
    fn a_source_sits_just_over_its_target_and_hints_hold() {
        // 0 -> 1 -> 2 -> 3, and 4 -> 3: 4 moves down beside 2.
        let edges = [(0, 1), (1, 2), (2, 3), (4, 3)];
        assert_eq!(ranks(5, &edges, &[None; 5]), vec![0, 1, 2, 3, 2]);
        // Where 4 was before (rank 0), it stays, rather than moving down.
        let hints = [None, None, None, None, Some(0)];
        assert_eq!(ranks(5, &edges, &hints), vec![0, 1, 2, 3, 0]);
    }
}
