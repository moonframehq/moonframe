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
    std::collections::{HashMap, HashSet},
};

/// Starts a start or end dot's id until [`name_dots`] gives it one no
/// state has: a state may be named `start` or `end`, even after the
/// transition that made the dot.
const DOT: &str = "\u{0}";

pub(super) fn parse(lines: &[Line<'_>]) -> Result<Diagram, Unsupported> {
    let mut d = Diagram::default();
    let mut open: Vec<String> = Vec::new();
    // The line each composite in `open` opened on.
    let mut opened: Vec<usize> = Vec::new();
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
            if open.pop().is_none() {
                return Err(Unsupported::at(number, "a `}` with no state open"));
            }
            opened.pop();
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
                opened.push(number);
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
            let scope = open.last().map_or(DOT.to_owned(), |g| format!("{DOT}{g}-"));
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
    if let (Some(state), Some(&number)) = (open.last(), opened.last()) {
        return Err(Unsupported::at(
            number,
            format!("`state {state} {{` is never closed"),
        ));
    }
    into_composites(&mut d);
    name_dots(&mut d);
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
        let (start, end) = (format!("{DOT}{group}-start"), format!("{DOT}{group}-end"));
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

/// Each dot its id without the [`DOT`] mark, made unlike any state's or
/// composite's by `_` added as often as needed.
fn name_dots(d: &mut Diagram) {
    let mut taken: HashSet<String> = d
        .nodes
        .iter()
        .map(|n| n.id.clone())
        .chain(d.groups.iter().map(|g| g.id.clone()))
        .collect();
    let mut names: HashMap<String, String> = HashMap::new();
    for node in &mut d.nodes {
        let Some(want) = node.id.strip_prefix(DOT) else {
            continue;
        };
        let mut name = want.to_owned();
        while taken.contains(&name) {
            name.push('_');
        }
        taken.insert(name.clone());
        names.insert(std::mem::replace(&mut node.id, name.clone()), name);
    }
    for edge in &mut d.edges {
        for end in [&mut edge.from, &mut edge.to] {
            if let Some(name) = names.get(end.as_str()) {
                end.clone_from(name);
            }
        }
    }
}
