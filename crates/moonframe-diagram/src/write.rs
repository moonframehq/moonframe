//! The model back to Mermaid, for "Copy as Mermaid" and for sending an
//! edited diagram to an agent: a flow as a `flowchart` (shapes, edge
//! kinds and heads, labels, subgraphs, fills as `style`, tones as
//! classes), a sequence as a `sequenceDiagram`. A flow whose edges carry
//! UML heads (inheritance, composition, aggregation) and that a class
//! diagram holds whole, with no groups and only plain boxes, is written
//! as a `classDiagram`, since a flowchart has no marks for those heads;
//! any other flow draws them as arrows. Read back with
//! [`crate::parse::mermaid`], it gives the same diagram, but for places
//! and a sublabel joined to its label.

use {
    crate::model::{
        Diagram, DiagramKind, Direction, EdgeKind, Head, MessageKind, Node, NodeKind, Tone,
    },
    std::collections::{HashMap, HashSet},
};

pub fn mermaid(d: &Diagram) -> String {
    match d.kind {
        DiagramKind::Flow if is_class_diagram(d) => class_diagram(d),
        DiagramKind::Flow => flowchart(d),
        DiagramKind::Sequence => sequence(d),
    }
}

/// Whether `d` has heads only a class diagram can mark, and nothing a
/// class diagram would drop.
fn is_class_diagram(d: &Diagram) -> bool {
    let uml = |h: Head| matches!(h, Head::Triangle | Head::Diamond | Head::HollowDiamond);
    d.edges.iter().any(|e| uml(e.start) || uml(e.end))
        && d.groups.is_empty()
        && d.nodes
            .iter()
            .all(|n| n.kind == NodeKind::Box && n.fill.is_none() && n.tone.is_none())
}

/// Each node's and group's Mermaid id, no two alike: names that clean up
/// the same (`api-gw` and `api_gw`, `end` and `end_`) get a number.
#[derive(Default)]
struct Ids {
    by_name: HashMap<String, String>,
    taken: HashSet<String>,
}

impl Ids {
    fn new(d: &Diagram) -> Self {
        let mut ids = Self::default();
        for n in &d.nodes {
            ids.name(&n.id);
        }
        ids
    }

    /// The id for `name`, made the first time it is asked for.
    fn name(&mut self, name: &str) -> String {
        if let Some(id) = self.by_name.get(name) {
            return id.clone();
        }
        let id = self.fresh(name);
        self.by_name.insert(name.to_owned(), id.clone());
        id
    }

    /// An id for `name` that no other has.
    fn fresh(&mut self, name: &str) -> String {
        let base = id(name);
        let mut out = base.clone();
        let mut n = 2;
        while !self.taken.insert(out.clone()) {
            out = format!("{base}_{n}");
            n += 1;
        }
        out
    }
}

/// An id Mermaid reads: letters, digits and `_`, else made so.
fn id(text: &str) -> String {
    let clean: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    match clean.chars().next() {
        None => "_".to_owned(),
        Some(c) if c.is_numeric() => format!("n{clean}"),
        // `end` closes a subgraph: never an id.
        _ if clean == "end" => "end_".to_owned(),
        _ => clean,
    }
}

/// A label, quoted, read back as written.
fn quoted(text: &str) -> String {
    format!("\"{}\"", escaped(text))
}

/// Text the parser reads back as written: what it would take for the end
/// of the quotes, a tag, an entity, markdown or a line's end is spelled
/// as an entity.
fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' => out.push_str("#quot;"),
            '<' => out.push_str("#lt;"),
            '>' => out.push_str("#gt;"),
            '#' | '*' | '\\' | '`' | '\n' | '\r' => out.push_str(&format!("#{};", u32::from(c))),
            c => out.push(c),
        }
    }
    out
}

fn node(n: &Node, id: &str) -> String {
    let (open, close) = match n.kind {
        NodeKind::Box | NodeKind::Text => ("[", "]"),
        NodeKind::Rounded => ("(", ")"),
        NodeKind::Pill => ("([", "])"),
        NodeKind::Diamond => ("{", "}"),
        NodeKind::Circle => ("((", "))"),
        NodeKind::Cylinder => ("[(", ")]"),
        NodeKind::Note => (">", "]"),
    };
    let mut text = n.label.clone();
    if let Some(sub) = &n.sublabel {
        text = format!("{text}<br/>{sub}");
    }
    format!("{id}{open}{}{close}", quoted(&text))
}

