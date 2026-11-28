//! The props a mod gives `ui.diagram`, read into the diagram model. Bad
//! values are left out rather than refused: a shape with no `id` is not
//! drawn, an unknown kind is a box.

use {
    moonframe_core::Data,
    moonframe_diagram::{
        Diagram, DiagramKind, Direction, Edge, EdgeKind, Group, Head, Message, MessageKind, Node,
        NodeKind, Note, Rect, Tone,
        view::{Mode, Tool},
    },
};

pub(super) enum Camera {
    Fit,
    ZoomBy(f32),
    ZoomTo(f32),
}

pub(super) struct Props {
    pub diagram: Diagram,
    /// The props the diagram was read from, to tell when it changed.
    pub document: Data,
    pub mode: Mode,
    pub tool: Tool,
    pub selected: Vec<String>,
    pub highlight: Vec<String>,
    /// A camera request and the `seq` that asks for it.
    pub camera: Option<(Data, Camera)>,
    pub scroll_needs_focus: bool,
}

fn text(data: &Data, key: &str) -> Option<String> {
    match data.get(key)? {
        Data::String(s) => Some(s.clone()),
        Data::Integer(n) => Some(n.to_string()),
        Data::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn number(data: &Data, key: &str) -> Option<f32> {
    data.get(key).and_then(Data::as_f64).map(|n| n as f32)
}

fn list<'a>(data: &'a Data, key: &str) -> &'a [Data] {
    match data.get(key) {
        Some(Data::List(items)) => items,
        _ => &[],
    }
}

fn strings(data: &Data, key: &str) -> Vec<String> {
    list(data, key)
        .iter()
        .filter_map(|d| d.as_str().map(str::to_owned))
        .collect()
}

fn place(data: &Data) -> Option<Rect> {
    Some(Rect::new(
        number(data, "x")?,
        number(data, "y")?,
        number(data, "w")?,
        number(data, "h")?,
    ))
}

fn node(data: &Data) -> Option<Node> {
    let id = text(data, "id")?;
    let label = text(data, "label").unwrap_or_default();
    Some(Node {
        id,
        label,
        sublabel: text(data, "sublabel"),
        kind: text(data, "kind")
            .and_then(|k| NodeKind::from_name(&k))
            .unwrap_or_default(),
        icon: text(data, "icon"),
        tone: text(data, "tone").and_then(|t| Tone::from_name(&t)),
        fill: text(data, "fill"),
        group: text(data, "group"),
        at: place(data),
    })
}

fn head(name: Option<String>, default: Head) -> Head {
    match name.as_deref() {
        Some("none") => Head::None,
        Some("arrow") => Head::Arrow,
        Some("open") => Head::Open,
        Some("circle") => Head::Circle,
        Some("cross") => Head::Cross,
        Some("triangle") => Head::Triangle,
        Some("diamond") => Head::Diamond,
        Some("hollow-diamond") => Head::HollowDiamond,
        _ => default,
    }
}

fn edge(data: &Data, index: usize) -> Option<Edge> {
    let from = text(data, "from")?;
    let to = text(data, "to")?;
    let dashed = data
        .get("dashed")
        .is_some_and(|d| matches!(d, Data::Bool(true)));
    let kind = match text(data, "kind").as_deref() {
        Some("dashed") => EdgeKind::Dashed,
        Some("dotted") => EdgeKind::Dotted,
        Some("thick") => EdgeKind::Thick,
        Some("invisible") => EdgeKind::Invisible,
        _ if dashed => EdgeKind::Dashed,
        _ => EdgeKind::Solid,
    };
    Some(Edge {
        id: text(data, "id").unwrap_or_else(|| format!("e{index}")),
        from,
        to,
        label: text(data, "label"),
        kind,
        start: head(text(data, "start"), Head::None),
        end: head(text(data, "end"), Head::Arrow),
        tone: text(data, "tone").and_then(|t| Tone::from_name(&t)),
    })
}

