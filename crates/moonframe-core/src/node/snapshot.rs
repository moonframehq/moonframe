//! A drawn view as plain data, for a host or an agent to inspect what a
//! mod shows without the window: each element's kind, path, line, classes,
//! text and the sizes it asked for, children nested, within a byte budget.

use {super::*, crate::Data};

/// The most bytes a snapshot's tree takes, roughly: past it, children are
/// left out and `truncated` is set.
pub const MAX_SNAPSHOT: usize = 256 * 1024;

/// What a view drew, as [`Node::snapshot`] gives it.
pub struct Snapshot {
    pub tree: Data,
    pub truncated: bool,
}

impl Node {
    /// This tree as data: `{ kind, path, line, class, text, width, height,
    /// children, ... }` per element, paths as diagnostics give them
    /// (`column[1].row`), at most [`MAX_SNAPSHOT`] bytes or so.
    pub fn snapshot(&self) -> Snapshot {
        let mut budget = Budget {
            left: MAX_SNAPSHOT,
            truncated: false,
        };
        let tree = self.data("", &mut budget);
        Snapshot {
            tree,
            truncated: budget.truncated,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Stack(stack) => stack.direction.name(),
            Self::Panel(_) => "panel",
            Self::Split(split) => match split.direction {
                Direction::Row => "hsplit",
                Direction::Column => "vsplit",
            },
            Self::Text(_) => "text",
            Self::Button(_) => "button",
            Self::Item(_) => "item",
            Self::Badge(_) => "badge",
            Self::Icon(_) => "icon",
            Self::Image(_) => "image",
            Self::Line(_) => "line",
            Self::Paths(_) => "paths",
            Self::RevisionTimeline(_) => "revision_timeline",
            Self::Hatch(_) => "hatch",
            Self::Map(_) => "map",
            Self::Terminal(_) => "terminal",
            Self::Browser(_) => "browser",
            Self::Video(_) => "video",
            Self::View(_) => "view",
            Self::Input(_) => "input",
            Self::Markdown(_) => "markdown",
            Self::Divider => "divider",
            Self::Spacer => "spacer",
            Self::Region(_) => "region",
            Self::Mount(_) => "mount",
            Self::Instance(_) => "instance",
            Self::Popover(_) => "popover",
            Self::Overlay(_) => "overlay",
            Self::List(_) => "list",
            Self::Problem(_) => "problem",
        }
    }

    fn look(&self) -> Option<&Look> {
        Some(match self {
            Self::Stack(node) => &node.look,
            Self::Panel(node) => &node.look,
            Self::Text(node) => &node.look,
            Self::Button(node) => &node.look,
            Self::Item(node) => &node.look,
            Self::Badge(node) => &node.look,
            Self::Icon(node) => &node.look,
            Self::Image(node) => &node.look,
            Self::Input(node) => &node.look,
            Self::Markdown(node) => &node.look,
            Self::Popover(node) => &node.look,
            Self::Overlay(node) => &node.look,
            Self::List(node) => &node.look,
            _ => return None,
        })
    }

