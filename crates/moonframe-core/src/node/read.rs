//! Reads a `view`'s Lua tables into [`Node`]s. Each element type says which
//! keys it takes and reads them itself ([`Build`]); unknown elements and keys
//! are refused, so a typo is reported where it is.

use {
    super::*,
    crate::{
        Error, Stylesheet,
        read::{Cx, Document, Fields, Read},
        style::Edges,
    },
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
    "shrink",
    "self_align",
    "hug",
    "max_width",
    "max_height",
    "float",
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
            Element::Hatch => Node::Hatch(self.build(kind, table, &cx)?),
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

impl Build for Stack {
    const CHILDREN: bool = true;
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, FRAME, LOOK, ACTION, EXTRAS, &[
        "align", "justify", "wrap", "selected", "drag", "key",
    ]];

    fn build(kind: Element, fields: &Fields, tree: &mut Tree<'_>) -> Result<Self, Error> {
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
            selected: fields.get("selected")?.unwrap_or(false),
            action: Action::from_fields(fields)?,
            extras: Extras::from_fields(fields)?,
            drag: fields.get("drag")?.unwrap_or(false),
            key: fields.get::<PlainId>("key")?.map(|key| key.0),
            children: tree.children(fields)?,
        })
    }
}

impl Build for Panel {
    const CHILDREN: bool = true;
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, FRAME, LOOK, &["title", "actions"]];

    fn build(_: Element, fields: &Fields, tree: &mut Tree<'_>) -> Result<Self, Error> {
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
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, &["id", "divider"]];

    fn build(kind: Element, fields: &Fields, tree: &mut Tree<'_>) -> Result<Self, Error> {
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
            divider: fields.get("divider")?.unwrap_or(true),
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
        "size", "min", "max", "fixed", "visible", "lazy", "gap", "padding", "surface", "scroll",
    ]];

    fn build(_: Element, fields: &Fields, tree: &mut Tree<'_>) -> Result<Self, Error> {
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
            lazy: fields.get("lazy")?.unwrap_or(false),
            frame: Frame::from_fields(fields)?,
            children: tree.children(fields)?,
        })
    }
}

impl Build for Text {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ATTACHED, &[
        "text", "tone", "size", "weight", "truncate", "lines", "wrap", "strike",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            text: fields.need("text")?,
            tone: fields.get("tone")?,
            size: fields.get("size")?,
            weight: fields.get("weight")?,
            truncate: fields.get("truncate")?.unwrap_or(false),
            lines: fields
                .number("lines", 1.0..=100.0)?
                .map(|lines| lines as u8),
            wrap: fields.get("wrap")?.unwrap_or(true),
            strike: fields.get("strike")?.unwrap_or(false),
            extras: Extras::from_fields(fields)?,
        })
    }
}

impl Build for Button {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ACTION, EXTRAS, &[
        "label", "icon", "variant", "small", "disabled",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let icon = fields.get("icon")?;
        let label = match icon {
            Some(_) => fields.get("label")?.unwrap_or_default(),
            None => fields.need("label")?,
        };
        Ok(Self {
            look: Look::from_fields(fields)?,
            label,
            icon,
            variant: fields.get("variant")?.unwrap_or(Variant::Normal),
            small: fields.get("small")?.unwrap_or(false),
            disabled: fields.get("disabled")?.unwrap_or(false),
            action: Action::from_fields(fields)?,
            extras: Extras::from_fields(fields)?,
        })
    }
}

impl Build for Item {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ACTION, EXTRAS, &[
        "label", "detail", "badge", "selected",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            label: fields.need("label")?,
            detail: fields.get("detail")?,
            badge: fields.get("badge")?,
            selected: fields.get("selected")?.unwrap_or(false),
            action: Action::from_fields(fields)?,
            extras: Extras::from_fields(fields)?,
        })
    }
}

impl Build for Badge {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, ATTACHED, &["text", "tone"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            text: fields.need("text")?,
            tone: fields.get("tone")?,
            extras: Extras::from_fields(fields)?,
        })
    }
}

impl Build for Icon {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, LOOK, ATTACHED, &["name", "tone", "size", "spin"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            name: fields.need("name")?,
            tone: fields.get("tone")?,
            size: fields.get("size")?,
            spin: fields.get("spin")?.unwrap_or(false),
            extras: Extras::from_fields(fields)?,
        })
    }
}

/// The most points a `ui.line` draws.
const MOST_POINTS: usize = 2000;

