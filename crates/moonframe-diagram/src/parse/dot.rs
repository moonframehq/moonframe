//! Graphviz DOT: `graph` and `digraph`, node and edge statements with
//! attribute lists, `node`/`edge` defaults per scope, chains (`a -> b ->
//! c`), subgraphs as edge ends, and clusters (`subgraph cluster_x`) as
//! groups. `rankdir`, `shape`, `label`, `style`, `fillcolor`, `dir` and
//! `arrowhead`/`arrowtail` map onto the model; the rest is passed over.
//! The layout is ours, not Graphviz's.

use {
    super::{Unsupported, label},
    crate::model::{Diagram, Direction, Edge, EdgeKind, Group, Head, Node, NodeKind},
    std::collections::HashMap,
};

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Id(String),
    Punct(char),
    /// `->` or `--`.
    Link,
}

/// A token and its line.
type Lexed = (Token, usize);

fn lex(text: &str) -> Result<Vec<Lexed>, Unsupported> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                line += 1;
                i += 1;
            },
            c if c.is_whitespace() => i += 1,
            // `#` lines are a preprocessor's: passed over, as Graphviz does.
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            },
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            },
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    line += usize::from(chars[i] == '\n');
                    i += 1;
                }
                i += 2;
            },
            '-' if matches!(chars.get(i + 1), Some('>' | '-')) => {
                out.push((Token::Link, line));
                i += 2;
            },
            '{' | '}' | '[' | ']' | ';' | ',' | '=' | ':' => {
                out.push((Token::Punct(c), line));
                i += 1;
            },
            '"' => {
                let mut s = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        match chars[i + 1] {
                            '"' => s.push('"'),
                            'n' | 'l' | 'r' => s.push(' '),
                            other => {
                                s.push('\\');
                                s.push(other);
                            },
                        }
                        i += 2;
                        continue;
                    }
                    line += usize::from(chars[i] == '\n');
                    s.push(chars[i]);
                    i += 1;
                }
                i += 1;
                out.push((Token::Id(s), line));
            },
            '<' => {
                // An HTML label: its text, tags dropped.
                let mut depth = 0;
                let start = i;
                while i < chars.len() {
                    match chars[i] {
                        '<' => depth += 1,
                        '>' => {
                            depth -= 1;
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        },
                        _ => {},
                    }
                    i += 1;
                }
                if depth != 0 {
                    return Err(Unsupported::at(line, "an HTML label `<` is never closed"));
                }
                let html: String = chars[start + 1..(i - 1).max(start + 1)].iter().collect();
                out.push((Token::Id(label(&html)), line));
            },
            c if c.is_alphanumeric() || c == '_' || c == '.' || c == '-' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric()
                        || chars[i] == '_'
                        || chars[i] == '.'
                        || (chars[i] == '-' && !matches!(chars.get(i + 1), Some('>' | '-'))))
                {
                    i += 1;
                }
                out.push((Token::Id(chars[start..i].iter().collect()), line));
            },
            other => return Err(Unsupported::at(line, format!("unexpected `{other}`"))),
        }
    }
    Ok(out)
}

type Attributes = HashMap<String, String>;

struct Parser {
    tokens: Vec<Lexed>,
    at: usize,
    directed: bool,
    diagram: Diagram,
    /// Each node's attributes as given so far, merged over the defaults.
    nodes: HashMap<String, usize>,
    clusters: usize,
    /// How many `{ }` blocks the statement being read is inside.
    depth: usize,
}

