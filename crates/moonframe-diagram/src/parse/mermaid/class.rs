//! `classDiagram`: classes with their members (in a `{ }` block or as
//! `A : +field` lines, shown under the name), and relations (`<|--`
//! inheritance, `*--` composition, `o--` aggregation, `-->` association,
//! `..>` dependency, `..|>` realization, `--` and `..` links) with labels
//! and cardinalities.

use {
    super::Line,
    crate::{
        model::{Diagram, Edge, EdgeKind, Head, Node},
        parse::{Unsupported, label},
    },
};

/// Relations, longest first, with the head each end gets.
const RELATIONS: &[(&str, Head, Head, EdgeKind)] = &[
    ("<|--", Head::Triangle, Head::None, EdgeKind::Solid),
    ("--|>", Head::None, Head::Triangle, EdgeKind::Solid),
    ("<|..", Head::Triangle, Head::None, EdgeKind::Dashed),
    ("..|>", Head::None, Head::Triangle, EdgeKind::Dashed),
    ("*--", Head::Diamond, Head::None, EdgeKind::Solid),
    ("--*", Head::None, Head::Diamond, EdgeKind::Solid),
    ("o--", Head::HollowDiamond, Head::None, EdgeKind::Solid),
    ("--o", Head::None, Head::HollowDiamond, EdgeKind::Solid),
    ("<--", Head::Arrow, Head::None, EdgeKind::Solid),
    ("-->", Head::None, Head::Arrow, EdgeKind::Solid),
    ("<..", Head::Open, Head::None, EdgeKind::Dashed),
    ("..>", Head::None, Head::Open, EdgeKind::Dashed),
    ("--", Head::None, Head::None, EdgeKind::Solid),
    ("..", Head::None, Head::None, EdgeKind::Dashed),
];

/// The most members shown under a class's name.
const MEMBERS: usize = 4;

pub(super) fn parse(lines: &[Line<'_>]) -> Result<Diagram, Unsupported> {
    let mut d = Diagram::default();
    let mut members: Vec<(String, Vec<String>)> = Vec::new();
    let mut inside: Option<String> = None;
    for &(number, line) in lines {
        if let Some(class) = &inside {
            if line == "}" {
                inside = None;
            } else if !line.starts_with("<<") {
                add_member(&mut members, class, line);
            }
            continue;
        }
        if line.starts_with("direction ")
            || line.starts_with("note")
            || line.starts_with("<<")
            || line.starts_with("style ")
            || line.starts_with("cssClass")
            || line.starts_with("click ")
        {
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            let opens = rest.trim_end().ends_with('{');
            let name = class_name(rest.trim_end_matches('{'));
            known(&mut d, &name);
            if opens {
                inside = Some(name);
            }
            continue;
        }
        if let Some(found) = RELATIONS
            .iter()
            .filter_map(|r| line.find(r.0).map(|at| (at, r)))
            .min_by_key(|(at, r)| (*at, std::cmp::Reverse(r.0.len())))
        {
            let (at, &(token, start, end, kind)) = found;
            let (left, rest) = (&line[..at], &line[at + token.len()..]);
            let (right, text) = rest
                .split_once(':')
                .map_or((rest, None), |(r, t)| (r, Some(label(t))));
            let (from, to) = (
                class_name(&strip_cardinality(left)),
                class_name(&strip_cardinality(right)),
            );
            if from.is_empty() || to.is_empty() {
                return Err(Unsupported::at(number, format!("cannot read `{line}`")));
            }
            known(&mut d, &from);
            known(&mut d, &to);
            let id = format!("e{}", d.edges.len());
            d.edges.push(Edge {
                label: text.filter(|t| !t.is_empty()),
                kind,
                start,
                end,
                ..Edge::new(id, from, to)
            });
            continue;
        }
        if let Some((class, member)) = line.split_once(':') {
            let class = class_name(class);
            known(&mut d, &class);
            add_member(&mut members, &class, member);
            continue;
        }
        return Err(Unsupported::at(number, format!("cannot read `{line}`")));
    }
    for (class, list) in members {
        if let Some(node) = d.nodes.iter_mut().find(|n| n.id == class) {
            let mut shown: Vec<String> = list.iter().take(MEMBERS).cloned().collect();
            if list.len() > MEMBERS {
                shown.push("…".into());
            }
            node.sublabel = Some(shown.join(", "));
        }
    }
    Ok(d)
}

fn known(d: &mut Diagram, name: &str) {
    if !d.nodes.iter().any(|n| n.id == name) {
        d.nodes.push(Node::new(name, name));
    }
}

fn add_member(members: &mut Vec<(String, Vec<String>)>, class: &str, member: &str) {
    let member = member.trim().to_owned();
    if member.is_empty() {
        return;
    }
    match members.iter_mut().find(|(c, _)| c == class) {
        Some((_, list)) => list.push(member),
        None => members.push((class.to_owned(), vec![member])),
    }
}

/// A class name without its generic part (`List~T~`) or label.
fn class_name(text: &str) -> String {
    let text = text.trim();
    let text = text.split(['~', '[', ' ']).next().unwrap_or(text);
    label(text)
}

/// A relation's side without its `"1..*"` cardinality.
fn strip_cardinality(text: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    for c in text.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if !quoted {
            out.push(c);
        }
    }
    out
}
