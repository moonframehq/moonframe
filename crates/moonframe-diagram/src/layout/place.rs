//! Where each node sits across its rank: as close as its order and the
//! gaps allow to the middle of its neighbours, so edges run straight. Each
//! pass solves one rank exactly: the places nearest (in squared distance)
//! to the wanted ones that keep the order and the gaps, by pooling
//! neighbours that would overlap (isotonic regression).

/// Passes down and up the ranks.
const PASSES: usize = 8;

/// Each node's centre across its rank. `layers` are the ranks in order,
/// `widths[v]` the room v takes across, `gap` the least space between two
/// neighbours, `down`/`up` the neighbours in the next and previous rank,
/// and `weight(a, b)` how much an edge pulls (long edges' pieces pull
/// harder, so they run straight).
pub(super) fn across(
    layers: &[Vec<usize>],
    widths: &[f32],
    gap: f32,
    down: &[Vec<usize>],
    up: &[Vec<usize>],
    weight: impl Fn(usize, usize) -> f32,
) -> Vec<f32> {
    let n = widths.len();
    let mut x = vec![0.0f32; n];
    // Packed from the left, each rank centred on the widest.
    let spans: Vec<f32> = layers.iter().map(|l| span(l, widths, gap)).collect();
    let widest = spans.iter().copied().fold(0.0, f32::max);
    for (layer, span) in layers.iter().zip(&spans) {
        let mut at = (widest - span) / 2.0;
        for &v in layer {
            x[v] = at + widths[v] / 2.0;
            at += widths[v] + gap;
        }
    }
    for pass in 0..PASSES {
        let downward = pass % 2 == 0;
        let ranks: Vec<usize> = if downward { (1..layers.len()).collect() } else { (0..layers.len().saturating_sub(1)).rev().collect() };
        for r in ranks {
            let layer = &layers[r];
            let wanted: Vec<f32> = layer
                .iter()
                .map(|&v| {
                    let around = if downward { &up[v] } else { &down[v] };
                    pull(v, around, &x, &weight).unwrap_or(x[v])
                })
                .collect();
            let placed = nearest_in_order(layer, &wanted, widths, gap);
            for (&v, p) in layer.iter().zip(placed) {
                x[v] = p;
            }
        }
    }
    // A last pass with both ranks' neighbours, so nodes sit between them.
    for layer in layers {
        let wanted: Vec<f32> = layer
            .iter()
            .map(|&v| {
                let mut around = up[v].clone();
                around.extend(&down[v]);
                pull(v, &around, &x, &weight).unwrap_or(x[v])
            })
            .collect();
        let placed = nearest_in_order(layer, &wanted, widths, gap);
        for (&v, p) in layer.iter().zip(placed) {
            x[v] = p;
        }
    }
    let least = layers
        .iter()
        .flat_map(|l| l.iter().map(|&v| x[v] - widths[v] / 2.0))
        .fold(f32::INFINITY, f32::min);
    if least.is_finite() {
        for v in &mut x {
            *v -= least;
        }
    }
    x
}

fn span(layer: &[usize], widths: &[f32], gap: f32) -> f32 {
    layer.iter().map(|&v| widths[v]).sum::<f32>() + gap * layer.len().saturating_sub(1) as f32
}

/// The weighted middle of `around`'s places, as `v` would like to sit.
fn pull(v: usize, around: &[usize], x: &[f32], weight: &impl Fn(usize, usize) -> f32) -> Option<f32> {
    if around.is_empty() {
        return None;
    }
    let (mut sum, mut total) = (0.0, 0.0);
    for &u in around {
        let w = weight(v, u);
        sum += x[u] * w;
        total += w;
    }
    (total > 0.0).then(|| sum / total)
}

/// The places closest to `wanted` that keep `layer`'s order with `gap`
/// between neighbours. With each place shifted by the room before it, the
/// order becomes "never decreasing", which pooling adjacent violators
/// solves exactly.
fn nearest_in_order(layer: &[usize], wanted: &[f32], widths: &[f32], gap: f32) -> Vec<f32> {
    let mut offset = Vec::with_capacity(layer.len());
    let mut at = 0.0;
    for (i, &v) in layer.iter().enumerate() {
        if i > 0 {
            at += widths[layer[i - 1]] / 2.0 + gap + widths[v] / 2.0;
        }
        offset.push(at);
    }
    // Blocks of (mean, count), merged while one would sit before the last.
    let mut blocks: Vec<(f32, usize)> = Vec::new();
    for (w, o) in wanted.iter().zip(&offset) {
        blocks.push((w - o, 1));
        while blocks.len() > 1 {
            let (b_mean, b_count) = blocks[blocks.len() - 1];
            let (a_mean, a_count) = blocks[blocks.len() - 2];
            if a_mean <= b_mean {
                break;
            }
            let count = a_count + b_count;
            let mean = (a_mean * a_count as f32 + b_mean * b_count as f32) / count as f32;
            blocks.truncate(blocks.len() - 2);
            blocks.push((mean, count));
        }
    }
    let mut out = Vec::with_capacity(layer.len());
    for (mean, count) in blocks {
        for _ in 0..count {
            let i = out.len();
            out.push(mean + offset[i]);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn neighbours_pushed_apart_keep_their_order() {
        // Both want 0; each is 10 wide with a gap of 4: they share the
        // difference.
        let placed = nearest_in_order(&[0, 1], &[0.0, 0.0], &[10.0, 10.0], 4.0);
        assert_eq!(placed, vec![-7.0, 7.0]);
        // Far enough apart already: untouched.
        assert_eq!(nearest_in_order(&[0, 1], &[0.0, 50.0], &[10.0, 10.0], 4.0), vec![0.0, 50.0]);
    }

    #[test]
    fn a_chain_runs_straight() {
        // 0 -> 1 -> 2, one per rank, and a wide neighbour 3 beside 1.
        let layers = vec![vec![0], vec![3, 1], vec![2]];
        let widths = [40.0, 40.0, 40.0, 100.0];
        let down = vec![vec![1], vec![2], vec![], vec![]];
        let up = vec![vec![], vec![0], vec![1], vec![]];
        let x = across(&layers, &widths, 10.0, &down, &up, |_, _| 1.0);
        assert!((x[0] - x[1]).abs() < 0.5 && (x[1] - x[2]).abs() < 0.5, "{x:?}");
        assert!(x[1] - x[3] >= 70.0 - 0.01, "{x:?}");
    }
}