pub fn parse(text: &str) -> Result<Diagram, Unsupported> {
    let mut p = Parser {
        tokens: lex(text)?,
        at: 0,
        directed: true,
        diagram: Diagram::default(),
        nodes: HashMap::new(),
        clusters: 0,
        depth: 0,
    };
    if p.peek_id()
        .is_some_and(|w| w.eq_ignore_ascii_case("strict"))
    {
        p.at += 1;
    }
    match p.next_id().map(|w| w.to_ascii_lowercase()).as_deref() {
        Some("digraph") => p.directed = true,
        Some("graph") => p.directed = false,
        _ => {
            return Err(Unsupported::at(
                p.line(),
                "a DOT graph starts with `graph` or `digraph`",
            ));
        },
    }
    if p.peek_id().is_some() {
        p.at += 1;
    }
    p.expect('{')?;
    p.statements(&Scope::default())?;
    Ok(p.diagram)
}

/// What a `{ }` block gives the statements in it.
#[derive(Clone, Default)]
struct Scope {
    node: Attributes,
    edge: Attributes,
    group: Option<String>,
}

impl Parser {
    fn line(&self) -> usize {
        self.tokens
            .get(self.at)
            .or(self.tokens.last())
            .map_or(0, |t| t.1)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|t| &t.0)
    }

    fn peek_id(&self) -> Option<&str> {
        match self.peek() {
            Some(Token::Id(s)) => Some(s),
            _ => None,
        }
    }

    fn next_id(&mut self) -> Option<String> {
        let id = self.peek_id().map(str::to_owned);
        if id.is_some() {
            self.at += 1;
        }
        id
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.peek() == Some(&Token::Punct(c));
        if found {
            self.at += 1;
        }
        found
    }

    fn expect(&mut self, c: char) -> Result<(), Unsupported> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(Unsupported::at(self.line(), format!("expected `{c}`")))
        }
    }

    /// Statements up to the `}` closing this block. Returns the nodes
    /// named in it, for a subgraph used as an edge's end.
    fn statements(&mut self, outer: &Scope) -> Result<Vec<String>, Unsupported> {
        self.depth += 1;
        if self.depth > super::MOST_DEPTH {
            return Err(Unsupported::at(self.line(), "subgraphs nest too deep"));
        }
        let named = self.block(outer);
        self.depth -= 1;
        named
    }

    fn block(&mut self, outer: &Scope) -> Result<Vec<String>, Unsupported> {
        let mut scope = outer.clone();
        let mut named = Vec::new();
        loop {
            if self.eat('}') {
                return Ok(named);
            }
            if self.peek().is_none() {
                return Err(Unsupported::at(self.line(), "a `{` is never closed"));
            }
            if self.eat(';') {
                continue;
            }
            let word = self.peek_id().map(str::to_ascii_lowercase);
            match word.as_deref() {
                Some("node") | Some("edge") | Some("graph")
                    if matches!(
                        self.tokens.get(self.at + 1).map(|t| &t.0),
                        Some(Token::Punct('['))
                    ) =>
                {
                    self.at += 1;
                    let given = self.attributes()?;
                    match word.as_deref() {
                        Some("node") => scope.node.extend(given),
                        Some("edge") => scope.edge.extend(given),
                        _ => self.graph_attributes(&given),
                    }
                },
                _ if matches!(
                    self.tokens.get(self.at + 1).map(|t| &t.0),
                    Some(Token::Punct('='))
                ) =>
                {
                    let key = self.next_id().unwrap_or_default();
                    self.at += 1;
                    let value = self.next_id().unwrap_or_default();
                    self.graph_attributes(&HashMap::from([(key, value)]));
                },
                _ => named.extend(self.node_or_edge(&scope)?),
            }
        }
    }

    fn graph_attributes(&mut self, given: &Attributes) {
        if let Some(dir) = given.get("rankdir") {
            self.diagram.direction = match dir.to_ascii_uppercase().as_str() {
                "LR" => Direction::Right,
                "RL" => Direction::Left,
                "BT" => Direction::Up,
                _ => Direction::Down,
            };
        }
    }

    fn attributes(&mut self) -> Result<Attributes, Unsupported> {
        let mut out = Attributes::new();
        while self.eat('[') {
            while !self.eat(']') {
                let Some(key) = self.next_id() else {
                    return Err(Unsupported::at(self.line(), "expected an attribute"));
                };
                let value = if self.eat('=') {
                    self.next_id().unwrap_or_default()
                } else {
                    "true".to_owned()
                };
                out.insert(key.to_ascii_lowercase(), value);
                let _ = self.eat(',') || self.eat(';');
            }
        }
        Ok(out)
    }

    /// One end of an edge: a node (with a port, passed over) or a
    /// subgraph's nodes.
    fn end(&mut self, scope: &Scope) -> Result<Vec<String>, Unsupported> {
        let is_subgraph = self
            .peek_id()
            .is_some_and(|w| w.eq_ignore_ascii_case("subgraph"));
        if is_subgraph || self.peek() == Some(&Token::Punct('{')) {
            if is_subgraph {
                self.at += 1;
            }
            let id = if self.peek() == Some(&Token::Punct('{')) {
                None
            } else {
                self.next_id()
            };
            self.expect('{')?;
            let mut inner = scope.clone();
            let label_at = self.at;
            if let Some(id) = id.filter(|id| id.starts_with("cluster")) {
                self.clusters += 1;
                let title = self.cluster_label(label_at).unwrap_or_else(|| {
                    id.trim_start_matches("cluster")
                        .trim_start_matches('_')
                        .to_owned()
                });
                self.diagram.groups.push(Group {
                    id: id.clone(),
                    label: title,
                    parent: scope.group.clone(),
                    tone: None,
                });
                inner.group = Some(id);
            }
            return self.statements(&inner);
        }
        let Some(id) = self.next_id() else {
            return Err(Unsupported::at(self.line(), "expected a node"));
        };
        if self.eat(':') {
            self.next_id();
            if self.eat(':') {
                self.next_id();
            }
        }
        Ok(vec![id])
    }

    /// A cluster's `label = "..."`, read ahead without moving on.
    fn cluster_label(&self, from: usize) -> Option<String> {
        let mut depth = 0;
        for w in self.tokens[from..].windows(3) {
            match &w[0].0 {
                Token::Punct('{') => depth += 1,
                Token::Punct('}') if depth == 0 => return None,
                Token::Punct('}') => depth -= 1,
                Token::Id(key) if depth == 0 && key == "label" && w[1].0 == Token::Punct('=') => {
                    if let Token::Id(value) = &w[2].0 {
                        return Some(label(value));
                    }
                },
                _ => {},
            }
        }
        None
    }

    fn node_or_edge(&mut self, scope: &Scope) -> Result<Vec<String>, Unsupported> {
        let mut ends = vec![self.end(scope)?];
        while self.peek() == Some(&Token::Link) {
            self.at += 1;
            ends.push(self.end(scope)?);
        }
        let given = self.attributes()?;
        let mut named = Vec::new();
        if ends.len() == 1 {
            for id in &ends[0] {
                let mut attributes = scope.node.clone();
                attributes.extend(given.clone());
                self.node(id, &attributes, scope, true);
                named.push(id.clone());
            }
            return Ok(named);
        }
        let mut attributes = scope.edge.clone();
        attributes.extend(given);
        let more: usize = ends
            .windows(2)
            .map(|pair| pair[0].len() * pair[1].len())
            .sum();
        if self.diagram.edges.len() + more > super::MOST_EDGES {
            return Err(Unsupported::too_big(self.line()));
        }
        for pair in ends.windows(2) {
            for a in &pair[0] {
                for b in &pair[1] {
                    self.node(a, &scope.node, scope, false);
                    self.node(b, &scope.node, scope, false);
                    self.edge(a, b, &attributes);
                }
            }
        }
        Ok(ends.concat())
    }

    /// The node `id`; `declared` when this statement is its own (so its
    /// attributes count), else it is only made if new.
    fn node(&mut self, id: &str, attributes: &Attributes, scope: &Scope, declared: bool) {
        let index = match self.nodes.get(id) {
            Some(_) if !declared => return,
            Some(&i) => i,
            None if self.diagram.nodes.len() >= super::MOST_NODES => return,
            None => {
                let mut node = Node::new(id, id);
                node.kind = NodeKind::Pill;
                node.group = scope.group.clone();
                self.diagram.nodes.push(node);
                self.nodes
                    .insert(id.to_owned(), self.diagram.nodes.len() - 1);
                self.diagram.nodes.len() - 1
            },
        };
        let node = &mut self.diagram.nodes[index];
        if let Some(text) = attributes.get("label") {
            node.label = if text == "\\N" {
                id.to_owned()
            } else {
                label(text)
            };
        }
        let style = attributes
            .get("style")
            .map(String::as_str)
            .unwrap_or_default();
        if let Some(shape) = attributes.get("shape") {
            node.kind = match shape.to_ascii_lowercase().as_str() {
                "box" | "rect" | "rectangle" | "square" | "record" | "component" => {
                    if style.contains("rounded") {
                        NodeKind::Rounded
                    } else {
                        NodeKind::Box
                    }
                },
                "mrecord" | "hexagon" | "octagon" => NodeKind::Rounded,
                "circle" | "doublecircle" | "point" => NodeKind::Circle,
                "diamond" | "mdiamond" => NodeKind::Diamond,
                "cylinder" => NodeKind::Cylinder,
                "note" | "tab" | "folder" => NodeKind::Note,
                "plaintext" | "plain" | "none" => NodeKind::Text,
                _ => NodeKind::Pill,
            };
        }
        let color = attributes.get("fillcolor").or_else(|| {
            style
                .contains("filled")
                .then(|| attributes.get("color"))
                .flatten()
        });
        if let Some(color) = color.filter(|c| c.starts_with('#') && matches!(c.len(), 4 | 7)) {
            node.fill = Some(color.clone());
        }
    }

    fn edge(&mut self, from: &str, to: &str, attributes: &Attributes) {
        let style = attributes
            .get("style")
            .map(String::as_str)
            .unwrap_or_default();
        let kind = if style.contains("invis") {
            EdgeKind::Invisible
        } else if style.contains("dashed") {
            EdgeKind::Dashed
        } else if style.contains("dotted") {
            EdgeKind::Dotted
        } else if style.contains("bold") {
            EdgeKind::Thick
        } else {
            EdgeKind::Solid
        };
        let head = |name: Option<&String>, default: Head| match name
            .map(|n| n.to_ascii_lowercase())
            .as_deref()
        {
            Some("none") => Head::None,
            Some("empty" | "onormal") => Head::Triangle,
            Some("diamond") => Head::Diamond,
            Some("odiamond") => Head::HollowDiamond,
            Some("dot" | "odot") => Head::Circle,
            Some("open" | "vee") => Head::Open,
            Some(_) => Head::Arrow,
            None => default,
        };
        let (start, end) = match attributes.get("dir").map(String::as_str) {
            Some("both") => (Head::Arrow, Head::Arrow),
            Some("back") => (Head::Arrow, Head::None),
            Some("none") => (Head::None, Head::None),
            Some(_) => (Head::None, Head::Arrow),
            None if self.directed => (Head::None, Head::Arrow),
            None => (Head::None, Head::None),
        };
        let (start, end) = (
            head(
                attributes.get("arrowtail").filter(|_| start != Head::None),
                start,
            ),
            head(
                attributes.get("arrowhead").filter(|_| end != Head::None),
                end,
            ),
        );
        let id = format!("e{}", self.diagram.edges.len());
        self.diagram.edges.push(Edge {
            label: attributes
                .get("label")
                .map(|l| label(l))
                .filter(|l| !l.is_empty()),
            kind,
            start,
            end,
            ..Edge::new(id, from, to)
        });
    }
}