/// The document: `shapes` and `links` (the Luau editor's), else `document`.
fn diagram(props: &Data) -> (Diagram, Data) {
    if props.get("shapes").is_some() {
        let source = Data::map([
            ("shapes", props.get("shapes").cloned().unwrap_or_default()),
            ("links", props.get("links").cloned().unwrap_or_default()),
        ]);
        let diagram = Diagram {
            nodes: list(props, "shapes").iter().filter_map(node).collect(),
            edges: list(props, "links")
                .iter()
                .enumerate()
                .filter_map(|(i, d)| edge(d, i))
                .collect(),
            ..Diagram::default()
        };
        return (diagram, source);
    }
    let Some(doc) = props.get("document") else {
        return (Diagram::default(), Data::Nil);
    };
    let diagram = Diagram {
        kind: match text(doc, "kind").as_deref() {
            Some("sequence") => DiagramKind::Sequence,
            _ => DiagramKind::Flow,
        },
        direction: match text(doc, "direction").as_deref() {
            Some("up") => Direction::Up,
            Some("right") => Direction::Right,
            Some("left") => Direction::Left,
            _ => Direction::Down,
        },
        nodes: list(doc, "nodes").iter().filter_map(node).collect(),
        edges: list(doc, "edges")
            .iter()
            .enumerate()
            .filter_map(|(i, d)| edge(d, i))
            .collect(),
        groups: list(doc, "groups")
            .iter()
            .filter_map(|g| {
                Some(Group {
                    id: text(g, "id")?,
                    label: text(g, "label").unwrap_or_default(),
                    parent: text(g, "parent"),
                    tone: text(g, "tone").and_then(|t| Tone::from_name(&t)),
                })
            })
            .collect(),
        messages: list(doc, "messages")
            .iter()
            .filter_map(|m| {
                Some(Message {
                    from: text(m, "from")?,
                    to: text(m, "to")?,
                    label: text(m, "label").unwrap_or_default(),
                    kind: match text(m, "kind").as_deref() {
                        Some("async") => MessageKind::Async,
                        Some("reply") => MessageKind::Reply,
                        _ => MessageKind::Sync,
                    },
                })
            })
            .collect(),
        notes: list(doc, "notes")
            .iter()
            .filter_map(|n| {
                Some(Note {
                    over: strings(n, "over"),
                    text: text(n, "text")?,
                    before: number(n, "before").map_or(usize::MAX, |b| b.max(0.0) as usize),
                })
            })
            .collect(),
    };
    (diagram, doc.clone())
}

pub(super) fn props(props: &Data) -> Props {
    let (diagram, document) = diagram(props);
    let editor = props.get("shapes").is_some();
    let mode = match text(props, "mode").as_deref() {
        Some("edit") => Mode::Edit,
        Some("view") => Mode::View,
        _ if editor => Mode::Edit,
        _ => Mode::View,
    };
    let tool = match text(props, "tool").as_deref() {
        None | Some("select") => Tool::Select,
        Some("hand") => Tool::Hand,
        Some("connector" | "connect") => Tool::Connect,
        Some(kind) => NodeKind::from_name(kind).map_or(Tool::Select, Tool::Create),
    };
    let camera = props.get("camera").and_then(|c| {
        let seq = c.get("seq").cloned().unwrap_or(Data::Nil);
        let ask = if matches!(c.get("fit"), Some(Data::Bool(true))) {
            Camera::Fit
        } else if let Some(by) = number(c, "zoom_by") {
            Camera::ZoomBy(by)
        } else {
            Camera::ZoomTo(number(c, "zoom")?)
        };
        Some((seq, ask))
    });
    Props {
        diagram,
        document,
        mode,
        tool,
        selected: strings(props, "selected"),
        highlight: strings(props, "highlight"),
        camera,
        scroll_needs_focus: text(props, "scroll").as_deref() == Some("focus"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn shape(id: &str, kind: &str) -> Data {
        Data::map([
            ("id", Data::String(id.into())),
            ("kind", Data::String(kind.into())),
            ("x", Data::Integer(0)),
            ("y", Data::Integer(0)),
            ("w", Data::Integer(100)),
            ("h", Data::Integer(40)),
        ])
    }

    #[test]
    fn the_luau_editors_document_reads_placed_and_editable() {
        let given = Data::map([
            (
                "shapes",
                Data::List(vec![shape("a", "sticky"), shape("b", "ellipse")]),
            ),
            (
                "links",
                Data::List(vec![Data::map([
                    ("id", Data::String("l".into())),
                    ("from", Data::String("a".into())),
                    ("to", Data::String("b".into())),
                    ("dashed", Data::Bool(true)),
                ])]),
            ),
            ("tool", Data::String("ellipse".into())),
        ]);
        let read = props(&given);
        assert!(read.diagram.placed());
        assert_eq!(read.diagram.nodes[0].kind, NodeKind::Note);
        assert_eq!(read.diagram.nodes[1].kind, NodeKind::Circle);
        assert_eq!(read.diagram.edges[0].kind, EdgeKind::Dashed);
        assert_eq!(read.mode, Mode::Edit);
        assert_eq!(read.tool, Tool::Create(NodeKind::Circle));
    }

    #[test]
    fn a_document_without_places_is_laid_out_and_only_shown() {
        let doc = Data::map([
            ("direction", Data::String("right".into())),
            (
                "nodes",
                Data::List(vec![Data::map([("id", Data::String("a".into()))])]),
            ),
        ]);
        let read = props(&Data::map([("document", doc)]));
        assert_eq!(read.diagram.direction, Direction::Right);
        assert!(!read.diagram.placed());
        assert_eq!(read.mode, Mode::View);
    }
}
