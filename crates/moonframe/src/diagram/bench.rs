//! What a frame of `ui.diagram` costs while nodes are dragged: a headless
//! window with 1,000 placed nodes in view, one dragged and then 100,
//! drawn frame after frame. Run in release:
//!
//!   cargo test --release -p moonframe --features capture,diagram diagram::bench -- --ignored --nocapture

use {
    gpui_kit::{AppContext as _, HeadlessAppContext, px, size},
    moonframe_diagram::{
        Diagram, Edge, Node, Rect,
        view::{DiagramView, Mode},
    },
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
};

fn diagram(n: usize, columns: usize) -> Diagram {
    let mut d = Diagram::default();
    for i in 0..n {
        let mut node = Node::new(format!("n{i}"), format!("Box {i}"));
        node.at = Some(Rect::new(
            (i % columns) as f32 * 160.0,
            (i / columns) as f32 * 90.0,
            120.0,
            50.0,
        ));
        d.nodes.push(node);
        if i > 0 && i % columns != 0 {
            d.edges.push(Edge::new(
                format!("e{i}"),
                format!("n{}", i - 1),
                format!("n{i}"),
            ));
        }
    }
    d
}

fn median(mut runs: Vec<Duration>) -> Duration {
    runs.sort();
    runs[runs.len() / 2]
}

#[test]
#[ignore = "a benchmark: run it with --ignored in release"]
fn frames_while_dragging() {
    let mut cx = HeadlessAppContext::with_platform(
        crate::render_entry::window::text_system(),
        Arc::new(crate::assets::Assets::new(None)),
        gpui_platform_capture::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    for (n, columns) in [(100, 10), (500, 25), (1000, 40)] {
        let mut view = None;
        let handle = cx
            .open_window(size(px(1400.0), px(900.0)), |_, cx| {
                let made = cx.new(DiagramView::new);
                view = Some(made.clone());
                made
            })
            .expect("a headless window");
        let view = view.expect("the view");
        let draw = |cx: &mut HeadlessAppContext| {
            let start = Instant::now();
            cx.update_window(handle.into(), |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
            .expect("drawn");
            start.elapsed()
        };
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |v, cx| {
                v.set_mode(Mode::Edit, cx);
                v.set_diagram(diagram(n, columns), window, cx);
            });
        })
        .expect("set");
        cx.run_until_parked();
        for _ in 0..3 {
            draw(&mut cx);
        }
        // Everything in view (the whole grid, fitted, labels too small to
        // draw), then at 60 % (labels drawn, part of the grid in view).
        for fitted in [true, false] {
            cx.update(|cx| {
                view.update(cx, |v, cx| {
                    if fitted {
                        v.fit(cx)
                    } else {
                        v.zoom_to(0.6, cx)
                    }
                })
            });
            draw(&mut cx);
            let idle: Vec<Duration> = (0..30).map(|_| draw(&mut cx)).collect();
            let one: Vec<Duration> = (0..60)
                .map(|k| {
                    cx.update(|cx| {
                        view.update(cx, |v, cx| {
                            v.hold_drag(vec!["n0".into()], k as f32 * 3.0, k as f32, cx)
                        })
                    });
                    draw(&mut cx)
                })
                .collect();
            let hundred: Vec<String> = (0..100.min(n)).map(|i| format!("n{i}")).collect();
            let many: Vec<Duration> = (0..60)
                .map(|k| {
                    cx.update(|cx| {
                        view.update(cx, |v, cx| {
                            v.hold_drag(hundred.clone(), k as f32 * 3.0, k as f32, cx)
                        })
                    });
                    draw(&mut cx)
                })
                .collect();
            let zoom = cx.update(|cx| view.read(cx).camera().zoom);
            println!(
                "{n:>5} nodes (zoom {:>3.0} %): idle {:>8.2?}, dragging one {:>8.2?}, dragging 100 {:>8.2?}",
                zoom * 100.0,
                median(idle),
                median(one),
                median(many)
            );
        }
    }
}