impl Build for Line {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "points", "min", "max", "baseline", "area", "dots", "stroke", "dashed", "width", "height",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        let points: Vec<f32> = fields.need("points")?;
        if points.len() > MOST_POINTS {
            return Err(fields
                .cx()
                .key("points")
                .fail(format!("at most {MOST_POINTS} points")));
        }
        if points.iter().any(|value| !value.is_finite()) {
            return Err(fields.cx().key("points").fail("points are numbers"));
        }
        let value = -1.0e12..=1.0e12;
        Ok(Self {
            look: Look::from_fields(fields)?,
            points,
            min: fields.number("min", value.clone())?,
            max: fields.number("max", value.clone())?,
            baseline: fields.number("baseline", value)?,
            area: fields.get("area")?.unwrap_or(false),
            dots: fields.get("dots")?,
            stroke: fields.number("stroke", 0.5..=8.0)?.unwrap_or(1.5),
            dashed: fields.get("dashed")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
        })
    }
}

impl Build for Hatch {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, LOOK, &["gap", "width", "height", "grow"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            gap: fields.number("gap", 2.0..=64.0)?.unwrap_or(6.0),
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            grow: fields.get("grow")?.unwrap_or(false),
        })
    }
}

impl Build for Image {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, LOOK, ATTACHED, &["src", "width", "height", "fit"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            src: fields.need::<ImageSrc>("src")?.0,
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            fit: fields.get("fit")?.unwrap_or(Fit::Contain),
            extras: Extras::from_fields(fields)?,
        })
    }
}

/// An image's `src`: a file. The host has no HTTP client, and a view must
/// not make the app fetch things, so a URL is refused with what to do.
struct ImageSrc(String);

impl Read for ImageSrc {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let src = String::read(value, cx)?;
        let scheme = src
            .split_once("://")
            .map(|(scheme, _)| scheme)
            .filter(|scheme| {
                !scheme.is_empty()
                    && scheme
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
            });
        if let Some(scheme) = scheme {
            return Err(cx.fail(format!(
                "images are files, not {scheme}:// URLs; have a provider download it to a \
                 cache and pass the path"
            )));
        }
        if src.trim().is_empty() {
            return Err(cx.fail("an empty path"));
        }
        Ok(Self(src))
    }
}

impl Build for Terminal {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, LOOK, &["id", "cwd", "engine", "backend"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields
                .get::<TerminalId>("id")?
                .map_or_else(|| "terminal".to_owned(), |id| id.0),
            cwd: fields.get("cwd")?,
            engine: fields.get("engine")?.unwrap_or(TerminalEngine::Alacritty),
            backend: fields.get("backend")?.unwrap_or(TerminalBackend::Auto),
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

/// Longest a terminal or browser `id` can be.
const MAX_TERMINAL_ID: usize = 64;

impl Read for TerminalId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        plain_id(value, cx).map(Self)
    }
}

/// An id the host keeps something by: 1 to 64 of `a-z A-Z 0-9 - _`.
fn plain_id(value: &Value, cx: &Cx) -> Result<String, Error> {
    let id = String::read(value, cx)?;
    let plain = !id.is_empty()
        && id.len() <= MAX_TERMINAL_ID
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if plain {
        Ok(id)
    } else {
        Err(cx.fail(format!(
            "an id is 1 to {MAX_TERMINAL_ID} of a-z, A-Z, 0-9, - and _"
        )))
    }
}

impl Build for Browser {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, LOOK, &["id", "url", "zoom", "navigate", "loading"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields
                .get::<BrowserId>("id")?
                .map_or_else(|| "browser".to_owned(), |id| id.0),
            url: fields.get::<WebUrl>("url")?.map(|url| url.0),
            zoom: fields.number("zoom", 0.25..=5.0)?.unwrap_or(1.0),
            navigate: fields.get("navigate")?,
            loading: fields.get("loading")?,
        })
    }
}

impl Build for View {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &["name", "id", "props"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let name = fields.get::<ViewName>("name")?.ok_or_else(|| {
            fields
                .cx()
                .key("name")
                .fail("a view needs the `name` the app registered")
        })?;
        Ok(Self {
            look: Look::from_fields(fields)?,
            name: name.0,
            id: fields
                .get::<BrowserId>("id")?
                .map_or_else(|| "view".to_owned(), |id| id.0),
            props: fields.get("props")?.unwrap_or(Data::Nil),
        })
    }
}

/// The name an app registered a view by: `herdr.workspace`, `chart`.
struct ViewName(String);

