//! Each fixture in `tests/fixtures` parsed and compared with its `.snap`
//! (run with `UPDATE_SNAPSHOTS=1` to write them), then laid out: no two
//! nodes overlap and every edge between known nodes has a route.

use {
    super::*,
    crate::{
        layout::{Approx, Options, layout},
        model::{DiagramKind, Direction, EdgeKind, Head, NodeKind, Tone},
        write,
    },
    std::{fmt::Write as _, path::PathBuf},
};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn head(h: Head) -> &'static str {
    match h {
        Head::None => "none",
        Head::Arrow => "arrow",
        Head::Open => "open",
        Head::Circle => "circle",
        Head::Cross => "cross",
        Head::Triangle => "triangle",
        Head::Diamond => "diamond",
        Head::HollowDiamond => "hollow-diamond",
    }
}

/// The model as plain lines, for snapshots a reviewer can read.
fn dump(d: &Diagram) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{:?} {:?}", d.kind, d.direction);
    for g in &d.groups {
        let _ = writeln!(
            out,
            "group {} {:?} in {}",
            g.id,
            g.label,
            g.parent.as_deref().unwrap_or("-")
        );
    }
    for n in &d.nodes {
        let _ = write!(out, "node {} {:?} {}", n.id, n.label, n.kind.name());
        for (key, value) in [("sub", &n.sublabel), ("group", &n.group), ("fill", &n.fill)] {
            if let Some(value) = value {
                let _ = write!(out, " {key}={value:?}");
            }
        }
        if let Some(tone) = n.tone {
            let _ = write!(out, " tone={tone:?}");
        }
        out.push('\n');
    }
    for e in &d.edges {
        let _ = writeln!(
            out,
            "edge {} {} -> {} {:?} {}..{}{}",
            e.id,
            e.from,
            e.to,
            e.kind,
            head(e.start),
            head(e.end),
            e.label
                .as_ref()
                .map(|l| format!(" {l:?}"))
                .unwrap_or_default()
        );
    }
    for m in &d.messages {
        let _ = writeln!(
            out,
            "message {} -> {} {:?} {:?}",
            m.from, m.to, m.kind, m.label
        );
    }
    for n in &d.notes {
        let _ = writeln!(
            out,
            "note over {} before {} {:?}",
            n.over.join(","),
            n.before,
            n.text
        );
    }
    out
}

#[test]
fn fixtures_parse_as_their_snapshots_and_lay_out() {
    let mut checked = 0;
    let mut entries: Vec<_> = std::fs::read_dir(fixtures())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        let language = match path.extension().and_then(|e| e.to_str()) {
            Some("mmd") => "mermaid",
            Some("dot") => "dot",
            _ => continue,
        };
        let text = std::fs::read_to_string(&path).unwrap();
        let parsed = fenced(language, &text).unwrap();
        let got = match &parsed {
            Ok(d) => dump(d),
            Err(e) => format!("unsupported: {e}\n"),
        };
        let snap = path.with_extension("snap");
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            std::fs::write(&snap, &got).unwrap();
        }
        // A Windows checkout may turn the snapshot's line ends into CRLF.
        let want = std::fs::read_to_string(&snap)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert_eq!(got, want, "{} differs from its snapshot", path.display());
        if let Ok(d) = parsed {
            let out = layout(&d, &Approx, &Options::default(), None);
            let rects: Vec<_> = out.nodes.iter().collect();
            for (i, (a, ra)) in rects.iter().enumerate() {
                for (b, rb) in &rects[i + 1..] {
                    assert!(!ra.overlaps(rb), "{}: {a} overlaps {b}", path.display());
                }
            }
            if d.kind == DiagramKind::Flow {
                for e in &d.edges {
                    assert!(
                        out.edges.contains_key(&e.id),
                        "{}: {} has no route",
                        path.display(),
                        e.id
                    );
                }
            }
        }
        checked += 1;
    }
    assert!(checked >= 5, "only {checked} fixtures");
}

