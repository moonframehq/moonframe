//! Reads a `view`'s Lua tables into [`Node`]s. Each element type says which
//! keys it takes and reads them itself ([`Build`]); unknown elements and keys
//! are refused, so a typo is reported where it is.

use {
    super::*,
    crate::{
        Error, Stylesheet,
        read::{Cx, Fields, Read},
        style::Edges,
    },
    mlua::{Table, Value},
};

const MAX_DEPTH: usize = 48;
const MAX_NODES: usize = 10_000;

/// Key groups elements share. `type` is the tag `ui` sets.
const TYPE: &[&str] = &["type"];
const FRAME: &[&str] = &[
    "gap", "padding", "grow", "width", "height", "surface", "border", "scroll",
];
const LOOK: &[&str] = &["class", "style"];
const ACTION: &[&str] = &["msg", "to", "command"];

/// Reads a whole tree; `sheet` resolves the color names in `style` keys.
/// Reads a view. A broken child becomes a [`Node::Problem`] in its place,
/// so one mistake marks one spot and the rest of the view still draws; its
/// error is returned alongside. The root, and the limits on size and depth,
/// still fail the whole view.
pub(crate) fn read(value: &Value, sheet: &Stylesheet) -> Result<(Node, Vec<Error>), Error> {
    let mut tree = Tree::default();
    let node = tree.node(value, &Cx::root(sheet.names()))?;
    Ok((node, tree.problems))
}

/// An element type: the keys it takes, and how it reads them.
trait Build: Sized {
    const KEYS: &'static [&'static [&'static str]];
    /// Whether its list part holds child elements.
    const CHILDREN: bool = false;

    fn build(kind: Element, fields: &Fields, tree: &mut Tree) -> Result<Self, Error>;
}

/// The reader's state across a tree: how many elements it has read, and
/// the children it could not.
#[derive(Default)]
struct Tree {
    nodes: usize,
    problems: Vec<Error>,
    /// Past a limit: the whole view fails rather than marking a spot.
    halted: bool,
}

impl Tree {
    fn node(&mut self, value: &Value, cx: &Cx) -> Result<Node, Error> {
        self.count(cx)?;
        let table = match value {
            Value::String(_) => return Ok(Node::Text(Text::plain(String::read(value, cx)?))),
            Value::Table(table) => table,
            _ => return Err(cx.fail("expected an element from `ui`, or a string")),
        };
        let kind = kind(table, cx)?;
        let cx = cx.key(kind.name());
        Ok(match kind {
            Element::Row | Element::Column => Node::Stack(self.build(kind, table, &cx)?),
            Element::Panel => Node::Panel(self.build(kind, table, &cx)?),
            Element::Hsplit | Element::Vsplit => Node::Split(self.build(kind, table, &cx)?),
            Element::Pane => return Err(cx.fail("a pane goes inside ui.hsplit or ui.vsplit")),
            Element::Text => Node::Text(self.build(kind, table, &cx)?),
            Element::Button => Node::Button(self.build(kind, table, &cx)?),
            Element::Item => Node::Item(self.build(kind, table, &cx)?),
            Element::Badge => Node::Badge(self.build(kind, table, &cx)?),
            Element::Icon => Node::Icon(self.build(kind, table, &cx)?),
            Element::Terminal => Node::Terminal(self.build(kind, table, &cx)?),
            Element::Divider => self
                .build::<Bare>(kind, table, &cx)
                .map(|_| Node::Divider)?,
            Element::Spacer => self.build::<Bare>(kind, table, &cx).map(|_| Node::Spacer)?,
            Element::Region => Node::Region(self.build(kind, table, &cx)?),
            Element::Mount => Node::Mount(self.build::<Mount>(kind, table, &cx)?.0),
            Element::Popover => Node::Popover(self.build(kind, table, &cx)?),
        })
    }

    fn build<T: Build>(&mut self, kind: Element, table: &Table, cx: &Cx) -> Result<T, Error> {
        let fields = Fields::table(table, cx, T::KEYS)?;
        if !T::CHILDREN && !fields.items().is_empty() {
            return Err(cx.fail("takes no children"));
        }
        T::build(kind, &fields, self)
    }

    fn children(&mut self, fields: &Fields) -> Result<Vec<Node>, Error> {
        self.nodes(fields.items(), fields.cx())
    }

    /// A list key of elements, such as a panel's `actions`.
    fn list(&mut self, fields: &Fields, key: &str) -> Result<Vec<Node>, Error> {
        let items = fields.get::<Vec<Value>>(key)?.unwrap_or_default();
        self.nodes(&items, &fields.cx().key(key))
    }

    fn nodes(&mut self, items: &[Value], cx: &Cx) -> Result<Vec<Node>, Error> {
        let mut nodes = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            match self.node(item, &cx.index(index + 1)) {
                Ok(node) => nodes.push(node),
                Err(error) if self.halted => return Err(error),
                Err(error) => {
                    nodes.push(Node::Problem(error.to_string()));
                    self.problems.push(error);
                },
            }
        }
        Ok(nodes)
    }

