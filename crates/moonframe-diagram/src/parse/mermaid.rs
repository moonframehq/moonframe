//! Mermaid, without mermaid.js: `flowchart`/`graph`, `sequenceDiagram`,
//! `stateDiagram`, `classDiagram` and `erDiagram` into the model. Front
//! matter, `%%{init}%%` directives and `%%` comments are passed over;
//! styling becomes fills and tones where it maps, else it is dropped.
//! Other diagram types (gantt, pie, mindmap...) are refused by name.

mod class;
mod er;
mod flowchart;
mod sequence;
mod state;

use {super::Unsupported, crate::model::Diagram};

/// One meaningful line: its number from 1, and its text trimmed.
pub(super) type Line<'a> = (usize, &'a str);

pub fn parse(text: &str) -> Result<Diagram, Unsupported> {
    let lines = meaningful(text);
    let Some(&(number, first)) = lines.first() else {
        return Err(Unsupported::at(0, "the diagram is empty"));
    };
    let kind = first.split_whitespace().next().unwrap_or_default();
    match kind {
        "flowchart" | "flowchart-elk" | "graph" => flowchart::parse(&lines),
        "sequenceDiagram" => sequence::parse(&lines[1..]),
        "stateDiagram" | "stateDiagram-v2" => state::parse(&lines[1..]),
        "classDiagram" | "classDiagram-v2" => class::parse(&lines[1..]),
        "erDiagram" => er::parse(&lines[1..]),
        other => Err(Unsupported::at(
            number,
            format!("`{other}` diagrams are not drawn yet"),
        )),
    }
}

/// The lines that say something: no front matter, directive or comment.
fn meaningful(text: &str) -> Vec<Line<'_>> {
    let mut lines: Vec<Line<'_>> = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .collect();
    // YAML front matter between `---` lines, at the very top.
    if lines
        .iter()
        .find(|(_, l)| !l.is_empty())
        .is_some_and(|(_, l)| *l == "---")
    {
        let open = lines.iter().position(|(_, l)| *l == "---").unwrap_or(0);
        if let Some(close) = lines[open + 1..].iter().position(|(_, l)| *l == "---") {
            lines.drain(..open + close + 2);
        }
    }
    lines.retain(|(_, l)| !l.is_empty() && !l.starts_with("%%"));
    lines
}

/// `text` before and after the first `sep` outside double quotes.
pub(super) fn split_outside_quotes<'a>(text: &'a str, sep: &str) -> Option<(&'a str, &'a str)> {
    let mut quoted = false;
    for (i, c) in text.char_indices() {
        if c == '"' {
            quoted = !quoted;
        } else if !quoted && text[i..].starts_with(sep) {
            return Some((&text[..i], &text[i + sep.len()..]));
        }
    }
    None
}
