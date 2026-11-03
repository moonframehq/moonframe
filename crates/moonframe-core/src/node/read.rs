//! Reads a `view`'s Lua tables into [`Node`]s. Each element type says which
//! keys it takes and reads them itself ([`Build`]); unknown elements and keys
//! are refused, so a typo is reported where it is.

// keep-sorted start
mod common;
mod content;
mod embed;
mod input;
mod layout;
mod map;
mod pointer;
// keep-sorted end

use {
    super::*,
    crate::{
        Error, Stylesheet,
        read::{Cx, Fields, Read},
    },
    common::PIXELS,
    embed::{BrowserId, plain_id},
    layout::{Bare, Mount, PlainId},
    mlua::{Table, Value},
    std::collections::{BTreeMap, BTreeSet},
};

const MAX_DEPTH: usize = 48;
const MAX_NODES: usize = 10_000;

/// Key groups elements share. `type` is the tag `ui` sets.
const TYPE: &[&str] = &["type"];
const FRAME: &[&str] = &[
    "gap",
    "padding",
    "grow",
    "width",
    "height",
    "surface",
    "border",
    "scroll",
    "follow",
    "shrink",
    "self_align",
    "hug",
    "max_width",
    "max_height",
    "float",
    "area",
];
const LOOK: &[&str] = &["class", "style", "state"];
const ACTION: &[&str] = &["msg", "to", "command", "browser", "window"];
const EXTRAS: &[&str] = &["tooltip", "context", "copy"];
/// What elements without a click take of [`EXTRAS`]: the same, and the
/// `to` a `context` message goes to.
const ATTACHED: &[&str] = &["tooltip", "context", "copy", "to"];

/// Reads a view; `sheet` resolves the color names in `style` keys. A broken
/// child becomes a [`Node::Problem`] in its place, so one mistake marks one
/// spot and the rest of the view still draws; its error is returned
/// alongside. The root, and the limits on size and depth,
/// still fail the whole view.
///
/// `windows` says which rows of each `ui.list` to build, by its `id`; a
/// list not in it builds its first rows.
pub(crate) fn read(
    value: &Value,
    sheet: &Stylesheet,
    windows: &BTreeMap<String, ListWindow>,
) -> Result<(Node, Vec<Error>), Error> {
    let mut tree = Tree {
        nodes: 0,
        problems: Vec::new(),
        halted: false,
        windows,
        lists: BTreeSet::new(),
    };
    let node = tree.node(value, &Cx::root(sheet.names()))?;
    Ok((node, tree.problems))
}

/// An element type: the keys it takes, and how it reads them.
trait Build: Sized {
    const KEYS: &'static [&'static [&'static str]];
    /// Whether its list part holds child elements.
    const CHILDREN: bool = false;

    fn build(kind: Element, fields: &Fields, tree: &mut Tree<'_>) -> Result<Self, Error>;
}

/// The reader's state across a tree: how many elements it has read, and
/// the children it could not.
struct Tree<'w> {
    nodes: usize,
    problems: Vec<Error>,
    /// Past a limit: the whole view fails rather than marking a spot.
    halted: bool,
    /// The rows of each list to build, by id.
    windows: &'w BTreeMap<String, ListWindow>,
    /// The ids of the lists read so far, which must differ.
    lists: BTreeSet<String>,
}

impl Tree<'_> {
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
            Element::Image => Node::Image(self.build(kind, table, &cx)?),
            Element::Line => Node::Line(self.build(kind, table, &cx)?),
            Element::Paths => Node::Paths(self.build(kind, table, &cx)?),
            Element::Hatch => Node::Hatch(self.build(kind, table, &cx)?),
            Element::Map => Node::Map(self.build(kind, table, &cx)?),
            Element::Terminal => Node::Terminal(self.build(kind, table, &cx)?),
            Element::Browser => Node::Browser(self.build(kind, table, &cx)?),
            Element::View => Node::View(self.build(kind, table, &cx)?),
            Element::Input => Node::Input(self.build(kind, table, &cx)?),
            Element::Markdown => Node::Markdown(self.build(kind, table, &cx)?),
            Element::Divider => self
                .build::<Bare>(kind, table, &cx)
                .map(|_| Node::Divider)?,
            Element::Spacer => self.build::<Bare>(kind, table, &cx).map(|_| Node::Spacer)?,
            Element::Region => Node::Region(self.build(kind, table, &cx)?),
            Element::Mount => match self.build::<Mount>(kind, table, &cx)? {
                Mount::Place(name) => Node::Mount(name),
                Mount::Instance(instance) => Node::Instance(instance),
            },
            Element::Instance => Node::Instance(self.build(kind, table, &cx)?),
            Element::Popover => Node::Popover(self.build(kind, table, &cx)?),
            Element::Overlay => Node::Overlay(self.build(kind, table, &cx)?),
            Element::List => Node::List(self.build(kind, table, &cx)?),
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
        let mut nodes = items
            .iter()
            .enumerate()
            .map(|(index, item)| self.child(item, &cx.index(index + 1)))
            .collect::<Result<Vec<_>, _>>()?;
        // A key names one child among its siblings: two with the same one
        // would share a place, and so its size and animations.
        let mut keys = BTreeSet::new();
        for (index, node) in nodes.iter_mut().enumerate() {
            if let Node::Stack(Stack { key: Some(key), .. }) = node
                && !keys.insert(key.clone())
            {
                let error = cx.index(index + 1).key("key").fail(format!(
                    "a sibling already has the key `{key}`; keys differ among siblings"
                ));
                *node = Node::Problem(error.to_string());
                self.problems.push(error);
            }
        }
        Ok(nodes)
    }

    /// A child element; one that fails to read becomes a [`Node::Problem`]
    /// in its place, unless a limit halted the whole view.
    fn child(&mut self, value: &Value, cx: &Cx) -> Result<Node, Error> {
        match self.node(value, cx) {
            Err(error) if !self.halted => {
                let problem = Node::Problem(error.to_string());
                self.problems.push(error);
                Ok(problem)
            },
            read => read,
        }
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