impl Read for ViewName {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let name = String::read(value, cx)?;
        let plain = !name.is_empty()
            && name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if plain {
            Ok(Self(name))
        } else {
            Err(cx.fail("a view's name is 1 to 64 of a-z, A-Z, 0-9, -, _ and ."))
        }
    }
}

/// A browser's `id`, kept by the mod like a terminal's.
struct BrowserId(String);

impl Read for BrowserId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        plain_id(value, cx).map(Self)
    }
}

/// A browser's `url`: only web and file addresses, never a script or an
/// app's own scheme.
struct WebUrl(String);

impl Read for WebUrl {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let url = String::read(value, cx)?;
        if is_web_url(&url) {
            Ok(Self(url))
        } else {
            Err(cx.fail(format!(
                "`{}` is not an http, https or file address",
                url.chars().take(80).collect::<String>()
            )))
        }
    }
}

/// A control's `command`: the app's own, or one for a browser.
enum AnyCommand {
    App(Command),
    Browser(BrowserCommand),
}

impl Read for AnyCommand {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let name = String::read(value, cx)?;
        Command::parse(&name)
            .map(Self::App)
            .or_else(|| BrowserCommand::parse(&name).map(Self::Browser))
            .ok_or_else(|| {
                cx.fail(format!(
                    "unknown `{name}`; expected one of {}, {}",
                    Command::NAMES.join(", "),
                    BrowserCommand::NAMES.join(", ")
                ))
            })
    }
}

impl Build for Input {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "id",
        "value",
        "placeholder",
        "to",
        "change",
        "submit",
        "keys",
        "multiline",
        "rows",
        "disabled",
        "focus",
        "mono",
        "width",
        "grow",
        "browser",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let multiline = fields.get("multiline")?.unwrap_or(false);
        let rows = fields.get::<f32>("rows")?;
        if rows.is_some() && !multiline {
            return Err(fields
                .cx()
                .key("rows")
                .fail("`rows` goes with `multiline = true`"));
        }
        let rows = match rows {
            None => Self::DEFAULT_ROWS,
            Some(rows) if rows.fract() == 0.0 && (1.0..=Self::MAX_ROWS as f32).contains(&rows) => {
                rows as u32
            },
            Some(_) => {
                return Err(fields
                    .cx()
                    .key("rows")
                    .fail(format!("a whole number from 1 to {}", Self::MAX_ROWS)));
            },
        };
        let change = fields.get::<Data>("change")?;
        let submit = fields.get::<Data>("submit")?;
        let keys = fields
            .get::<InputKeys>("keys")?
            .map(|keys| keys.0)
            .unwrap_or_default();
        let to = fields.get::<String>("to")?;
        let browser = fields.get::<BrowserId>("browser")?.map(|id| id.0);
        if to.is_some()
            && change.is_none()
            && submit.is_none()
            && keys.is_empty()
            && browser.is_none()
        {
            return Err(fields
                .cx()
                .key("to")
                .fail("`to` goes with `change`, `submit`, `keys` or `browser`"));
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields.get::<InputId>("id")?.map(|id| id.0),
            // A text area may hold a whole document.
            value: if multiline {
                fields.get::<Document>("value")?.map(|text| text.0)
            } else {
                fields.get("value")?
            },
            placeholder: fields.get("placeholder")?,
            to,
            change,
            submit,
            keys,
            multiline,
            rows,
            disabled: fields.get("disabled")?.unwrap_or(false),
            focus: fields.get("focus")?.unwrap_or(false),
            mono: fields.get("mono")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            grow: fields.get("grow")?.unwrap_or(false),
            browser,
        })
    }
}

impl Build for Markdown {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "text",
        "link",
        "to",
        "grow",
        "scroll",
        "width",
        "height",
        "max_width",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let link = fields.get::<Data>("link")?;
        let to = fields.get::<String>("to")?;
        if to.is_some() && link.is_none() {
            return Err(fields.cx().key("to").fail("`to` goes with `link`"));
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            text: fields
                .get::<Document>("text")?
                .map(|text| text.0)
                .unwrap_or_default(),
            link,
            to,
            grow: fields.get("grow")?.unwrap_or(false),
            scroll: fields.get("scroll")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            max_width: fields.number("max_width", PIXELS)?,
        })
    }
}

/// An input's `keys`: a table from key names to the messages they send.
struct InputKeys(Vec<(KeyChord, Data)>);

/// Most keys one input can take over.
const MAX_KEYS: usize = 32;

