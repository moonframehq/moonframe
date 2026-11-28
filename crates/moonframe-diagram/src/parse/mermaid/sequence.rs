//! `sequenceDiagram`: participants and actors (`participant A as Alice`),
//! messages (`->>`, `-->>`, `->`, `-->`, `-x`, `--x`, `-)`, `--)`, with
//! `+`/`-` activations), and notes (`Note right of A`, `Note over A,B`).
//! Blocks (`loop`, `alt`, `opt`, `par`, `rect`, `critical`, `break`, `box`)
//! keep their messages; their frames are not drawn yet.

use {
    super::Line,
    crate::{
        model::{Diagram, DiagramKind, Message, MessageKind, Node, NodeKind, Note},
        parse::{Unsupported, label},
    },
};

/// Arrows, longest first so `-->>` is not read as `-->`.
const ARROWS: &[(&str, MessageKind)] = &[
    ("-->>", MessageKind::Reply),
    ("->>", MessageKind::Sync),
    ("<<->>", MessageKind::Sync),
    ("<<-->>", MessageKind::Reply),
    ("--x", MessageKind::Reply),
    ("-x", MessageKind::Sync),
    ("--)", MessageKind::Async),
    ("-)", MessageKind::Async),
    ("-->", MessageKind::Reply),
    ("->", MessageKind::Sync),
];

/// Words that open a block, closed by `end`.
const BLOCKS: &[&str] = &[
    "alt", "box", "break", "critical", "loop", "opt", "par", "rect",
];

/// Words that divide a block in parts, only inside one.
const PARTS: &[&str] = &["and", "else", "option"];

const IGNORED: &[&str] = &[
    "autonumber",
    "activate",
    "deactivate",
    "title",
    "accTitle",
    "accDescr",
    "links",
    "link",
    "destroy",
    "create",
];

pub(super) fn parse(lines: &[Line<'_>]) -> Result<Diagram, Unsupported> {
    let mut d = Diagram {
        kind: DiagramKind::Sequence,
        ..Diagram::default()
    };
    // The line each open block began on.
    let mut blocks: Vec<usize> = Vec::new();
    for &(number, line) in lines {
        let word = line.split_whitespace().next().unwrap_or_default();
        let word = word.trim_end_matches(':');
        if BLOCKS.contains(&word) {
            blocks.push(number);
        } else if word == "end" {
            if blocks.pop().is_none() {
                return Err(Unsupported::at(number, "an `end` with no block open"));
            }
        } else if PARTS.contains(&word) {
            if blocks.is_empty() {
                return Err(Unsupported::at(number, format!("`{word}` outside a block")));
            }
        } else if word == "participant" || word == "actor" {
            participant(&mut d, line[word.len()..].trim(), word == "actor");
        } else if word.eq_ignore_ascii_case("note") {
            note(&mut d, line, number)?;
        } else if IGNORED.contains(&word) {
        } else {
            message(&mut d, line, number)?;
        }
    }
    if let Some(&number) = blocks.last() {
        return Err(Unsupported::at(
            number,
            "a block is never closed with `end`",
        ));
    }
    Ok(d)
}

/// `A`, `A as Alice` or `"Long name" as A`.
fn participant(d: &mut Diagram, rest: &str, actor: bool) {
    let (id, name) = match rest.split_once(" as ") {
        Some((id, name)) => (label(id), label(name)),
        None => (label(rest), label(rest)),
    };
    let mut node = Node::new(id.clone(), name);
    if actor {
        node.kind = NodeKind::Pill;
    }
    match d.nodes.iter_mut().find(|n| n.id == id) {
        Some(known) => *known = node,
        None => d.nodes.push(node),
    }
}

fn known(d: &mut Diagram, id: &str) {
    if !d.nodes.iter().any(|n| n.id == id) {
        d.nodes.push(Node::new(id, id));
    }
}

/// `A->>+B: text`.
fn message(d: &mut Diagram, line: &str, number: usize) -> Result<(), Unsupported> {
    let (head, text) = line.split_once(':').unwrap_or((line, ""));
    let found = ARROWS
        .iter()
        .filter_map(|(arrow, kind)| head.find(arrow).map(|at| (at, *arrow, *kind)))
        .min_by_key(|(at, arrow, _)| (*at, std::cmp::Reverse(arrow.len())));
    let Some((at, arrow, kind)) = found else {
        return Err(Unsupported::at(number, format!("cannot read `{line}`")));
    };
    let from = head[..at].trim();
    let to = head[at + arrow.len()..]
        .trim()
        .trim_start_matches(['+', '-'])
        .trim();
    if from.is_empty() || to.is_empty() {
        return Err(Unsupported::at(
            number,
            format!("a message needs two participants: `{line}`"),
        ));
    }
    known(d, from);
    known(d, to);
    d.messages.push(Message {
        from: from.to_owned(),
        to: to.to_owned(),
        label: label(text),
        kind,
    });
    Ok(())
}

/// `Note right of A: text`, `Note left of A: ...`, `Note over A,B: ...`.
fn note(d: &mut Diagram, line: &str, number: usize) -> Result<(), Unsupported> {
    let (head, text) = line.split_once(':').unwrap_or((line, ""));
    let words: Vec<&str> = head.split_whitespace().skip(1).collect();
    let over = match words.as_slice() {
        ["right" | "left", "of", who @ ..] | ["over", who @ ..] => who.join(" "),
        _ => return Err(Unsupported::at(number, format!("cannot read `{line}`"))),
    };
    let over: Vec<String> = over
        .split(',')
        .map(|w| w.trim().to_owned())
        .filter(|w| !w.is_empty())
        .collect();
    for id in &over {
        known(d, id);
    }
    d.notes.push(Note {
        over,
        text: label(text),
        before: d.messages.len(),
    });
    Ok(())
}