#[test]
fn what_is_written_reads_back_the_same() {
    for name in [
        "flowchart-review.mmd",
        "flowchart-shapes.mmd",
        "sequence-login.mmd",
        "digraph-deps.dot",
        "class-shapes.mmd",
    ] {
        let path = fixtures().join(name);
        let language = if name.ends_with(".dot") {
            "dot"
        } else {
            "mermaid"
        };
        let first = fenced(language, &std::fs::read_to_string(&path).unwrap())
            .unwrap()
            .unwrap();
        let written = write::mermaid(&first);
        let again = mermaid::parse(&written).unwrap_or_else(|e| panic!("{name}: {e}\n{written}"));
        // Places and sublabels aside, the same nodes, edges and messages.
        // A flowchart has no UML heads: it draws them as arrows.
        let flowchart = written.starts_with("flowchart");
        let drawn = |h: Head| match h {
            Head::Triangle | Head::Diamond | Head::HollowDiamond | Head::Open if flowchart => {
                Head::Arrow
            },
            h => h,
        };
        let shape = |d: &Diagram| {
            let mut nodes: Vec<_> = d
                .nodes
                .iter()
                .map(|n| (n.kind, n.fill.clone(), n.tone.map(|t| format!("{t:?}"))))
                .collect();
            nodes.sort_by_key(|(k, f, t)| (k.name(), f.clone(), t.clone()));
            let edges: Vec<_> = d
                .edges
                .iter()
                .map(|e| (drawn(e.start), drawn(e.end), e.label.clone()))
                .collect();
            (nodes, edges, d.messages.len(), d.groups.len())
        };
        assert_eq!(shape(&first), shape(&again), "{name}:\n{written}");
    }
}

#[test]
fn labels_lose_their_markup() {
    assert_eq!(
        label("\"Address comments<br/>and push\""),
        "Address comments and push"
    );
    assert_eq!(label("\"`**Bold** start`\""), "Bold start");
    assert_eq!(
        label("say #quot;hi#quot; to <b>Ann</b>"),
        "say \"hi\" to Ann"
    );
    assert_eq!(label("a < b"), "a < b");
}

#[test]
fn an_edge_needs_an_arrow_it_knows() {
    let err = mermaid::parse("flowchart TD\n  A ->> B").unwrap_err();
    assert_eq!(err.line, 2);
}

#[test]
fn hostile_text_is_refused_not_obeyed() {
    // An HTML label never closed, at the very end.
    assert!(dot::parse("digraph { a [label=<").is_err());
    // Subgraphs nested past the limit.
    let deep = format!(
        "digraph {{ {} a {} }}",
        "subgraph { ".repeat(200),
        "} ".repeat(200)
    );
    assert!(dot::parse(&deep).is_err());
    let deep = format!(
        "flowchart TD\n{}A-->B\n{}",
        "subgraph s\n".repeat(200),
        "end\n".repeat(200)
    );
    assert!(mermaid::parse(&deep).is_err());
    // Edges multiplied past the limit.
    let many: Vec<String> = (0..120).map(|i| format!("n{i}")).collect();
    let group = format!("{{{}}}", many.join(" "));
    assert!(dot::parse(&format!("digraph {{ {group} -> {group} }}")).is_ok());
    let wide = format!("digraph {{ {group} -> {group} -> {group} }}");
    assert_eq!(
        dot::parse(&wide)
            .map(|d| d.edges.len())
            .map_err(|e| e.reason.contains("edges")),
        Err(true)
    );
    let side = many.join(" & ");
    assert!(mermaid::parse(&format!("flowchart TD\n{side} --> {side} --> {side}")).is_err());
}

#[test]
fn nodes_alone_count_toward_the_limit() {
    let names: Vec<String> = (0..MOST_NODES + 1).map(|i| format!("n{i}")).collect();
    let text = format!("flowchart TD\n{}", names.join(" & "));
    assert!(mermaid::parse(&text).is_err());
    let styles: String = names
        .iter()
        .map(|n| format!("style {n} fill:#fff\n"))
        .collect();
    assert!(mermaid::parse(&format!("flowchart TD\n{styles}")).is_err());
    let messages: String = (0..MOST_EDGES + 1).map(|_| "A->>B: hi\n").collect();
    assert!(mermaid::parse(&format!("sequenceDiagram\n{messages}")).is_err());
    let dot_nodes: String = names.iter().map(|n| format!("{n};")).collect();
    assert!(dot::parse(&format!("digraph {{ {dot_nodes} }}")).is_err());
}

#[test]
fn a_subgraph_keeps_what_its_nodes_were_given() {
    let d = dot::parse(
        "digraph { node [shape=box]; subgraph cluster_x { a [shape=diamond, label=\"A\"]; } }",
    )
    .unwrap();
    assert_eq!(d.nodes[0].kind, NodeKind::Diamond);
    assert_eq!(d.nodes[0].label, "A");
    assert_eq!(d.nodes[0].group.as_deref(), Some("cluster_x"));
    // Defaults still reach a node first named in the subgraph.
    let d = dot::parse("digraph { node [shape=box]; subgraph { b } }").unwrap();
    assert_eq!(d.nodes[0].kind, NodeKind::Box);
}

