//! Each fixture in `tests/fixtures` parsed and compared with its `.snap`
//! (run with `UPDATE_SNAPSHOTS=1` to write them), then laid out: no two
//! nodes overlap and every edge between known nodes has a route.

use {
    super::*,
    crate::{
        layout::{Approx, Options, layout},
        model::{DiagramKind, Head},
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
        let want = std::fs::read_to_string(&snap).unwrap_or_default();
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
        let shape = |d: &Diagram| {
            let mut nodes: Vec<_> = d.nodes.iter().map(|n| (n.kind, n.fill.clone())).collect();
            nodes.sort_by_key(|(k, f)| (k.name(), f.clone()));
            let edges: Vec<_> = d.edges.iter().map(|e| (e.end, e.label.clone())).collect();
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
