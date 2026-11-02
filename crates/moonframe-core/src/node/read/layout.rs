//! Containers and placement: stacks, panels, splits and their panes,
//! regions, popovers, overlays, lists, mounts and instances.

use super::*;

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
pub(super) enum Mount {
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
pub(super) struct PlainId(pub(super) String);

impl Read for PlainId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        plain_id(value, cx).map(Self)
    }
}

/// An element with nothing to read: dividers and spacers.
pub(super) struct Bare;

impl Build for Bare {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE];

    fn build(_: Element, _: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self)
    }
}
