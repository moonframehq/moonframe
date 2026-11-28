//! `classDiagram`: classes with their members (in a `{ }` block or as
//! `A : +field` lines, shown under the name), and relations (`<|--`
//! inheritance, `*--` composition, `o--` aggregation, `-->` association,
//! `..>` dependency, `..|>` realization, `--` and `..` links, marked at
//! one end or both) with labels and cardinalities, and a shown name
//! (`class A["Name"]`).

use {
    super::{Line, split_outside_quotes},
    crate::{
        model::{Diagram, Edge, EdgeKind, Head, Node},
        parse::{Unsupported, label},
    },
};

/// A relation's marks at its start, longest first, and the heads they draw.
const STARTS: &[(&str, Head)] = &[
    ("<|", Head::Triangle),
    ("<", Head::Arrow),
    ("*", Head::Diamond),
    ("o", Head::HollowDiamond),
    ("", Head::None),
];
/// A relation's marks at its end.
const ENDS: &[(&str, Head)] = &[
    ("|>", Head::Triangle),
    (">", Head::Arrow),
    ("*", Head::Diamond),
    ("o", Head::HollowDiamond),
    ("", Head::None),
];

/// The most members shown under a class's name.
const MEMBERS: usize = 4;

pub(super) fn parse(lines: &[Line<'_>]) -> Result<Diagram, Unsupported> {
    let mut d = Diagram::default();
    let mut members: Vec<(String, Vec<String>)> = Vec::new();
    // The class whose `{ }` members are being read, and the line it opened on.
    let mut inside: Option<(usize, String)> = None;
    for &(number, line) in lines {
        if let Some((_, class)) = &inside {
            if line == "}" {
                inside = None;
            } else if !line.starts_with("<<") {
                add_member(&mut members, class, line);
            }
            continue;
        }
        if let Some(direction) = super::direction(line) {
            d.direction = direction;
            continue;
        }
        if line.starts_with("note")
            || line.starts_with("<<")
            || line.starts_with("style ")
            || line.starts_with("cssClass")
            || line.starts_with("click ")
        {
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            let opens = rest.trim_end().ends_with('{');
            let rest = rest.trim_end().trim_end_matches('{').trim_end();
            let name = class_name(rest);
            known(&mut d, &name);
            // `class A["Shown name"]`.
            if let Some(shown) = rest.split_once('[').and_then(|(_, l)| l.strip_suffix(']'))
                && let Some(node) = d.nodes.iter_mut().find(|n| n.id == name)
            {
                node.label = label(shown);
            }
            if opens {
                inside = Some((number, name));
            }
            continue;
        }
        let (head, text) =
            split_outside_quotes(line, ":").map_or((line, None), |(h, t)| (h, Some(label(t))));
        if let Some(Relation {
            at,
            to: after,
            start,
            end,
            kind,
        }) = relation(head)
        {
            let (left, right) = (&head[..at], &head[after..]);
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
    if let Some((number, class)) = inside {
        return Err(Unsupported::at(
            number,
            format!("the `{{` after class `{class}` is never closed"),
        ));
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

/// A relation in a line: the byte range of its token, its heads and line.
struct Relation {
    at: usize,
    to: usize,
    start: Head,
    end: Head,
    kind: EdgeKind,
}

/// The first relation outside quotes, so a `"0..1"` cardinality is not
/// read as a dashed link: a start mark, `--` or `..`, an end mark. An
/// `o` mark stands apart from a name, so `Foo--Bar` is a plain link.
fn relation(text: &str) -> Option<Relation> {
    let apart = |c: Option<char>| c.is_none_or(|c| c.is_whitespace() || c == '"');
    let mut quoted = false;
    for (i, c) in text.char_indices() {
        if c == '"' {
            quoted = !quoted;
        }
        if quoted || c == '"' {
            continue;
        }
        for &(mark, start) in STARTS {
            if mark == "o" && !apart(text[..i].chars().next_back()) {
                continue;
            }
            let Some(rest) = text[i..].strip_prefix(mark) else {
                continue;
            };
            let kind = if rest.starts_with("--") {
                EdgeKind::Solid
            } else if rest.starts_with("..") {
                EdgeKind::Dashed
            } else {
                continue;
            };
            let rest = &rest[2..];
            let &(end_mark, end) = ENDS
                .iter()
                .find(|(m, _)| {
                    rest.strip_prefix(m)
                        .is_some_and(|after| *m != "o" || apart(after.chars().next()))
                })
                .unwrap_or(&("", Head::None));
            // An arrow on a dashed line is an open one: a dependency.
            let open = |h| match (h, kind) {
                (Head::Arrow, EdgeKind::Dashed) => Head::Open,
                (h, _) => h,
            };
            return Some(Relation {
                at: i,
                to: i + mark.len() + 2 + end_mark.len(),
                start: open(start),
                end: open(end),
                kind,
            });
        }
    }
    None
}