#[test]
fn trapezoids_find_their_own_closers() {
    let d = mermaid::parse(
        "flowchart TD\n  A[/Trapezoid\\] --> B[\\Inverse/]\n  C[/Lean/] --> D[\\Back\\]",
    )
    .unwrap();
    let labels: Vec<_> = d.nodes.iter().map(|n| n.label.as_str()).collect();
    assert_eq!(labels, ["Trapezoid", "Inverse", "Lean", "Back"]);
    // The nearest closer wins when two could match on one line.
    let d = mermaid::parse("flowchart TD\n  A[/x\\] --> B[/y/]").unwrap();
    assert_eq!(d.nodes[0].label, "x");
    assert_eq!(d.nodes[1].label, "y");
}

#[test]
fn class_relations_are_found_outside_cardinalities() {
    let d = mermaid::parse("classDiagram\n  A \"0..1\" --> \"1\" B : owns").unwrap();
    assert_eq!(d.edges.len(), 1);
    let e = &d.edges[0];
    assert_eq!((e.from.as_str(), e.to.as_str()), ("A", "B"));
    assert_eq!(
        (e.start, e.end, e.kind),
        (Head::None, Head::Arrow, EdgeKind::Solid)
    );
    assert_eq!(e.label.as_deref(), Some("owns"));
    // Marks at both ends, and a name ending in `o` is not a diamond.
    let d = mermaid::parse("classDiagram\n  A <|--|> B\n  Foo--Bar\n  C o..> D").unwrap();
    let heads: Vec<_> = d.edges.iter().map(|e| (e.start, e.end, e.kind)).collect();
    assert_eq!(heads, [
        (Head::Triangle, Head::Triangle, EdgeKind::Solid),
        (Head::None, Head::None, EdgeKind::Solid),
        (Head::HollowDiamond, Head::Open, EdgeKind::Dashed),
    ]);
    assert_eq!(d.edges[1].from, "Foo");
}

#[test]
fn a_body_never_closed_is_an_error_on_its_line() {
    let err = mermaid::parse("classDiagram\n  class A {\n    +x int\n  A --> B").unwrap_err();
    assert_eq!(err.line, 2);
    let err =
        mermaid::parse("erDiagram\n  A ||--o{ B : has\n  B {\n    string id\n  A }o--|| C : x")
            .unwrap_err();
    assert_eq!(err.line, 3);
}

#[test]
fn written_ids_never_merge_two_nodes() {
    let d =
        mermaid::parse("flowchart TD\n  api-gw --> api_gw\n  e[end] --> end_\n  1 --> n1").unwrap();
    let written = write::mermaid(&d);
    let again = mermaid::parse(&written).unwrap_or_else(|e| panic!("{e}\n{written}"));
    assert_eq!(again.nodes.len(), d.nodes.len(), "{written}");
    assert!(again.edges.iter().all(|e| e.from != e.to), "{written}");
}

#[test]
fn repeated_group_names_are_written_once_each() {
    let d = mermaid::parse("stateDiagram-v2\nstate A {\nstate A {\nB --> C\n}\n}").unwrap();
    let written = write::mermaid(&d);
    assert_eq!(
        written.matches("subgraph").count(),
        d.groups.len(),
        "{written}"
    );
    // A group that is its own parent is still written, with its nodes.
    let mut d = mermaid::parse("flowchart TD\n  subgraph g\n    a\n  end").unwrap();
    d.groups[0].parent = Some("g".into());
    let written = write::mermaid(&d);
    assert!(
        written.contains("subgraph g") && written.contains("a[\"a\"]"),
        "{written}"
    );
}

#[test]
fn tones_are_written_as_classes() {
    let d =
        mermaid::parse("flowchart TD\n  a --> b\n  classDef danger stroke:#f00\n  class b danger")
            .unwrap();
    assert_eq!(d.nodes[1].tone, Some(Tone::Danger));
    let again = mermaid::parse(&write::mermaid(&d)).unwrap();
    assert_eq!(again.nodes[1].tone, Some(Tone::Danger));
    assert_eq!(again.nodes[0].tone, None);
}

#[test]
fn class_heads_and_names_survive_writing() {
    let text = "classDiagram\n  class Canvas[\"The canvas\"]\n  Canvas o-- Theme\n  Shape <|-- Circle : is a";
    let d = mermaid::parse(text).unwrap();
    let written = write::mermaid(&d);
    assert!(written.starts_with("classDiagram"), "{written}");
    let again = mermaid::parse(&written).unwrap();
    let heads = |d: &Diagram| -> Vec<_> {
        d.edges
            .iter()
            .map(|e| {
                (
                    e.from.clone(),
                    e.to.clone(),
                    e.start,
                    e.end,
                    e.label.clone(),
                )
            })
            .collect()
    };
    assert_eq!(heads(&again), heads(&d), "{written}");
    assert_eq!(again.nodes[0].label, "The canvas");
}