    /// The text it shows, and the sizes it asked for.
    fn fields(&self, fields: &mut Vec<(&'static str, Data)>) {
        let text = |text: &str| Data::from(text);
        let size =
            |fields: &mut Vec<(&'static str, Data)>, width: Option<f32>, height: Option<f32>| {
                if let Some(width) = width {
                    fields.push(("width", Data::Number(f64::from(width))));
                }
                if let Some(height) = height {
                    fields.push(("height", Data::Number(f64::from(height))));
                }
            };
        match self {
            Self::Stack(stack) => size(fields, stack.frame.width, stack.frame.height),
            Self::Panel(panel) => {
                if let Some(title) = &panel.title {
                    fields.push(("text", text(title)));
                }
                size(fields, panel.frame.width, panel.frame.height);
            },
            Self::Text(node) => fields.push(("text", text(&node.text))),
            Self::Button(node) => {
                fields.push(("text", text(&node.label)));
                fields.push(("variant", text(node.variant.name())));
                if let Some(icon) = &node.icon {
                    fields.push(("icon", text(icon.name())));
                }
            },
            Self::Item(node) => {
                fields.push(("text", text(&node.label)));
                fields.push(("selected", Data::Bool(node.selected)));
            },
            Self::Badge(node) => fields.push(("text", text(&node.text))),
            Self::Icon(node) => fields.push(("icon", text(node.name.name()))),
            Self::Image(node) => {
                fields.push(("src", text(&node.src)));
                size(fields, node.width, node.height);
            },
            Self::Input(node) => {
                if let Some(value) = &node.value {
                    fields.push(("text", text(value)));
                }
            },
            Self::Markdown(node) => {
                fields.push(("text", text(&node.text)));
                size(fields, node.width, node.height);
            },
            Self::View(view) => fields.push(("name", text(&view.name))),
            Self::Region(region) => fields.push(("name", text(&region.name))),
            Self::Mount(name) => fields.push(("name", text(name))),
            Self::Instance(instance) => fields.push(("id", text(&instance.id))),
            Self::Popover(node) => fields.push(("open", Data::Bool(node.open))),
            Self::Overlay(node) => fields.push(("open", Data::Bool(node.open))),
            Self::List(list) => fields.push(("count", Data::Integer(list.count as i64))),
            Self::Problem(message) => fields.push(("problem", text(message))),
            _ => {},
        }
    }

    /// Its children, each under the key that holds it: `""` for the list
    /// part, `actions`, `anchor`, `content`.
    fn parts(&self) -> Vec<(&'static str, Vec<&Node>)> {
        fn all(nodes: &[Node]) -> Vec<&Node> {
            nodes.iter().collect()
        }
        match self {
            Self::Stack(stack) => vec![("", all(&stack.children))],
            Self::Panel(panel) => {
                vec![("actions", all(&panel.actions)), ("", all(&panel.children))]
            },
            Self::Split(split) => vec![(
                "",
                split
                    .panes
                    .iter()
                    .flat_map(|pane| pane.children.iter())
                    .collect(),
            )],
            Self::Popover(popover) => vec![
                ("anchor", all(&popover.anchor)),
                ("content", vec![popover.content.as_ref()]),
            ],
            Self::Overlay(overlay) => vec![("content", vec![overlay.content.as_ref()])],
            Self::List(list) => vec![("", all(&list.rows))],
            Self::Image(image) => vec![("overlay", all(&image.overlay))],
            _ => Vec::new(),
        }
    }

    fn data(&self, at: &str, budget: &mut Budget) -> Data {
        let kind = self.kind();
        let path = if at.is_empty() {
            kind.to_owned()
        } else {
            format!("{at}.{kind}")
        };
        let mut fields: Vec<(&'static str, Data)> = vec![
            ("kind", Data::from(kind)),
            ("path", Data::from(path.as_str())),
        ];
        if let Some(look) = self.look() {
            if let Some(line) = look.line {
                fields.push(("line", Data::Integer(i64::from(line))));
            }
            if !look.class.is_empty() {
                fields.push(("class", Data::from(look.class.join(" "))));
            }
            if let Some(state) = &look.state {
                fields.push(("state", Data::from(state.as_str())));
            }
        }
        self.fields(&mut fields);
        budget.spend(
            fields
                .iter()
                .map(|(key, value)| key.len() + weight(value))
                .sum(),
        );
        for (key, nodes) in self.parts() {
            if nodes.is_empty() {
                continue;
            }
            let mut children = Vec::new();
            for (index, node) in nodes.into_iter().enumerate() {
                if budget.left == 0 {
                    budget.truncated = true;
                    break;
                }
                let at = match key {
                    "" => format!("{path}[{}]", index + 1),
                    key => format!("{path}.{key}[{}]", index + 1),
                };
                children.push(node.data(&at, budget));
            }
            fields.push((
                if key.is_empty() {
                    "children"
                } else {
                    key
                },
                Data::List(children),
            ));
        }
        Data::map(fields)
    }
}

struct Budget {
    left: usize,
    truncated: bool,
}

impl Budget {
    fn spend(&mut self, bytes: usize) {
        self.left = self.left.saturating_sub(bytes + 16);
    }
}

/// About how many bytes a value takes written out.
fn weight(value: &Data) -> usize {
    match value {
        Data::String(text) => text.len() + 2,
        _ => 8,
    }
}
