//! `flowchart` and `graph`: nodes with their shapes (`[box]`, `(rounded)`,
//! `([stadium])`, `[(database)]`, `((circle))`, `{decision}`, ...), chains
//! of edges (`-->`, `---`, `-.->`, `==>`, `--o`, `--x`, `<-->`, `~~~`) with
//! labels (`-->|yes|`, `-- yes -->`), `&` for several at once, subgraphs,
//! and `classDef`, `class`, `:::` and `style` as fills and tones.

use {
    super::{Line, split_outside_quotes},
    crate::{
        model::{Diagram, Direction, Edge, EdgeKind, Group, Head, Node, NodeKind, Tone},
        parse::{Unsupported, label},
    },
    std::collections::HashMap,
};

/// A node's shape by its brackets, longest openers first.
const SHAPES: &[(&str, &str, NodeKind)] = &[
    ("(((", ")))", NodeKind::Circle),
    ("((", "))", NodeKind::Circle),
    ("([", "])", NodeKind::Pill),
    ("[[", "]]", NodeKind::Box),
    ("[(", ")]", NodeKind::Cylinder),
    ("{{", "}}", NodeKind::Rounded),
    ("[/", "/]", NodeKind::Box),
    ("[/", "\\]", NodeKind::Box),
    ("[\\", "\\]", NodeKind::Box),
    ("[\\", "/]", NodeKind::Box),
    ("(", ")", NodeKind::Rounded),
    ("[", "]", NodeKind::Box),
    ("{", "}", NodeKind::Diamond),
    (">", "]", NodeKind::Note),
];

/// What a `classDef` gives a node.
#[derive(Clone, Default)]
struct Look {
    fill: Option<String>,
    tone: Option<Tone>,
}

#[derive(Default)]
struct Builder {
    diagram: Diagram,
    index: HashMap<String, usize>,
    /// The subgraphs the statement is inside, innermost last, with the
    /// line each opened on.
    open: Vec<(String, usize)>,
    looks: HashMap<String, Look>,
    /// Classes given to nodes, applied once every `classDef` is known.
    classed: Vec<(usize, String)>,
    groups_made: usize,
}

pub(super) fn parse(lines: &[Line<'_>]) -> Result<Diagram, Unsupported> {
    let mut b = Builder::default();
    let (first_number, header) = lines[0];
    // `graph TD; A --> B` may hold statements after its header.
    let (head, rest) = header.split_once(';').unwrap_or((header, ""));
    b.diagram.direction = match head.split_whitespace().nth(1) {
        Some("BT") => Direction::Up,
        Some("LR") => Direction::Right,
        Some("RL") => Direction::Left,
        _ => Direction::Down,
    };
    let mut statements: Vec<(usize, &str)> = vec![(first_number, rest)];
    statements.extend(lines[1..].iter().copied());
    for (number, line) in statements {
        for statement in split_statements(line) {
            b.statement(number, statement.trim())?;
        }
    }
    if let Some((group, number)) = b.open.last() {
        return Err(Unsupported::at(
            *number,
            format!("`subgraph {group}` is never closed with `end`"),
        ));
    }
    for (node, class) in std::mem::take(&mut b.classed) {
        if let Some(look) = b.looks.get(&class) {
            let node = &mut b.diagram.nodes[node];
            node.fill = look.fill.clone().or(node.fill.take());
            node.tone = look.tone.or(node.tone);
        }
    }
    Ok(b.diagram)
}

/// A line's statements, split at `;` outside quotes and brackets.
fn split_statements(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quoted, mut start) = (0i32, false, 0);
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '[' | '(' | '{' if !quoted => depth += 1,
            ']' | ')' | '}' if !quoted => depth -= 1,
            ';' if !quoted && depth <= 0 => {
                out.push(&line[start..i]);
                start = i + 1;
            },
            _ => {},
        }
    }
    out.push(&line[start..]);
    out.into_iter().filter(|s| !s.trim().is_empty()).collect()
}

