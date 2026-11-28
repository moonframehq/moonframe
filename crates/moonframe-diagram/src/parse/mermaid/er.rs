//! `erDiagram`: entities with their attributes (shown under the name) and
//! relationships (`CUSTOMER ||--o{ ORDER : places`). A "many" end draws an
//! open head, a "one" end none; `..` is a non-identifying (dashed) line.

use {
    super::Line,
    crate::{
        model::{Diagram, Edge, EdgeKind, Head, Node},
        parse::{Unsupported, label},
    },
};

/// The most attributes shown under an entity's name.
const ATTRIBUTES: usize = 4;

pub(super) fn parse(lines: &[Line<'_>]) -> Result<Diagram, Unsupported> {
    let mut d = Diagram::default();
    // The entity whose `{ }` attributes are being read, and its line.
    let mut inside: Option<(usize, String, Vec<String>)> = None;
    for &(number, line) in lines {
        if let Some((_, entity, attributes)) = &mut inside {
            if line == "}" {
                let shown: Vec<String> = attributes.iter().take(ATTRIBUTES).cloned().collect();
                let entity = entity.clone();
                if let Some(node) = d.nodes.iter_mut().find(|n| n.id == entity) {
                    node.sublabel = Some(shown.join(", "));
                }
                inside = None;
            } else {
                // `string name PK "comment"`: the name.
                let name = line.split_whitespace().nth(1).unwrap_or(line);
                attributes.push(name.to_owned());
            }
            continue;
        }
        if line.starts_with("direction ") || line.starts_with("title ") {
            continue;
        }
        if let Some(entity) = line.strip_suffix('{') {
            let entity = entity
                .trim()
                .split('[')
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            known(&mut d, &entity);
            inside = Some((number, entity, Vec::new()));
            continue;
        }
        let (head, text) = line
            .split_once(':')
            .map_or((line, None), |(h, t)| (h, Some(label(t))));
        let words: Vec<&str> = head.split_whitespace().collect();
        if let [from, link, to] = words.as_slice() {
            let Some((start, kind, end)) = relationship(link) else {
                return Err(Unsupported::at(number, format!("cannot read `{line}`")));
            };
            known(&mut d, from);
            known(&mut d, to);
            let id = format!("e{}", d.edges.len());
            d.edges.push(Edge {
                label: text.filter(|t| !t.is_empty()),
                kind,
                start,
                end,
                ..Edge::new(id, *from, *to)
            });
        } else if let [entity] = words.as_slice() {
            known(&mut d, entity);
        } else {
            return Err(Unsupported::at(number, format!("cannot read `{line}`")));
        }
    }
    if let Some((number, entity, _)) = inside {
        return Err(Unsupported::at(
            number,
            format!("the `{{` after entity `{entity}` is never closed"),
        ));
    }
    Ok(d)
}

fn known(d: &mut Diagram, name: &str) {
    if !d.nodes.iter().any(|n| n.id == name) {
        d.nodes.push(Node::new(name, name));
    }
}

/// `||--o{` as its heads and line: two marks, `--` or `..`, two marks.
fn relationship(link: &str) -> Option<(Head, EdgeKind, Head)> {
    if link.len() < 6 || !link.is_ascii() {
        return None;
    }
    let (left, line, right) = (&link[..2], &link[2..4], &link[4..]);
    let kind = match line {
        "--" => EdgeKind::Solid,
        ".." => EdgeKind::Dashed,
        _ => return None,
    };
    let many = |marks: &str| marks.contains('{') || marks.contains('}');
    let head = |marks: &str| {
        if many(marks) {
            Head::Open
        } else {
            Head::None
        }
    };
    Some((head(left), kind, head(right)))
}
