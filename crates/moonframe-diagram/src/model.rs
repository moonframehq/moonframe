//! What a diagram says, with no say in how it looks: nodes and the edges
//! between them, groups around nodes, and for a sequence diagram the
//! messages between participants in order. Mermaid, DOT, an agent's graph
//! and an editor's document all become this.

use crate::geometry::Rect;

/// A whole diagram.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagram {
    pub kind: DiagramKind,
    /// Which way a flow's ranks run. A sequence diagram ignores it.
    pub direction: Direction,
    /// In a flow, its boxes; in a sequence diagram, its participants, left
    /// to right.
    pub nodes: Vec<Node>,
    /// A flow's edges.
    pub edges: Vec<Edge>,
    pub groups: Vec<Group>,
    /// A sequence diagram's messages, top to bottom.
    pub messages: Vec<Message>,
    /// A sequence diagram's notes.
    pub notes: Vec<Note>,
}

impl Diagram {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn edge(&self, id: &str) -> Option<&Edge> {
        self.edges.iter().find(|edge| edge.id == id)
    }

    /// A sequence diagram's participants, left to right: its nodes, then
    /// any a message names that no node is, labelled with its id.
    pub fn participants(&self) -> Vec<Node> {
        let mut people = self.nodes.clone();
        for m in &self.messages {
            for id in [&m.from, &m.to] {
                if !people.iter().any(|p| &p.id == id) {
                    people.push(Node::new(id.clone(), id.clone()));
                }
            }
        }
        people
    }

    /// Whether every node has a place of its own: an editor's document,
    /// whose nodes are never moved by a layout.
    pub fn placed(&self) -> bool {
        !self.nodes.is_empty() && self.nodes.iter().all(|node| node.at.is_some())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiagramKind {
    // keep-sorted start
    /// Boxes and arrows: flowcharts, state and class diagrams, graphs.
    #[default]
    Flow,
    /// Participants side by side and the messages between them in order.
    Sequence,
    // keep-sorted end
}

/// Which way a flow's ranks run: `Down` puts each rank under the last.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    // keep-sorted start
    #[default]
    Down,
    Left,
    Right,
    Up,
    // keep-sorted end
}

impl Direction {
    /// Whether ranks run along the y axis.
    pub fn vertical(self) -> bool {
        matches!(self, Self::Down | Self::Up)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    /// A second, quieter line under the label.
    pub sublabel: Option<String>,
    pub kind: NodeKind,
    /// An icon name the view may draw beside the label.
    pub icon: Option<String>,
    pub tone: Option<Tone>,
    /// A color of its own, `#rrggbb`, over the tone: an editor's fill.
    pub fill: Option<String>,
    /// The group it sits in.
    pub group: Option<String>,
    /// Its place and size, when the diagram's author chose them (an
    /// editor); a layout keeps it.
    pub at: Option<Rect>,
}

impl Node {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            ..Self::default()
        }
    }
}

/// Declares [`NodeKind`] once: each kind with its name and the other names
/// it is read from, generating the enum, `ALL`, `name` and `from_name`.
macro_rules! node_kinds {
    ($($(#[$attr:meta])* $kind:ident = $name:literal $(| $alias:literal)*,)+) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub enum NodeKind {
            $($(#[$attr])* $kind,)+
        }

        impl NodeKind {
            /// Every kind, in a picker's order.
            pub const ALL: &[Self] = &[$(Self::$kind,)+];

            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$kind => $name,)+
                }
            }

            /// The kind a name gives, also the Luau editor's names
            /// (`ellipse`, `sticky`) and Mermaid's (`stadium`, `rhombus`).
            pub fn from_name(name: &str) -> Option<Self> {
                Some(match name {
                    $($name $(| $alias)* => Self::$kind,)+
                    _ => return None,
                })
            }
        }
    };
}

// In a picker's order, which users see: not sorted.
node_kinds! {
    #[default]
    Box = "box" | "rect",
    Rounded = "rounded",
    Pill = "pill" | "stadium",
    Diamond = "diamond" | "decision" | "rhombus",
    Circle = "circle" | "ellipse",
    Cylinder = "cylinder" | "database",
    Note = "note" | "sticky",
    /// Words alone, no outline.
    Text = "text",
}

/// A meaning a node or edge carries, drawn in the theme's color for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    // keep-sorted start
    Accent,
    Danger,
    Muted,
    Success,
    Warning,
    // keep-sorted end
}

impl Tone {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "accent" | "info" => Self::Accent,
            "success" | "ok" => Self::Success,
            "warning" => Self::Warning,
            "danger" | "error" => Self::Danger,
            "muted" => Self::Muted,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub kind: EdgeKind,
    /// What its start and end look like.
    pub start: Head,
    pub end: Head,
    pub tone: Option<Tone>,
}

impl Edge {
    pub fn new(id: impl Into<String>, from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            end: Head::Arrow,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EdgeKind {
    // keep-sorted start
    Dashed,
    Dotted,
    /// Laid out as an edge, drawn as nothing: keeps two nodes near.
    Invisible,
    #[default]
    Solid,
    Thick,
    // keep-sorted end
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Head {
    // keep-sorted start
    Arrow,
    Circle,
    Cross,
    /// A filled diamond: composition.
    Diamond,
    /// A hollow diamond: aggregation.
    HollowDiamond,
    #[default]
    None,
    /// An open arrow, as a reply or an async message.
    Open,
    /// A hollow triangle: inheritance.
    Triangle,
    // keep-sorted end
}

/// A box drawn around nodes, maybe inside another group.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub id: String,
    pub label: String,
    pub parent: Option<String>,
    pub tone: Option<Tone>,
}

/// One message of a sequence diagram, from one participant to another (or
/// itself).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub label: String,
    pub kind: MessageKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageKind {
    // keep-sorted start
    /// A solid line, an open head.
    Async,
    /// A return: a dashed line.
    Reply,
    /// A call: a solid line, a filled head.
    #[default]
    Sync,
    // keep-sorted end
}

/// A note in a sequence diagram, over one or more participants, before the
/// message at `before` (the number of messages above it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Note {
    pub over: Vec<String>,
    pub text: String,
    pub before: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_is_read_back_from_its_name() {
        for &kind in NodeKind::ALL {
            assert_eq!(NodeKind::from_name(kind.name()), Some(kind));
        }
        assert_eq!(NodeKind::from_name("ellipse"), Some(NodeKind::Circle));
        assert_eq!(NodeKind::from_name("hexagon"), None);
    }
}
