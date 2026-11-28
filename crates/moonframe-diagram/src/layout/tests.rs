use {
    super::*,
    crate::model::{Diagram, DiagramKind, Direction, Edge, Group, Message, Node, Note},
};

fn flow(nodes: &[&str], edges: &[(&str, &str)]) -> Diagram {
    Diagram {
        nodes: nodes.iter().map(|id| Node::new(*id, *id)).collect(),
        edges: edges
            .iter()
            .enumerate()
            .map(|(i, (a, b))| Edge::new(format!("e{i}"), *a, *b))
            .collect(),
        ..Diagram::default()
    }
}

fn lay(diagram: &Diagram) -> Layout {
    layout(diagram, &Approx, &Options::default(), None)
}

/// Whether two nodes' boxes share any area.
fn overlapping(out: &Layout) -> Option<(String, String)> {
    let all: Vec<_> = out.nodes.iter().collect();
    for (i, (a, ra)) in all.iter().enumerate() {
        for (b, rb) in &all[i + 1..] {
            if ra.overlaps(rb) {
                return Some(((*a).clone(), (*b).clone()));
            }
        }
    }
    None
}

#[test]
fn a_chain_goes_down_in_ranks_and_its_edges_run_straight() {
    let out = lay(&flow(&["a", "b", "c"], &[("a", "b"), ("b", "c")]));
    let (a, b, c) = (out.nodes["a"], out.nodes["b"], out.nodes["c"]);
    assert!(a.bottom() < b.y && b.bottom() < c.y, "{out:?}");
    assert!((a.center().x - c.center().x).abs() < 0.5);
    let e = &out.edges["e0"];
    assert_eq!(e.points.len(), 2, "{e:?}");
    assert_eq!(e.points[0].y, a.bottom());
    assert_eq!(e.points[1].y, b.y);
}

#[test]
fn every_direction_turns_the_ranks() {
    for (direction, check) in [
        (
            Direction::Down,
            (|a: Rect, b: Rect| a.bottom() < b.y) as fn(Rect, Rect) -> bool,
        ),
        (Direction::Up, |a, b| b.bottom() < a.y),
        (Direction::Right, |a, b| a.right() < b.x),
        (Direction::Left, |a, b| b.right() < a.x),
    ] {
        let mut d = flow(&["a", "b"], &[("a", "b")]);
        d.direction = direction;
        let out = lay(&d);
        assert!(
            check(out.nodes["a"], out.nodes["b"]),
            "{direction:?}: {out:?}"
        );
    }
}

#[test]
fn edges_are_orthogonal_and_end_on_their_nodes() {
    // A diamond with a long edge across it and a cycle back.
    let d = flow(&["a", "b", "c", "d"], &[
        ("a", "b"),
        ("a", "c"),
        ("b", "d"),
        ("c", "d"),
        ("a", "d"),
        ("d", "a"),
    ]);
    let out = lay(&d);
    assert_eq!(overlapping(&out), None);
    for edge in &d.edges {
        let route = &out.edges[&edge.id];
        for w in route.points.windows(2) {
            assert!(
                (w[0].x - w[1].x).abs() < 0.01 || (w[0].y - w[1].y).abs() < 0.01,
                "{}: {:?}",
                edge.id,
                route.points
            );
        }
        let (from, to) = (
            out.nodes[&edge.from].inflate(0.5),
            out.nodes[&edge.to].inflate(0.5),
        );
        assert!(
            from.contains(route.points[0]),
            "{} starts off its node: {:?}",
            edge.id,
            route.points
        );
        assert!(
            to.contains(*route.points.last().unwrap()),
            "{} ends off its node: {:?}",
            edge.id,
            route.points
        );
    }
}

#[test]
fn a_bigger_graph_has_no_overlaps() {
    let ids: Vec<String> = (0..60).map(|i| format!("n{i}")).collect();
    let names: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut pairs = Vec::new();
    for i in 1..60 {
        pairs.push((names[(i * 7) % i], names[i]));
        if i % 5 == 0 {
            pairs.push((names[i], names[i / 3]));
        }
    }
    let out = lay(&flow(&names, &pairs));
    assert_eq!(overlapping(&out), None);
    assert_eq!(out.nodes.len(), 60);
}

#[test]
fn adding_a_node_keeps_the_others_in_rank_and_order() {
    let before = flow(&["a", "b", "c", "d"], &[("a", "b"), ("a", "c"), ("c", "d")]);
    let first = lay(&before);
    let mut after = before.clone();
    after.nodes.push(Node::new("e", "e"));
    after.edges.push(Edge::new("new", "b", "e"));
    let second = layout(&after, &Approx, &Options::default(), Some(&first));
    // Same ranks, and the same order among the nodes that were there.
    let old = ["a", "b", "c", "d"];
    for id in old {
        assert_eq!(first.hints[id].0, second.hints[id].0, "{id} changed rank");
    }
    let sequence = |out: &Layout| {
        let mut ids = old.to_vec();
        ids.sort_by(|a, b| {
            let (ra, oa) = out.hints[*a];
            let (rb, ob) = out.hints[*b];
            ra.cmp(&rb).then(oa.total_cmp(&ob))
        });
        ids
    };
    assert_eq!(sequence(&first), sequence(&second));
}