impl Read for InputKeys {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let Value::Table(table) = value else {
            return Err(
                cx.fail("expected a table such as { up = \"previous\", escape = \"close\" }")
            );
        };
        let mut keys: Vec<(String, KeyChord, Data)> = Vec::new();
        for pair in table.pairs::<Value, Value>() {
            let (name, msg) = pair?;
            let Value::String(name) = name else {
                return Err(cx.fail("keys are key names such as `up` or `ctrl-n`"));
            };
            let name = name.to_string_lossy();
            let cx = cx.quoted(&name);
            let chord = KeyChord::parse(&name).map_err(|problem| cx.fail(problem))?;
            if let Some((other, ..)) = keys.iter().find(|(_, other, _)| *other == chord) {
                return Err(cx.fail(format!("the same key as `{other}`")));
            }
            keys.push((name, chord, <Data as Read>::read(&msg, &cx)?));
            if keys.len() > MAX_KEYS {
                return Err(cx.fail(format!("more than {MAX_KEYS} keys")));
            }
        }
        // Table order is not the script's; sort so a view reads the same each time.
        keys.sort_by(|(a, ..), (b, ..)| a.cmp(b));
        Ok(Self(
            keys.into_iter()
                .map(|(_, chord, msg)| (chord, msg))
                .collect(),
        ))
    }
}

/// An input's `id`, kept by the mod like a terminal's.
struct InputId(String);

impl Read for InputId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        plain_id(value, cx).map(Self)
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

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
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
        "open", "dismiss", "to", "side", "align", "gap", "width", "keys", "content",
    ]];

    fn build(_: Element, fields: &Fields, tree: &mut Tree<'_>) -> Result<Self, Error> {
        let content = content(fields)?;
        Ok(Self {
            look: Look::from_fields(fields)?,
            open: fields.get("open")?.unwrap_or(false),
            dismiss: fields.get("dismiss")?,
            to: fields.get("to")?,
            side: fields.get("side")?.unwrap_or(Side::Top),
            align: fields.get("align")?.unwrap_or(Anchor::Start),
            gap: fields.number("gap", 0.0..=1000.0)?.unwrap_or(6.0),
            width: fields.number("width", PIXELS)?,
            keys: fields.get("keys")?.unwrap_or_default(),
            anchor: tree.children(fields)?,
            content: Box::new(tree.node(content, &fields.cx().nested("content"))?),
        })
    }
}

impl Build for Overlay {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "open", "dismiss", "to", "place", "backdrop", "width", "height", "keys", "content",
    ]];

    fn build(_: Element, fields: &Fields, tree: &mut Tree<'_>) -> Result<Self, Error> {
        let content = content(fields)?;
        Ok(Self {
            look: Look::from_fields(fields)?,
            open: fields.get("open")?.unwrap_or(false),
            dismiss: fields.get("dismiss")?,
            to: fields.get("to")?,
            place: fields.get("place")?.unwrap_or(Placement::Center),
            backdrop: fields.get("backdrop")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            keys: fields.get("keys")?.unwrap_or_default(),
            content: Box::new(tree.node(content, &fields.cx().nested("content"))?),
        })
    }
}

impl Build for List {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, FRAME, LOOK, &["id", "count", "row_height", "row"]];

    fn build(_: Element, fields: &Fields, tree: &mut Tree<'_>) -> Result<Self, Error> {
        let id = fields
            .get::<PlainId>("id")?
            .map_or_else(|| "list".to_owned(), |id| id.0);
        if !tree.lists.insert(id.clone()) {
            return Err(fields
                .cx()
                .key("id")
                .fail(format!("two lists with the id `{id}`; give each its own")));
        }
        let count = fields.need::<i64>("count")?;
        let count = usize::try_from(count)
            .ok()
            .filter(|count| *count <= MAX_ROWS)
            .ok_or_else(|| {
                fields
                    .cx()
                    .key("count")
                    .fail(format!("expected 0 to {MAX_ROWS} rows"))
            })?;
        let row_height = fields
            .number("row_height", 1.0..=10_000.0)?
            .ok_or_else(|| {
                fields
                    .cx()
                    .key("row_height")
                    .fail("missing: each row's height in pixels")
            })?;
        let row: mlua::Function = fields.need("row")?;
        let window = tree.windows.get(&id).copied().unwrap_or(ListWindow::FIRST);
        let range = window.range(count);
        let cx = fields.cx().key("row");
        let mut rows = Vec::with_capacity(range.len());
        for index in range.clone() {
            // Rows count from 1 in Lua.
            let value = row
                .call::<Value>(index + 1)
                .map_err(|error| Error::from(error).context(format!("row({})", index + 1)))?;
            rows.push(tree.child(&value, &cx.index(index + 1))?);
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            frame: Frame::from_fields(fields)?,
            id,
            count,
            row_height,
            start: range.start,
            rows,
        })
    }
}

