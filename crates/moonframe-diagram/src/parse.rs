//! Text into the model: Mermaid ([`mermaid`]) and Graphviz DOT ([`dot`]).
//! Our own parsers, so a diagram an agent writes needs no browser or
//! Node.js; layout is always ours, so both look alike.
//!
//! What a parser does not understand it says, with the line, rather than
//! drawing half a diagram: the caller shows the text as code instead.

pub mod dot;
pub mod mermaid;
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

use crate::model::Diagram;

/// The most nodes a parsed diagram may have: text from anywhere (an
/// agent, a file) can't make one that takes the layout minutes.
pub const MOST_NODES: usize = 5_000;
/// The most edges, which `a & b --> c & d` or DOT's `{a b} -> {c d}`
/// multiply.
pub const MOST_EDGES: usize = 20_000;
/// The deepest nesting of subgraphs or groups a parser follows.
pub(crate) const MOST_DEPTH: usize = 64;

/// Why a text is not a diagram this crate draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported {
    /// The line it stopped at, from 1; 0 for the text as a whole.
    pub line: usize,
    pub reason: String,
}

impl Unsupported {
    pub(crate) fn too_big(line: usize) -> Self {
        Self::at(
            line,
            format!("more than {MOST_NODES} nodes or {MOST_EDGES} edges"),
        )
    }

    pub(crate) fn at(line: usize, reason: impl Into<String>) -> Self {
        Self {
            line,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.reason)
        } else {
            write!(f, "line {}: {}", self.line, self.reason)
        }
    }
}

impl std::error::Error for Unsupported {}

/// A fenced block's text by its language: `mermaid`, `dot` or `graphviz`.
/// `None` for another language.
pub fn fenced(language: &str, text: &str) -> Option<Result<Diagram, Unsupported>> {
    match language.trim().to_ascii_lowercase().as_str() {
        "mermaid" => Some(mermaid::parse(text)),
        "dot" | "graphviz" | "gv" => Some(dot::parse(text)),
        _ => None,
    }
}

/// A label as written, made plain: quotes off, `<br>` as a space,
/// Mermaid's markdown strings (`` "`**bold**`" ``) as their words, and its
/// `#quot;` style entities decoded.
pub(crate) fn label(raw: &str) -> String {
    let mut text = raw.trim();
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        text = &text[1..text.len() - 1];
    }
    if text.len() >= 2 && text.starts_with('`') && text.ends_with('`') {
        text = &text[1..text.len() - 1];
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let lower = tail.get(..6).unwrap_or(tail).to_ascii_lowercase();
        if lower.starts_with("<br") {
            out.push(' ');
            rest = tail.find('>').map_or("", |end| &tail[end + 1..]);
        } else if let Some(end) = tail
            .find('>')
            .filter(|_| tail[1..].starts_with(|c: char| c == '/' || c.is_ascii_alphabetic()))
        {
            // Other tags (`<b>`, `</i>`) leave their words.
            rest = &tail[end + 1..];
        } else {
            out.push('<');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    let out = out
        .replace("#quot;", "\"")
        .replace("#amp;", "&")
        .replace("#lt;", "<")
        .replace("#gt;", ">")
        .replace("#35;", "#")
        .replace("**", "")
        .replace("\\n", " ");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