#[test]
fn a_group_surrounds_its_nodes() {
    let mut d = flow(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
    d.groups.push(Group {
        id: "g".into(),
        label: "Group".into(),
        ..Group::default()
    });
    d.nodes[0].group = Some("g".into());
    d.nodes[1].group = Some("g".into());
    let out = lay(&d);
    let g = out.groups["g"];
    for id in ["a", "b"] {
        let r = out.nodes[id];
        assert!(
            g.x <= r.x && g.y <= r.y && g.right() >= r.right() && g.bottom() >= r.bottom(),
            "{id}"
        );
    }
}

#[test]
fn placed_nodes_stay_and_a_new_one_goes_beside_its_neighbour() {
    let mut d = flow(&["a", "b", "c"], &[("a", "b"), ("b", "c")]);
    d.nodes[0].at = Some(Rect::new(0.0, 0.0, 100.0, 40.0));
    d.nodes[1].at = Some(Rect::new(300.0, 0.0, 100.0, 40.0));
    let out = lay(&d);
    assert_eq!(out.nodes["a"], Rect::new(0.0, 0.0, 100.0, 40.0));
    assert!(out.nodes["c"].x > out.nodes["b"].right(), "{out:?}");
    assert_eq!(overlapping(&out), None);
}

#[test]
fn a_sequence_puts_messages_in_rows_between_lanes() {
    let d = Diagram {
        kind: DiagramKind::Sequence,
        nodes: vec![Node::new("alice", "Alice"), Node::new("bob", "Bob")],
        messages: vec![
            Message {
                from: "alice".into(),
                to: "bob".into(),
                label: "Hello Bob, how are you?".into(),
                ..Message::default()
            },
            Message {
                from: "bob".into(),
                to: "alice".into(),
                label: "Fine".into(),
                ..Message::default()
            },
            Message {
                from: "bob".into(),
                to: "bob".into(),
                label: "think".into(),
                ..Message::default()
            },
        ],
        ..Diagram::default()
    };
    let out = lay(&d);
    assert_eq!(out.messages.len(), 3);
    assert!(out.messages[0].points[0].y < out.messages[1].points[0].y);
    let gap = out.nodes["bob"].center().x - out.nodes["alice"].center().x;
    assert!(
        gap > Approx.width("Hello Bob, how are you?", 14.0 * 0.9),
        "lanes too close: {gap}"
    );
    assert_eq!(out.lifelines.len(), 2);
}

fn message(from: &str, to: &str, label: &str) -> Message {
    Message {
        from: from.into(),
        to: to.into(),
        label: label.into(),
        ..Message::default()
    }
}

#[test]
fn notes_sit_before_their_message_in_any_order_given() {
    let note = |text: &str, before| Note {
        over: vec!["a".into()],
        text: text.into(),
        before,
    };
    let d = Diagram {
        kind: DiagramKind::Sequence,
        messages: vec![
            message("a", "b", "one"),
            message("b", "a", "two"),
            message("a", "b", "three"),
        ],
        notes: vec![note("late", 2), note("early", 0)],
        ..Diagram::default()
    };
    let out = lay(&d);
    // Each rect stays at its note's index: `early` above the first
    // message, `late` between the second and the third.
    let (late, early) = (out.notes[0], out.notes[1]);
    assert!(early.bottom() < out.messages[0].points[0].y, "{early:?}");
    assert!(late.y > out.messages[1].points[0].y, "{late:?}");
    assert!(late.bottom() < out.messages[2].points[0].y, "{late:?}");
}

#[test]
fn participants_only_messages_name_get_a_box() {
    let d = Diagram {
        kind: DiagramKind::Sequence,
        nodes: vec![Node::new("a", "Alice")],
        messages: vec![message("a", "b", "hi")],
        ..Diagram::default()
    };
    let ids: Vec<String> = d.participants().into_iter().map(|p| p.id).collect();
    assert_eq!(ids, ["a", "b"]);
    let out = lay(&d);
    assert!(out.nodes.contains_key("b"));
}

#[test]
fn a_flows_bounds_hold_its_edge_labels() {
    let long = "a rather long sentence beside the loop";
    let mut d = flow(&["a", "b"], &[("a", "a"), ("a", "b")]);
    d.edges[0].label = Some(long.into());
    let out = lay(&d);
    let label = out.edges["e0"].label.unwrap();
    let half = (Approx.width(long, 14.0 * 0.86) + 10.0) / 2.0;
    assert!(out.bounds.right() >= label.x + half, "{:?}", out.bounds);
    assert!(out.bounds.x <= label.x - half, "{:?}", out.bounds);
}

#[test]
fn a_sequences_bounds_hold_its_message_labels() {
    let long = "a rather long label for a message to itself";
    let d = Diagram {
        kind: DiagramKind::Sequence,
        nodes: vec![Node::new("a", "A"), Node::new("b", "B")],
        messages: vec![message("a", "b", "go"), message("b", "b", long)],
        ..Diagram::default()
    };
    let out = lay(&d);
    let label = out.messages[1].label.unwrap();
    let half = Approx.width(long, 14.0 * 0.9) / 2.0;
    assert!(out.bounds.right() >= label.x + half, "{:?}", out.bounds);
}
