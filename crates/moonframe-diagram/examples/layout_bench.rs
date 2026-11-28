//! How long layouts take: layered flows of 100, 500 and 1,000 nodes (about
//! 1.4 edges each, some back), and routing a placed diagram of the same
//! sizes. Run in release:
//!
//!   cargo run --release -p moonframe-diagram --example layout-bench

use {
    moonframe_diagram::{
        Diagram, Edge, Node, Rect,
        layout::{Approx, Options, layout},
    },
    std::time::{Duration, Instant},
};

/// A repeatable pseudo-random graph: each node linked from one or two
/// earlier ones, and now and then back to an earlier one.
fn graph(n: usize, placed: bool) -> Diagram {
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = |below: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % below.max(1) as u64) as usize
    };
    let mut d = Diagram::default();
    let columns = (n as f32).sqrt().ceil() as usize;
    for i in 0..n {
        let mut node = Node::new(format!("n{i}"), format!("Step {i}"));
        if placed {
            node.at = Some(Rect::new(
                (i % columns) as f32 * 180.0,
                (i / columns) as f32 * 110.0,
                140.0,
                60.0,
            ));
        }
        d.nodes.push(node);
        if i > 0 {
            d.edges.push(Edge::new(
                format!("e{i}"),
                format!("n{}", next(i)),
                format!("n{i}"),
            ));
            if i % 3 == 0 {
                d.edges.push(Edge::new(
                    format!("f{i}"),
                    format!("n{}", next(i)),
                    format!("n{i}"),
                ));
            }
            if i % 17 == 0 {
                d.edges.push(Edge::new(
                    format!("b{i}"),
                    format!("n{i}"),
                    format!("n{}", next(i)),
                ));
            }
        }
    }
    d
}

fn median(mut runs: Vec<Duration>) -> Duration {
    runs.sort();
    runs[runs.len() / 2]
}

fn main() {
    let options = Options::default();
    for placed in [false, true] {
        for n in [100, 500, 1000] {
            let d = graph(n, placed);
            let runs = (0..7)
                .map(|_| {
                    let start = Instant::now();
                    let out = layout(&d, &Approx, &options, None);
                    std::hint::black_box(out);
                    start.elapsed()
                })
                .collect();
            let what = if placed {
                "placed, routed"
            } else {
                "layered"
            };
            println!(
                "{what:>15}: {n:>5} nodes, {:>5} edges: {:>8.1?}",
                d.edges.len(),
                median(runs)
            );
        }
    }
    // Growing: one node more, laid out from the last layout.
    let d = graph(500, false);
    let first = layout(&d, &Approx, &options, None);
    let mut grown = d.clone();
    grown.nodes.push(Node::new("extra", "Extra"));
    grown.edges.push(Edge::new("extra-edge", "n10", "extra"));
    let start = Instant::now();
    let _ = layout(&grown, &Approx, &options, Some(&first));
    println!(
        "{:>15}: 501 nodes from the last layout: {:>8.1?}",
        "incremental",
        start.elapsed()
    );
}