/// The most rows a list may have; it builds only those that show.
const MAX_ROWS: usize = 10_000_000;

/// The element a popover or overlay floats.
fn content<'f>(fields: &'f Fields) -> Result<&'f Value, Error> {
    fields
        .raw("content")
        .ok_or_else(|| fields.cx().key("content").fail("missing"))
}

/// `ui.mount "name"` draws a mod where it is; with an `id`, it mounts an
/// instance of the mod with state of its own.
enum Mount {
    Place(String),
    Instance(Instance),
}

impl Build for Mount {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, &["mod", "id", "props"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let name: String = fields.need("mod")?;
        // A mod's name is its folder's, never a path: an instance loads
        // `mods/<name>/main.luau`.
        if !crate::style::valid_name(&name) {
            return Err(fields
                .cx()
                .key("mod")
                .fail(format!("`{name}` is not a mod name: use a-z, 0-9, - and _")));
        }
        match fields.get::<PlainId>("id")? {
            Some(PlainId(id)) => Ok(Self::Instance(Instance {
                of: InstanceOf::Mod(name),
                id,
                props: fields.get("props")?.unwrap_or(Data::Nil),
            })),
            None if fields.raw("props").is_some() => Err(fields
                .cx()
                .key("props")
                .fail("props go to an instance: give it an `id`")),
            None => Ok(Self::Place(name)),
        }
    }
}

/// What `use` returns for a component that keeps state builds: an instance
/// of it.
impl Build for Instance {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, &["component", "id", "props"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            of: InstanceOf::Component(fields.need("component")?),
            id: fields.need::<PlainId>("id")?.0,
            props: fields.get("props")?.unwrap_or(Data::Nil),
        })
    }
}

impl Read for TabKind {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&["title", "icon", "props", "report"]])?;
        Ok(Self {
            title: fields.get("title")?,
            icon: fields.get("icon")?,
            props: fields.get("props")?.unwrap_or(Data::Nil),
            report: fields.get("report")?,
        })
    }
}

/// An instance's `id`, as plain as a terminal's.
struct PlainId(String);

impl Read for PlainId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        plain_id(value, cx).map(Self)
    }
}

/// An element with nothing to read: dividers and spacers.
struct Bare;

impl Build for Bare {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE];

    fn build(_: Element, _: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
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
            scroll: fields
                .get::<ScrollKey>("scroll")?
                .map_or(Scroll::No, |key| key.0),
            shrink: fields.get("shrink")?.unwrap_or(true),
            self_align: fields.get("self_align")?,
            hug: fields.get("hug")?.unwrap_or(false),
            max_width: fields.number("max_width", PIXELS)?,
            max_height: fields.number("max_height", PIXELS)?,
            float: fields.get("float")?,
        })
    }
}

impl Action {
    /// A control's `msg` (with an optional `to`), `command` or `window`; a browser
    /// command names its browser with `browser` (and its mod with `to`).
    pub(crate) fn from_fields(fields: &Fields) -> Result<Option<Self>, Error> {
        if let Some(name) = fields.get::<PlainId>("window")? {
            let alone = ["msg", "command", "to", "browser"]
                .iter()
                .all(|key| fields.raw(key).is_none());
            return if alone {
                Ok(Some(Self::Window(name.0)))
            } else {
                Err(fields.cx().key("window").fail(
                    "`window` (a mod to open in its own window) takes no `msg`, `to` or `command`",
                ))
            };
        }
        let browser = fields.get::<BrowserId>("browser")?.map(|id| id.0);
        let misplaced = || {
            fields.cx().key("browser").fail(format!(
                "`browser` goes with a browser command: {}",
                BrowserCommand::NAMES.join(", ")
            ))
        };
        match (
            fields.get::<Data>("msg")?,
            fields.get::<String>("to")?,
            fields.get::<AnyCommand>("command")?,
        ) {
            (Some(_), _, Some(_)) => Err(fields.cx().fail("takes `msg` or `command`, not both")),
            (Some(_), _, None) if browser.is_some() => Err(misplaced()),
            (Some(msg), to, None) => Ok(Some(Self::Send { to, msg })),
            (None, to, Some(AnyCommand::Browser(command))) => {
                let id = browser.ok_or_else(|| {
                    fields
                        .cx()
                        .key("command")
                        .fail("a browser command needs `browser = \"<id>\"`, its ui.browser's id")
                })?;
                Ok(Some(Self::Browser { to, id, command }))
            },
            (None, ..) if browser.is_some() => Err(misplaced()),
            (None, Some(_), _) if fields.raw("context").is_none() => Err(fields
                .cx()
                .key("to")
                .fail("`to` goes with a `msg`, `context` or browser `command`")),
            (None, _, Some(AnyCommand::App(command))) => Ok(Some(Self::Command(command))),
            (None, _, None) => Ok(None),
        }
    }
}