    fn count(&mut self, cx: &Cx) -> Result<(), Error> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            self.halted = true;
            return Err(cx.fail(format!("more than {MAX_NODES} elements")));
        }
        if cx.depth() >= MAX_DEPTH {
            self.halted = true;
            return Err(cx.fail(format!("nested deeper than {MAX_DEPTH}")));
        }
        Ok(())
    }
}

/// The element type a table's `type` names.
fn kind(table: &Table, cx: &Cx) -> Result<Element, Error> {
    match table.raw_get::<Value>("type")? {
        Value::Nil => Err(cx.fail("a table without `type`; build elements with `ui`")),
        kind => Element::read(&kind, &cx.key("type")),
    }
}

impl Build for Stack {
    const CHILDREN: bool = true;
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, FRAME, LOOK, ACTION, &["align", "justify", "wrap"]];

    fn build(kind: Element, fields: &Fields, tree: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            direction: match kind {
                Element::Row => Direction::Row,
                _ => Direction::Column,
            },
            frame: Frame::from_fields(fields)?,
            align: fields.get("align")?.unwrap_or(Align::Stretch),
            justify: fields.get("justify")?.unwrap_or(Justify::Start),
            wrap: fields.get("wrap")?.unwrap_or(false),
            action: Action::from_fields(fields)?,
            children: tree.children(fields)?,
        })
    }
}

impl Build for Panel {
    const CHILDREN: bool = true;
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, FRAME, LOOK, &["title", "actions"]];

    fn build(_: Element, fields: &Fields, tree: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            title: fields.get("title")?,
            actions: tree.list(fields, "actions")?,
            frame: Frame::from_fields(fields)?,
            children: tree.children(fields)?,
        })
    }
}

impl Build for Split {
    const CHILDREN: bool = true;
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, &["id"]];

    fn build(kind: Element, fields: &Fields, tree: &mut Tree) -> Result<Self, Error> {
        let panes = fields
            .items()
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let cx = fields.cx().index(index + 1);
                tree.count(&cx)?;
                match item {
                    Value::Table(table) if kind_of(table) == Some(Element::Pane) => {
                        tree.build::<Pane>(Element::Pane, table, &cx.key("pane"))
                    },
                    _ => Err(cx.fail("the children of a split are ui.pane")),
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            direction: match kind {
                Element::Hsplit => Direction::Row,
                _ => Direction::Column,
            },
            id: fields.get("id")?,
            panes,
        })
    }
}

/// The element type a table names, if it names a known one.
fn kind_of(table: &Table) -> Option<Element> {
    let kind = table.raw_get::<String>("type").ok()?;
    Element::parse(&kind)
}

impl Build for Pane {
    const CHILDREN: bool = true;
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "size", "min", "max", "fixed", "visible", "gap", "padding", "surface", "scroll",
    ]];

    fn build(_: Element, fields: &Fields, tree: &mut Tree) -> Result<Self, Error> {
        let size = fields.number("size", PIXELS)?;
        let fixed = fields.get("fixed")?.unwrap_or(false);
        if fixed && size.is_none() {
            return Err(fields.cx().key("fixed").fail("a fixed pane needs a `size`"));
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            size,
            min: fields.number("min", PIXELS)?,
            max: fields.number("max", PIXELS)?,
            fixed,
            visible: fields.get("visible")?.unwrap_or(true),
            frame: Frame::from_fields(fields)?,
            children: tree.children(fields)?,
        })
    }
}

impl Build for Text {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, LOOK, &["text", "tone", "size", "weight", "truncate"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            text: fields.need("text")?,
            tone: fields.get("tone")?,
            size: fields.get("size")?,
            weight: fields.get("weight")?,
            truncate: fields.get("truncate")?.unwrap_or(false),
        })
    }
}

impl Build for Button {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ACTION, &[
        "label", "variant", "small", "disabled",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            label: fields.need("label")?,
            variant: fields.get("variant")?.unwrap_or(Variant::Normal),
            small: fields.get("small")?.unwrap_or(false),
            disabled: fields.get("disabled")?.unwrap_or(false),
            action: Action::from_fields(fields)?,
        })
    }
}

impl Build for Item {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ACTION, &[
        "label", "detail", "badge", "selected",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            label: fields.need("label")?,
            detail: fields.get("detail")?,
            badge: fields.get("badge")?,
            selected: fields.get("selected")?.unwrap_or(false),
            action: Action::from_fields(fields)?,
        })
    }
}

impl Build for Badge {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &["text", "tone"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            text: fields.need("text")?,
            tone: fields.get("tone")?,
        })
    }
}

impl Build for Icon {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &["name", "tone", "size"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            name: fields.need("name")?,
            tone: fields.get("tone")?,
            size: fields.get("size")?,
        })
    }
}