/// The class a tone is read back from, and a stroke for it in Mermaid.
fn tone_class(tone: Tone) -> (&'static str, &'static str) {
    match tone {
        Tone::Accent => ("accent", "#2563eb"),
        Tone::Success => ("success", "#059669"),
        Tone::Warning => ("warning", "#d97706"),
        Tone::Danger => ("danger", "#dc2626"),
        Tone::Muted => ("muted", "#6b7280"),
    }
}

fn direction(d: &Diagram) -> &'static str {
    match d.direction {
        Direction::Down => "TD",
        Direction::Up => "BT",
        Direction::Right => "LR",
        Direction::Left => "RL",
    }
}

/// Groups as subgraphs, each with its nodes and the groups inside it.
struct Subgraphs<'a> {
    d: &'a Diagram,
    /// Each group's parent as an index: the first group of that id.
    parents: Vec<Option<usize>>,
    /// Each node's group as an index; `None` at the top level, also when
    /// the group is unknown.
    homes: Vec<Option<usize>>,
    /// Groups already written: a parent loop or repeated id is written
    /// once, not followed forever.
    written: Vec<bool>,
    ids: Vec<String>,
}

impl<'a> Subgraphs<'a> {
    fn new(d: &'a Diagram, ids: &mut Ids) -> Self {
        let mut first: HashMap<&str, usize> = HashMap::new();
        for (i, g) in d.groups.iter().enumerate() {
            first.entry(g.id.as_str()).or_insert(i);
        }
        let index = |name: &Option<String>| name.as_deref().and_then(|g| first.get(g).copied());
        Self {
            d,
            parents: d.groups.iter().map(|g| index(&g.parent)).collect(),
            homes: d.nodes.iter().map(|n| index(&n.group)).collect(),
            written: vec![false; d.groups.len()],
            ids: d.groups.iter().map(|g| ids.fresh(&g.id)).collect(),
        }
    }

    /// Everything in `parent` (the top level for `None`), then any group
    /// a loop of parents kept out of reach, at the top level.
    fn write(&mut self, nodes: &[String], out: &mut String) {
        self.inside(None, nodes, 1, out);
        while let Some(g) = self.written.iter().position(|w| !w) {
            self.group(g, nodes, 1, out);
        }
    }

    fn inside(&mut self, parent: Option<usize>, nodes: &[String], depth: usize, out: &mut String) {
        let pad = "    ".repeat(depth);
        for (i, n) in self.d.nodes.iter().enumerate() {
            if self.homes[i] == parent {
                out.push_str(&format!("{pad}{}\n", node(n, &nodes[i])));
            }
        }
        for g in 0..self.d.groups.len() {
            if !self.written[g] && self.parents[g] == parent {
                self.group(g, nodes, depth, out);
            }
        }
    }

    fn group(&mut self, g: usize, nodes: &[String], depth: usize, out: &mut String) {
        self.written[g] = true;
        let pad = "    ".repeat(depth);
        out.push_str(&format!(
            "{pad}subgraph {}[{}]\n",
            self.ids[g],
            quoted(&self.d.groups[g].label)
        ));
        self.inside(Some(g), nodes, depth + 1, out);
        out.push_str(&format!("{pad}end\n"));
    }
}

fn flowchart(d: &Diagram) -> String {
    let mut out = format!("flowchart {}\n", direction(d));
    let mut ids = Ids::new(d);
    let nodes: Vec<String> = d.nodes.iter().map(|n| ids.name(&n.id)).collect();
    Subgraphs::new(d, &mut ids).write(&nodes, &mut out);
    for e in &d.edges {
        let line = match e.kind {
            EdgeKind::Solid => "--",
            EdgeKind::Dashed | EdgeKind::Dotted => "-.-",
            EdgeKind::Thick => "==",
            EdgeKind::Invisible => "~~~",
        };
        let arrow = if e.kind == EdgeKind::Invisible {
            line.to_owned()
        } else {
            let start = match e.start {
                Head::None => "",
                Head::Circle => "o",
                Head::Cross => "x",
                _ => "<",
            };
            let end = match e.end {
                Head::None if e.kind == EdgeKind::Thick => "=",
                Head::None if e.kind == EdgeKind::Solid => "-",
                Head::None => "",
                Head::Circle => "o",
                Head::Cross => "x",
                _ => ">",
            };
            format!("{start}{line}{end}")
        };
        let label = e
            .label
            .as_deref()
            .filter(|l| !l.is_empty())
            .map(|l| format!("|{}|", quoted(l)))
            .unwrap_or_default();
        out.push_str(&format!(
            "    {} {arrow}{label} {}\n",
            ids.name(&e.from),
            ids.name(&e.to)
        ));
    }
    for (n, id) in d.nodes.iter().zip(&nodes) {
        if let Some(fill) = &n.fill {
            out.push_str(&format!("    style {id} fill:{fill}\n"));
        }
    }
    for tone in [
        Tone::Accent,
        Tone::Success,
        Tone::Warning,
        Tone::Danger,
        Tone::Muted,
    ] {
        let toned: Vec<&str> = d
            .nodes
            .iter()
            .zip(&nodes)
            .filter(|(n, _)| n.tone == Some(tone))
            .map(|(_, id)| id.as_str())
            .collect();
        if !toned.is_empty() {
            let (class, stroke) = tone_class(tone);
            out.push_str(&format!("    classDef {class} stroke:{stroke}\n"));
            out.push_str(&format!("    class {} {class}\n", toned.join(",")));
        }
    }
    out
}