impl Extras {
    /// A control's `tooltip`, `context` (a message sent to `to` like `msg`)
    /// and `copy`.
    fn from_fields(fields: &Fields) -> Result<Self, Error> {
        let context = fields
            .get::<Data>("context")?
            .map(|msg| -> Result<Action, Error> {
                Ok(Action::Send {
                    to: fields.get("to")?,
                    msg,
                })
            })
            .transpose()?;
        Ok(Self {
            tooltip: fields.get("tooltip")?,
            context,
            copy: fields.get("copy")?,
        })
    }
}

/// Where `float = "<at>"` pins an element: the edges it is held to.
const FLOAT_AT: &[(&str, [bool; 4])] = &[
    // top, right, bottom, left
    ("top-left", [true, false, false, true]),
    ("top-right", [true, true, false, false]),
    ("bottom-left", [false, false, true, true]),
    ("bottom-right", [false, true, true, false]),
    ("top", [true, true, false, true]),
    ("bottom", [false, true, true, true]),
    ("left", [true, false, true, true]),
    ("right", [true, true, true, false]),
    ("fill", [true, true, true, true]),
];

/// `float = { top = -4, right = -4 }`: pixels from the parent's edges. Or
/// pinned to an edge or corner, `float = "top-right"`, and set in from it,
/// `float = { at = "top-right", inset = 4 }`.
impl Read for Offsets {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        const RANGE: std::ops::RangeInclusive<f32> = -100_000.0..=100_000.0;
        if let Value::String(_) = value {
            return pinned(&String::read(value, cx)?, 0.0, cx);
        }
        let fields = Fields::read(value, cx, &[&[
            "top", "right", "bottom", "left", "at", "inset",
        ]])?;
        if let Some(at) = fields.get::<String>("at")? {
            let edges = ["top", "right", "bottom", "left"];
            if let Some(edge) = edges.iter().find(|edge| fields.raw(edge).is_some()) {
                return Err(cx
                    .key(edge)
                    .fail("give `at` and `inset`, or the edges, not both"));
            }
            let inset = fields.number("inset", RANGE)?.unwrap_or(0.0);
            return pinned(&at, inset, &cx.key("at"));
        }
        if fields.raw("inset").is_some() {
            return Err(cx.key("inset").fail("`inset` goes with `at`"));
        }
        let edge = |key| fields.number(key, RANGE);
        Ok(Self {
            top: edge("top")?,
            right: edge("right")?,
            bottom: edge("bottom")?,
            left: edge("left")?,
        })
    }
}

/// The offsets that hold an element `inset` pixels in from the edges `at`
/// names.
fn pinned(at: &str, inset: f32, cx: &Cx) -> Result<Offsets, Error> {
    let Some((_, [top, right, bottom, left])) = FLOAT_AT.iter().find(|(name, _)| *name == at)
    else {
        let names: Vec<&str> = FLOAT_AT.iter().map(|(name, _)| *name).collect();
        return Err(cx.fail(format!(
            "`{at}` is not a place; use {}, or {{ top, right, bottom, left }}",
            names.join(", ")
        )));
    };
    let edge = |held: &bool| held.then_some(inset);
    Ok(Offsets {
        top: edge(top),
        right: edge(right),
        bottom: edge(bottom),
        left: edge(left),
    })
}

/// `scroll = true` (along the axis), or `"x"`, `"y"`, `"both"`.
struct ScrollKey(Scroll);

impl Read for ScrollKey {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Boolean(true) => Ok(Self(Scroll::Along)),
            Value::Boolean(false) => Ok(Self(Scroll::No)),
            _ => Scroll::read(value, cx).map(Self),
        }
    }
}
