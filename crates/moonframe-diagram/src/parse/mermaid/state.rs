//! `stateDiagram`: states, transitions (`A --> B : label`), `[*]` as a
//! start and an end dot, `state "Long name" as A`, `A : description`,
//! composite states (`state A { ... }`) as groups, and `<<choice>>`
//! states as diamonds. Notes and concurrency (`--`) are left out.

use {
    super::Line,
    crate::{
        model::{Diagram, Edge, Group, Head, Node, NodeKind},
        parse::{Unsupported, label},
    },
};

pub(super) fn parse(lines: &[Line<'_>]) -> Result<Diagram, Unsupported> {
    let mut d = Diagram::default();
    let mut open: Vec<String> = Vec::new();
    let mut in_note = false;
    for &(number, line) in lines {
        if in_note {
            in_note = !line.starts_with("end note");
            continue;
        }
        if line.starts_with("note ") {
            in_note = !line.contains(':');
            continue;
        }
        if line == "}" {
            open.pop();
            continue;
        }
        if line == "--"
            || line.starts_with("direction ")
            || line.starts_with("classDef ")
            || line.starts_with("class ")
        {
            if let Some(dir) = line.strip_prefix("direction ") {
                d.direction = match dir.trim() {
                    "LR" => crate::model::Direction::Right,
                    "RL" => crate::model::Direction::Left,
                    "BT" => crate::model::Direction::Up,
                    _ => d.direction,
                };
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("state ") {
            let rest = rest.trim();
            let composite = rest.ends_with('{');
            let rest = rest.trim_end_matches('{').trim();
            let (id, name) = match rest.split_once(" as ") {
                Some((name, id)) => (id.trim().to_owned(), label(name)),
                None => {
                    let id = rest
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_owned();
                    (id.clone(), id)
                },
            };
            if composite {
                d.groups.push(Group {
                    id: id.clone(),
                    label: name,
                    parent: open.last().cloned(),
                    tone: None,
                });
                open.push(id);
            } else {
                let node = state(&mut d, &id, &open);
                if rest.contains("<<choice>>") {
                    d.nodes[node].kind = NodeKind::Diamond;
                    d.nodes[node].label = String::new();
                } else if rest.contains("<<fork>>") || rest.contains("<<join>>") {
                    d.nodes[node].kind = NodeKind::Pill;
                    d.nodes[node].label = String::new();
                } else {
                    d.nodes[node].label = name;
                }
            }
            continue;
        }
        let (head, text) = match line.split_once(':') {
            Some((head, text)) => (head.trim(), Some(label(text))),
            None => (line, None),
        };
        if let Some((from, to)) = head.split_once("-->") {
            let scope = open.last().map_or(String::new(), |g| format!("{g}-"));
            let from = endpoint(&mut d, from.trim(), &format!("{scope}start"), &open);
            let to = endpoint(&mut d, to.trim(), &format!("{scope}end"), &open);
            let id = format!("e{}", d.edges.len());
            d.edges.push(Edge {
                label: text.filter(|t| !t.is_empty()),
                end: Head::Arrow,
                ..Edge::new(id, from, to)
            });
        } else if !head.is_empty() && head.chars().all(|c| c.is_alphanumeric() || c == '_') {
            // `A : what it does` describes A; `A` alone names it.
            let node = state(&mut d, head, &open);
            if let Some(text) = text {
                d.nodes[node].sublabel = Some(text);
            }
        } else {
            return Err(Unsupported::at(number, format!("cannot read `{line}`")));
        }
    }
    into_composites(&mut d);
    Ok(d)
}

/// A transition into a composite state goes to its start dot, one out of
/// it from its end dot (made when it has none), and the composite is a
/// group, not a node as well.
fn into_composites(d: &mut Diagram) {
    let composites: Vec<String> = d.groups.iter().map(|g| g.id.clone()).collect();
    for group in composites {
        if !d.nodes.iter().any(|n| n.id == group) {
            continue;
        }
        let open = [group.clone()];
        let (start, end) = (format!("{group}-start"), format!("{group}-end"));
        for edge in &mut d.edges {
            if edge.to == group {
                edge.to.clone_from(&start);
            }
            if edge.from == group {
                edge.from.clone_from(&end);
            }
        }
        for dot in [&start, &end] {
            if d.edges.iter().any(|e| &e.from == dot || &e.to == dot) {
                let node = state(d, dot, &open);
                d.nodes[node].kind = NodeKind::Circle;
                d.nodes[node].label = String::new();
            }
        }
        d.nodes.retain(|n| n.id != group);
    }
}

/// The state `id`, made in the innermost open composite the first time.
fn state(d: &mut Diagram, id: &str, open: &[String]) -> usize {
    if let Some(i) = d.nodes.iter().position(|n| n.id == id) {
        return i;
    }
    let mut node = Node::new(id, id);
    node.kind = NodeKind::Rounded;
    node.group = open.last().cloned();
    d.nodes.push(node);
    d.nodes.len() - 1
}

/// A transition's end: a state, or `[*]` as this scope's start or end dot.
fn endpoint(d: &mut Diagram, text: &str, dot: &str, open: &[String]) -> String {
    if text == "[*]" {
        let node = state(d, dot, open);
        d.nodes[node].kind = NodeKind::Circle;
        d.nodes[node].label = String::new();
        d.nodes[node].at = None;
        return dot.to_owned();
    }
    state(d, text, open);
    text.to_owned()
}