#[test]
fn a_later_dot_statement_keeps_earlier_choices() {
    let d =
        dot::parse("digraph { node [shape=box]; a [shape=diamond]; a [label=\"A\"]; }").unwrap();
    assert_eq!(
        (d.nodes[0].kind, d.nodes[0].label.as_str()),
        (NodeKind::Diamond, "A")
    );
    // Fill needs `style=filled` and a color, given in either order.
    let d = dot::parse("digraph { a [style=filled]; a [fillcolor=\"#fff\"]; }").unwrap();
    assert_eq!(d.nodes[0].fill.as_deref(), Some("#fff"));
}

#[test]
fn text_after_a_dot_graph_is_an_error() {
    let err = dot::parse("digraph {\n a -> b\n}\nc -> d").unwrap_err();
    assert_eq!(err.line, 4);
}

#[test]
fn subgraphs_open_and_close_in_pairs() {
    let err = mermaid::parse("flowchart TD\n  subgraph g\n  A --> B").unwrap_err();
    assert_eq!(err.line, 2);
    let err = mermaid::parse("flowchart TD\n  A --> B\n  end").unwrap_err();
    assert_eq!(err.line, 3);
}

#[test]
fn class_and_er_diagrams_take_their_direction() {
    let d = mermaid::parse("classDiagram\n  direction LR\n  A --> B").unwrap();
    assert_eq!(d.direction, Direction::Right);
    let d = mermaid::parse("erDiagram\n  direction RL\n  A ||--o{ B : has").unwrap();
    assert_eq!(d.direction, Direction::Left);
}

#[test]
fn states_named_start_or_end_stay_states() {
    let d = mermaid::parse("stateDiagram-v2\n[*] --> start\nstart --> end\nend --> [*]").unwrap();
    let ids: Vec<_> = d.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids.len(), 4, "{ids:?}");
    assert!(d.edges.iter().all(|e| e.from != e.to), "{:?}", d.edges);
    assert!(
        d.nodes.iter().all(|n| !n.id.starts_with('\u{0}')),
        "{ids:?}"
    );
}

#[test]
fn state_braces_open_and_close_in_pairs() {
    let err = mermaid::parse("stateDiagram-v2\nstate A {\nB --> C").unwrap_err();
    assert_eq!(err.line, 2);
    let err = mermaid::parse("stateDiagram-v2\nB --> C\n}").unwrap_err();
    assert_eq!(err.line, 3);
}

#[test]
fn a_dot_subgraph_names_each_node_once() {
    let d = dot::parse("digraph { { a -> b; b -> c; } -> d; }").unwrap();
    let to_d = d.edges.iter().filter(|e| e.to == "d").count();
    assert_eq!(to_d, 3);
}

#[test]
fn labels_read_back_as_written() {
    for text in [
        "List<T>",
        "a #quot; b",
        "#35;lt;",
        "**bold**",
        "a\\nb",
        "`tick`",
        "x > y & \"z\"",
    ] {
        let mut d = Diagram::default();
        let mut n = crate::model::Node::new("a", text);
        n.kind = NodeKind::Box;
        d.nodes.push(n);
        let written = write::mermaid(&d);
        let again = mermaid::parse(&written).unwrap_or_else(|e| panic!("{e}\n{written}"));
        assert_eq!(again.nodes[0].label, text, "{written}");
    }
    // An entity read from text stays itself when written again.
    let d = mermaid::parse("flowchart TD\nA[\"List#lt;T#gt;\"]").unwrap();
    assert_eq!(d.nodes[0].label, "List<T>");
    let again = mermaid::parse(&write::mermaid(&d)).unwrap();
    assert_eq!(again.nodes[0].label, "List<T>");
}

#[test]
fn sequence_blocks_open_and_close_in_pairs() {
    let fine =
        "sequenceDiagram\nloop every minute\nalt ok\nA->>B: hi\nelse not\nB->>A: no\nend\nend";
    assert_eq!(mermaid::parse(fine).unwrap().messages.len(), 2);
    let err = mermaid::parse("sequenceDiagram\nA->>B: hi\nloop again\nA->>B: hi").unwrap_err();
    assert_eq!(err.line, 3);
    let err = mermaid::parse("sequenceDiagram\nA->>B: hi\nend").unwrap_err();
    assert_eq!(err.line, 3);
    let err = mermaid::parse("sequenceDiagram\nelse\nA->>B: hi").unwrap_err();
    assert_eq!(err.line, 2);
    let err = mermaid::parse("erDiagram\n  A ||--o{ B : has\n  }").unwrap_err();
    assert_eq!(err.line, 3);
}

#[test]
fn a_long_label_of_hashes_or_tags_is_read_quickly() {
    let text = format!("\"{};\"", "#".repeat(200_000));
    let started = std::time::Instant::now();
    assert_eq!(label(&text).len(), 200_001);
    let text = "<a".repeat(100_000);
    assert_eq!(label(&text), text);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}