impl Builder {
    fn statement(&mut self, number: usize, s: &str) -> Result<(), Unsupported> {
        let word = s.split_whitespace().next().unwrap_or_default();
        match word {
            "subgraph" => self.subgraph(number, s["subgraph".len()..].trim())?,
            "end" if s == "end" => {
                if self.open.pop().is_none() {
                    return Err(Unsupported::at(number, "an `end` with no `subgraph` open"));
                }
            },
            "direction" | "linkStyle" | "click" | "accTitle" | "accDescr" | "title" => {},
            "classDef" => {
                let mut parts = s.split_whitespace().skip(1);
                let names = parts.next().unwrap_or_default();
                let look = look(&parts.collect::<Vec<_>>().join(" "), names);
                for name in names.split(',') {
                    self.looks.insert(name.to_owned(), look.clone());
                }
            },
            "class" => {
                let mut parts = s.split_whitespace().skip(1);
                let (Some(ids), Some(class)) = (parts.next(), parts.next()) else {
                    return Ok(());
                };
                for id in ids.split(',') {
                    let node = self.node_index(number, id)?;
                    self.classed.push((node, class.to_owned()));
                }
            },
            "style" => {
                let mut parts = s.split_whitespace().skip(1);
                if let Some(id) = parts.next() {
                    let given = look(&parts.collect::<Vec<_>>().join(" "), "");
                    let node = self.node_index(number, id)?;
                    self.diagram.nodes[node].fill = given.fill;
                }
            },
            _ if s.starts_with("accTitle:") || s.starts_with("accDescr") => {},
            _ => self.chain(number, s)?,
        }
        Ok(())
    }

    fn subgraph(&mut self, number: usize, rest: &str) -> Result<(), Unsupported> {
        if self.open.len() >= crate::parse::MOST_DEPTH {
            return Err(Unsupported::at(number, "subgraphs nest too deep"));
        }
        self.groups_made += 1;
        let (id, title) = if rest.starts_with('"') || rest.is_empty() {
            (format!("subgraph{}", self.groups_made), label(rest))
        } else {
            let end = rest
                .find(|c: char| c == '[' || c.is_whitespace())
                .unwrap_or(rest.len());
            let id = rest[..end].to_owned();
            let after = rest[end..].trim();
            let title = match after.strip_prefix('[').and_then(|a| a.strip_suffix(']')) {
                Some(inside) => label(inside),
                None if after.is_empty() => id.clone(),
                None => label(after),
            };
            (id, title)
        };
        self.diagram.groups.push(Group {
            id: id.clone(),
            label: title,
            parent: self.open.last().map(|(g, _)| g.clone()),
            tone: None,
        });
        self.open.push((id, number));
        Ok(())
    }

    /// The node `id`, made the first time it is named, unless the diagram
    /// already has as many as it may.
    fn node_index(&mut self, number: usize, id: &str) -> Result<usize, Unsupported> {
        if let Some(&i) = self.index.get(id) {
            return Ok(i);
        }
        if self.diagram.nodes.len() >= crate::parse::MOST_NODES {
            return Err(Unsupported::too_big(number));
        }
        let mut node = Node::new(id, id);
        node.group = self.open.last().map(|(g, _)| g.clone());
        self.diagram.nodes.push(node);
        let i = self.diagram.nodes.len() - 1;
        self.index.insert(id.to_owned(), i);
        Ok(i)
    }

    /// `a --> b -->|x| c & d`: nodes, then edges between each group of
    /// them and the next.
    fn chain(&mut self, number: usize, s: &str) -> Result<(), Unsupported> {
        let mut c = Cursor { s, at: 0 };
        let mut previous = self.nodes(&mut c, number)?;
        loop {
            c.skip_spaces();
            if c.done() {
                return Ok(());
            }
            let Some(spec) = edge(&mut c) else {
                return Err(Unsupported::at(
                    number,
                    format!("cannot read `{}`", c.rest()),
                ));
            };
            c.skip_spaces();
            let next = self.nodes(&mut c, number)?;
            if self.diagram.edges.len() + previous.len() * next.len() > crate::parse::MOST_EDGES {
                return Err(Unsupported::too_big(number));
            }
            for &a in &previous {
                for &b in &next {
                    let id = format!("e{}", self.diagram.edges.len());
                    let (from, to) = (
                        self.diagram.nodes[a].id.clone(),
                        self.diagram.nodes[b].id.clone(),
                    );
                    self.diagram.edges.push(Edge {
                        id,
                        from,
                        to,
                        label: spec.label.clone(),
                        kind: spec.kind,
                        start: spec.start,
                        end: spec.end,
                        tone: None,
                    });
                }
            }
            previous = next;
        }
    }