/// A flow of plain boxes with UML heads: classes, their members (the
/// sublabel), and relations marked at either end.
fn class_diagram(d: &Diagram) -> String {
    // A class diagram says `TB` where a flowchart says `TD`.
    let dir = match d.direction {
        Direction::Down => "TB",
        _ => direction(d),
    };
    let mut out = format!("classDiagram\n    direction {dir}\n");
    let mut ids = Ids::new(d);
    for n in &d.nodes {
        let id = ids.name(&n.id);
        if n.label == n.id {
            out.push_str(&format!("    class {id}\n"));
        } else {
            out.push_str(&format!("    class {id}[{}]\n", quoted(&n.label)));
        }
        for member in n.sublabel.iter().flat_map(|s| s.split(", ")) {
            out.push_str(&format!("    {id} : {member}\n"));
        }
    }
    for e in &d.edges {
        let start = match e.start {
            Head::None => "",
            Head::Triangle => "<|",
            Head::Diamond => "*",
            Head::HollowDiamond => "o",
            _ => "<",
        };
        let end = match e.end {
            Head::None => "",
            Head::Triangle => "|>",
            Head::Diamond => "*",
            Head::HollowDiamond => "o",
            _ => ">",
        };
        let line = match e.kind {
            EdgeKind::Dashed | EdgeKind::Dotted | EdgeKind::Invisible => "..",
            EdgeKind::Solid | EdgeKind::Thick => "--",
        };
        let label = e
            .label
            .as_deref()
            .filter(|l| !l.is_empty())
            .map(|l| format!(" : {}", escaped(l)))
            .unwrap_or_default();
        out.push_str(&format!(
            "    {} {start}{line}{end} {}{label}\n",
            ids.name(&e.from),
            ids.name(&e.to)
        ));
    }
    out
}

fn sequence(d: &Diagram) -> String {
    let mut out = "sequenceDiagram\n".to_owned();
    let mut ids = Ids::new(d);
    let mut id = |name: &str| ids.name(name);
    for n in &d.nodes {
        let word = if n.kind == NodeKind::Pill {
            "actor"
        } else {
            "participant"
        };
        let written = id(&n.id);
        if n.label == written {
            out.push_str(&format!("    {word} {written}\n"));
        } else {
            out.push_str(&format!("    {word} {written} as {}\n", escaped(&n.label)));
        }
    }
    let mut notes = d.notes.iter().peekable();
    for (i, m) in d.messages.iter().enumerate() {
        while let Some(note) = notes.next_if(|n| n.before <= i) {
            let over: Vec<String> = note.over.iter().map(|o| id(o)).collect();
            out.push_str(&format!(
                "    Note over {}: {}\n",
                over.join(","),
                escaped(&note.text)
            ));
        }
        let arrow = match m.kind {
            MessageKind::Sync => "->>",
            MessageKind::Async => "-)",
            MessageKind::Reply => "-->>",
        };
        out.push_str(&format!(
            "    {}{arrow}{}: {}\n",
            id(&m.from),
            id(&m.to),
            escaped(&m.label)
        ));
    }
    for note in notes {
        let over: Vec<String> = note.over.iter().map(|o| id(o)).collect();
        out.push_str(&format!(
            "    Note over {}: {}\n",
            over.join(","),
            escaped(&note.text)
        ));
    }
    out
}
