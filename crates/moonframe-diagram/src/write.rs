//! The model back to Mermaid, for "Copy as Mermaid" and for sending an
//! edited diagram to an agent: a flow as a `flowchart` (shapes, edge
//! kinds and heads, labels, subgraphs, fills as `style`), a sequence as a
//! `sequenceDiagram`. Read back with [`crate::parse::mermaid`], it gives
//! the same diagram, but for places and a sublabel joined to its label.

use crate::model::{Diagram, DiagramKind, Direction, EdgeKind, Head, MessageKind, Node, NodeKind};

pub fn mermaid(d: &Diagram) -> String {
    match d.kind {
        DiagramKind::Flow => flowchart(d),
        DiagramKind::Sequence => sequence(d),
    }
}

/// An id Mermaid reads: letters, digits and `_`, else made so.
fn id(text: &str) -> String {
    let clean: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    match clean.chars().next() {
        None => "_".to_owned(),
        Some(c) if c.is_numeric() => format!("n{clean}"),
        // `end` closes a subgraph: never an id.
        _ if clean == "end" => "end_".to_owned(),
        _ => clean,
    }
}

/// A label, quoted, with what would end the quotes spelled as entities.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "#quot;"))
}

fn node(n: &Node) -> String {
    let (open, close) = match n.kind {
        NodeKind::Box | NodeKind::Text => ("[", "]"),
        NodeKind::Rounded => ("(", ")"),
        NodeKind::Pill => ("([", "])"),
        NodeKind::Diamond => ("{", "}"),
        NodeKind::Circle => ("((", "))"),
        NodeKind::Cylinder => ("[(", ")]"),
        NodeKind::Note => (">", "]"),
    };
    let mut text = n.label.clone();
    if let Some(sub) = &n.sublabel {
        text = format!("{text}<br/>{sub}");
    }
    format!("{}{open}{}{close}", id(&n.id), quoted(&text))
}

fn flowchart(d: &Diagram) -> String {
    let direction = match d.direction {
        Direction::Down => "TD",
        Direction::Up => "BT",
        Direction::Right => "LR",
        Direction::Left => "RL",
    };
    let mut out = format!("flowchart {direction}\n");
    // Groups, innermost inside their parents, each with its nodes.
    fn group(d: &Diagram, parent: Option<&str>, depth: usize, out: &mut String) {
        let pad = "    ".repeat(depth);
        for n in d.nodes.iter().filter(|n| n.group.as_deref() == parent) {
            out.push_str(&format!("{pad}{}\n", node(n)));
        }
        for g in d.groups.iter().filter(|g| g.parent.as_deref() == parent) {
            out.push_str(&format!(
                "{pad}subgraph {}[{}]\n",
                id(&g.id),
                quoted(&g.label)
            ));
            group(d, Some(&g.id), depth + 1, out);
            out.push_str(&format!("{pad}end\n"));
        }
    }
    // Nodes whose group is unknown go at the top level.
    let known = |g: &Option<String>| {
        g.as_ref()
            .is_none_or(|g| d.groups.iter().any(|h| &h.id == g))
    };
    let mut top = d.clone();
    for n in &mut top.nodes {
        if !known(&n.group) {
            n.group = None;
        }
    }
    group(&top, None, 1, &mut out);
    for e in &d.edges {
        let line = match e.kind {
            EdgeKind::Solid => "--",
            EdgeKind::Dashed | EdgeKind::Dotted => "-.-",
            EdgeKind::Thick => "==",
            EdgeKind::Invisible => "~~~",
        };
        let arrow = if e.kind == EdgeKind::Invisible {
            line.to_owned()
        } else {
            let start = match e.start {
                Head::None => "",
                Head::Circle => "o",
                Head::Cross => "x",
                _ => "<",
            };
            let end = match e.end {
                Head::None if e.kind == EdgeKind::Thick => "=",
                Head::None if e.kind == EdgeKind::Solid => "-",
                Head::None => "",
                Head::Circle => "o",
                Head::Cross => "x",
                _ => ">",
            };
            format!("{start}{line}{end}")
        };
        let label = e
            .label
            .as_deref()
            .filter(|l| !l.is_empty())
            .map(|l| format!("|{}|", quoted(l)))
            .unwrap_or_default();
        out.push_str(&format!(
            "    {} {arrow}{label} {}\n",
            id(&e.from),
            id(&e.to)
        ));
    }
    for n in d.nodes.iter().filter(|n| n.fill.is_some()) {
        out.push_str(&format!(
            "    style {} fill:{}\n",
            id(&n.id),
            n.fill.as_deref().unwrap_or_default()
        ));
    }
    out
}

fn sequence(d: &Diagram) -> String {
    let mut out = "sequenceDiagram\n".to_owned();
    for n in &d.nodes {
        let word = if n.kind == NodeKind::Pill {
            "actor"
        } else {
            "participant"
        };
        if n.label == n.id {
            out.push_str(&format!("    {word} {}\n", id(&n.id)));
        } else {
            out.push_str(&format!("    {word} {} as {}\n", id(&n.id), n.label));
        }
    }
    let mut notes = d.notes.iter().peekable();
    for (i, m) in d.messages.iter().enumerate() {
        while let Some(note) = notes.next_if(|n| n.before <= i) {
            let over: Vec<String> = note.over.iter().map(|o| id(o)).collect();
            out.push_str(&format!(
                "    Note over {}: {}\n",
                over.join(","),
                note.text
            ));
        }
        let arrow = match m.kind {
            MessageKind::Sync => "->>",
            MessageKind::Async => "-)",
            MessageKind::Reply => "-->>",
        };
        out.push_str(&format!(
            "    {}{arrow}{}: {}\n",
            id(&m.from),
            id(&m.to),
            m.label
        ));
    }
    for note in notes {
        let over: Vec<String> = note.over.iter().map(|o| id(o)).collect();
        out.push_str(&format!(
            "    Note over {}: {}\n",
            over.join(","),
            note.text
        ));
    }
    out
}