    fn nodes(&mut self, c: &mut Cursor<'_>, number: usize) -> Result<Vec<usize>, Unsupported> {
        let mut out = vec![self.node(c, number)?];
        loop {
            let mark = c.at;
            c.skip_spaces();
            if c.eat("&") {
                c.skip_spaces();
                out.push(self.node(c, number)?);
            } else {
                c.at = mark;
                return Ok(out);
            }
        }
    }

    fn node(&mut self, c: &mut Cursor<'_>, number: usize) -> Result<usize, Unsupported> {
        let id = c.ident();
        if id.is_empty() {
            return Err(Unsupported::at(
                number,
                format!("expected a node at `{}`", c.rest()),
            ));
        }
        let index = self.node_index(number, id)?;
        // Named inside a subgraph, a node with none yet joins it, as
        // Mermaid has it: `a --> b` first, then `subgraph g` listing `b`.
        if self.diagram.nodes[index].group.is_none() {
            self.diagram.nodes[index].group = self.open.last().map(|(g, _)| g.clone());
        }
        // Mermaid 11's `A@{ shape: ..., label: ... }`: the label alone.
        if c.eat("@{") {
            let body = c.until("}").unwrap_or_default();
            if let Some((_, value)) = body
                .split(',')
                .filter_map(|p| p.split_once(':'))
                .find(|(k, _)| k.trim() == "label")
            {
                self.diagram.nodes[index].label = label(value);
            }
        }
        // Trapezoids share openers: of the entries opening here, the one
        // whose closer comes first.
        let opening = SHAPES
            .iter()
            .find(|s| c.rest().starts_with(s.0))
            .map(|s| s.0);
        let shape = opening.and_then(|open| {
            let after = &c.rest()[open.len()..];
            SHAPES
                .iter()
                .filter(|s| s.0 == open)
                .min_by_key(|s| after.find(s.1).unwrap_or(usize::MAX))
        });
        if let Some(&(open, close, kind)) = shape {
            c.at += open.len();
            let text = if c.rest().starts_with('"') {
                let quoted = c.quoted();
                c.until(close);
                quoted
            } else {
                c.until(close).ok_or_else(|| {
                    Unsupported::at(number, format!("`{open}` is never closed with `{close}`"))
                })?
            };
            let node = &mut self.diagram.nodes[index];
            node.label = label(text);
            node.kind = kind;
        }
        if c.eat(":::") {
            let class = c.ident().to_owned();
            self.classed.push((index, class));
        }
        Ok(index)
    }
}

/// What an edge token says.
struct Spec {
    kind: EdgeKind,
    start: Head,
    end: Head,
    label: Option<String>,
}