impl Build for Terminal {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &["id", "cwd", "engine"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields
                .get::<TerminalId>("id")?
                .map_or_else(|| "terminal".to_owned(), |id| id.0),
            cwd: fields.get("cwd")?,
            engine: fields.get("engine")?.unwrap_or(TerminalEngine::Alacritty),
        })
    }
}

/// Why `name` is no icon, with the bundled names closest to it rather than
/// all of them, which would bury the message.
fn unknown_icon(name: &str) -> String {
    let close: Vec<&str> = ICONS
        .iter()
        .copied()
        .filter(|icon| {
            icon.contains(name)
                || name.contains(icon)
                || icon
                    .split('-')
                    .any(|part| part.len() > 2 && name.contains(part))
                || edits(icon, name) <= 2
        })
        .take(6)
        .collect();
    let all = "see `IconName` in types/moonframe.d.luau for all of them";
    if close.is_empty() {
        format!("no icon named `{name}`; {all}")
    } else {
        format!(
            "no icon named `{name}`; close ones: {}; {all}",
            close.join(", ")
        )
    }
}

/// How many single-letter changes turn `a` into `b`.
fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut last = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(last + usize::from(ca != *cb));
            last = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

/// A terminal's `id`: one shell per mod and id, so it is short and plain.
struct TerminalId(String);

/// Longest a terminal `id` can be.
const MAX_TERMINAL_ID: usize = 64;

impl Read for TerminalId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let id = String::read(value, cx)?;
        let plain = !id.is_empty()
            && id.len() <= MAX_TERMINAL_ID
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if plain {
            Ok(Self(id))
        } else {
            Err(cx.fail(format!(
                "a terminal id is 1 to {MAX_TERMINAL_ID} of a-z, A-Z, 0-9, - and _"
            )))
        }
    }
}

impl Read for IconName {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let name = String::read(value, cx)?;
        Self::new(&name).ok_or_else(|| cx.fail(unknown_icon(&name)))
    }
}

impl Build for Region {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, &["name", "direction", "gap", "justify"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            name: fields.need("name")?,
            direction: fields.get("direction")?.unwrap_or(Direction::Column),
            gap: fields.get("gap")?.unwrap_or(Space::Normal),
            justify: fields.get("justify")?.unwrap_or(Justify::Start),
        })
    }
}

impl Build for Popover {
    const CHILDREN: bool = true;
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "open", "dismiss", "side", "align", "gap", "width", "content",
    ]];

    fn build(_: Element, fields: &Fields, tree: &mut Tree) -> Result<Self, Error> {
        let content = fields
            .raw("content")
            .ok_or_else(|| fields.cx().key("content").fail("missing"))?;
        Ok(Self {
            look: Look::from_fields(fields)?,
            open: fields.get("open")?.unwrap_or(false),
            dismiss: fields.get("dismiss")?,
            side: fields.get("side")?.unwrap_or(Side::Top),
            align: fields.get("align")?.unwrap_or(Anchor::Start),
            gap: fields.number("gap", 0.0..=1000.0)?.unwrap_or(6.0),
            width: fields.number("width", PIXELS)?,
            anchor: tree.children(fields)?,
            content: Box::new(tree.node(content, &fields.cx().key("content"))?),
        })
    }
}

/// `ui.mount "name"`: the mod to draw.
struct Mount(String);

impl Build for Mount {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, &["mod"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        fields.need("mod").map(Self)
    }
}

/// An element with nothing to read: dividers and spacers.
struct Bare;

impl Build for Bare {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE];

    fn build(_: Element, _: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self)
    }
}

/// Widths, heights, and pane sizes.
const PIXELS: std::ops::RangeInclusive<f32> = 0.0..=100_000.0;

impl Frame {
    fn from_fields(fields: &Fields) -> Result<Self, Error> {
        Ok(Self {
            gap: fields.get("gap")?,
            padding: fields.get("padding")?,
            grow: fields.get("grow")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            surface: fields.get("surface")?,
            border: fields.get::<Edges>("border")?.map(|edges| edges.0),
            scroll: fields.get("scroll")?.unwrap_or(false),
        })
    }
}

impl Action {
    /// A control's `msg` (with an optional `to`) or `command`.
    fn from_fields(fields: &Fields) -> Result<Option<Self>, Error> {
        match (
            fields.get::<Data>("msg")?,
            fields.get::<String>("to")?,
            fields.get::<Command>("command")?,
        ) {
            (Some(_), _, Some(_)) => Err(fields.cx().fail("takes `msg` or `command`, not both")),
            (Some(msg), to, None) => Ok(Some(Self::Send { to, msg })),
            (None, Some(_), _) => Err(fields.cx().key("to").fail("`to` goes with a `msg`")),
            (None, None, command) => Ok(command.map(Self::Command)),
        }
    }
}