/// An edge at the cursor: its line, heads and label, or `None` when
/// there is none.
fn edge(c: &mut Cursor<'_>) -> Option<Spec> {
    let start_at = c.at;
    let rest = c.rest();
    if rest.starts_with("~~~") {
        c.at += rest.chars().take_while(|&ch| ch == '~').count();
        return Some(Spec {
            kind: EdgeKind::Invisible,
            start: Head::None,
            end: Head::None,
            label: None,
        });
    }
    let bytes = rest.as_bytes();
    let mut i = 0;
    let start = match bytes.first() {
        Some(b'<') => {
            i = 1;
            Head::Arrow
        },
        Some(b'o' | b'x') if rest[1..].starts_with("--") || rest[1..].starts_with("==") => {
            i = 1;
            if bytes[0] == b'o' {
                Head::Circle
            } else {
                Head::Cross
            }
        },
        _ => Head::None,
    };
    let line_len = rest[i..]
        .chars()
        .take_while(|ch| matches!(ch, '-' | '.' | '='))
        .count();
    if line_len < 2 {
        c.at = start_at;
        return None;
    }
    let line = &rest[i..i + line_len];
    let kind = if line.contains('=') {
        EdgeKind::Thick
    } else if line.contains('.') {
        EdgeKind::Dashed
    } else {
        EdgeKind::Solid
    };
    let mut j = i + line_len;
    let after = &rest[j..];
    let head = |after: &str| match after.chars().next() {
        Some('>') => Some((Head::Arrow, 1)),
        Some(h @ ('o' | 'x'))
            if after[1..]
                .chars()
                .next()
                .is_none_or(|n| n.is_whitespace() || n == '|') =>
        {
            Some((
                if h == 'o' {
                    Head::Circle
                } else {
                    Head::Cross
                },
                1,
            ))
        },
        _ => None,
    };
    let mut label_text = None;
    let end = match head(after) {
        Some((h, n)) => {
            j += n;
            h
        },
        // `-- text -->`: the words up to the arrow that closes them.
        None if matches!(line, "--" | "==" | "-.") && after.starts_with(' ') => {
            let closers = [
                "-->", "---", "==>", "===", ".->", ".-", "--o", "--x", "==o", "==x",
            ];
            let found = closers
                .iter()
                .filter_map(|t| after.find(t).map(|at| (at, *t)))
                .min_by_key(|(at, t)| (*at, std::cmp::Reverse(t.len())));
            let Some((at, closer)) = found else {
                c.at = start_at;
                return None;
            };
            label_text = Some(label(&after[..at]));
            j += at + closer.len();
            match closer.chars().last() {
                Some('>') => Head::Arrow,
                Some('o') => Head::Circle,
                Some('x') => Head::Cross,
                _ => Head::None,
            }
        },
        None => Head::None,
    };
    c.at = start_at + j;
    c.skip_spaces();
    if c.rest().starts_with('|') {
        c.at += 1;
        if let Some(text) = c.until("|") {
            label_text = Some(label(text));
        }
    }
    Some(Spec {
        kind,
        start,
        end,
        label: label_text.filter(|l| !l.is_empty()),
    })
}

/// A `classDef` or `style` body (`fill:#f9f,stroke:#333`) as a look; a
/// class named like a tone (`danger`, `success`) takes that tone.
fn look(body: &str, name: &str) -> Look {
    let mut out = Look {
        tone: name.split(',').find_map(Tone::from_name),
        ..Look::default()
    };
    for part in body.split(',') {
        if let Some((key, value)) = part.split_once(':')
            && key.trim() == "fill"
        {
            let value = value.trim().trim_end_matches(';');
            if crate::model::is_hex_color(value) {
                out.fill = Some(value.to_owned());
            }
        }
    }
    out
}

/// A position in one statement.
struct Cursor<'a> {
    s: &'a str,
    at: usize,
}

impl<'a> Cursor<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.at..]
    }

    fn done(&self) -> bool {
        self.at >= self.s.len()
    }

    fn skip_spaces(&mut self) {
        self.at += self.rest().len() - self.rest().trim_start().len();
    }

    fn eat(&mut self, token: &str) -> bool {
        let found = self.rest().starts_with(token);
        if found {
            self.at += token.len();
        }
        found
    }

    /// A node id: letters, digits, `_`, and `-` between them (`api-gw`,
    /// but `a-->b` is `a` then an edge).
    fn ident(&mut self) -> &'a str {
        let rest = self.rest();
        let mut end = 0;
        let mut chars = rest.char_indices().peekable();
        while let Some((i, ch)) = chars.next() {
            let next = chars.peek().map(|&(_, n)| n);
            let ok = ch.is_alphanumeric()
                || ch == '_'
                || (ch == '-' && end > 0 && next.is_some_and(char::is_alphanumeric));
            if !ok {
                break;
            }
            end = i + ch.len_utf8();
        }
        self.at += end;
        &rest[..end]
    }

    /// A `"quoted"` text, the quotes off.
    fn quoted(&mut self) -> &'a str {
        let rest = self.rest();
        match rest[1..].find('"') {
            Some(end) => {
                self.at += end + 2;
                &rest[1..end + 1]
            },
            None => {
                self.at = self.s.len();
                &rest[1..]
            },
        }
    }

    /// What comes before `close`, which is passed over.
    fn until(&mut self, close: &str) -> Option<&'a str> {
        let rest = self.rest();
        let (before, _) = split_outside_quotes(rest, close)?;
        self.at += before.len() + close.len();
        Some(before)
    }
}
