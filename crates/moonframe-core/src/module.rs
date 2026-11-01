//! One loaded module: its own sealed Luau state, its definition, and its
//! state between calls.

use {
    crate::{
        About, AppMenu, Chrome, Data, Error, ListWindow, Node, Rules, SHELL, Stylesheet, TabKind,
        component::Library,
        menus::Menus,
        node,
        read::{Cx, Fields, NO_NAMES, Read},
        vm::{self, Meter},
    },
    mlua::{Function, Lua, Table, Value},
    std::{
        cell::RefCell,
        collections::{BTreeMap, BTreeSet},
        rc::Rc,
    },
};

/// What a view gets as `app`, by key.
pub(crate) type AppData = BTreeMap<String, Data>;

/// The keys of `app` a view read: it is drawn again only when one of
/// them changes. Going through all of `app` reads every key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Reads {
    Keys(BTreeSet<String>),
    #[default]
    All,
}

/// How a view sees `app`: an empty table whose keys are filled in as the
/// view reads them, so only what it reads crosses into Luau, and the reads
/// are counted.
struct Lens {
    /// The `app` of the view being drawn.
    app: Rc<RefCell<Rc<AppData>>>,
    reads: Rc<RefCell<Reads>>,
    meta: Table,
}

impl Lens {
    fn new(lua: &Lua) -> mlua::Result<Self> {
        let app = Rc::new(RefCell::new(Rc::new(AppData::new())));
        let reads = Rc::new(RefCell::new(Reads::Keys(BTreeSet::new())));
        let meta = lua.create_table()?;
        let (from, seen) = (app.clone(), reads.clone());
        let index = lua.create_function(move |lua, (table, key): (Table, Value)| {
            let Value::String(name) = &key else {
                return Ok(Value::Nil);
            };
            let name = name.to_string_lossy();
            if let Reads::Keys(keys) = &mut *seen.borrow_mut() {
                keys.insert(name.clone());
            }
            let value = match from.borrow().get(&name) {
                Some(data) => data.to_lua(lua)?,
                None => Value::Nil,
            };
            // Read once per view: the next read finds it in place.
            table.raw_set(key, value.clone())?;
            Ok(value)
        })?;
        let (from, seen) = (app.clone(), reads.clone());
        let iter = lua.create_function(move |lua, table: Table| {
            *seen.borrow_mut() = Reads::All;
            for (name, data) in from.borrow().iter() {
                if table.raw_get::<Value>(name.as_str())?.is_nil() {
                    table.raw_set(name.as_str(), data.to_lua(lua)?)?;
                }
            }
            let next = lua.globals().get::<Function>("next")?;
            Ok((next, table))
        })?;
        meta.raw_set("__index", index)?;
        meta.raw_set("__iter", iter)?;
        meta.set_readonly(true);
        Ok(Self { app, reads, meta })
    }

    /// A fresh `app` table showing `app`, with no reads yet.
    fn open(&self, lua: &Lua, app: &Rc<AppData>) -> mlua::Result<Table> {
        *self.app.borrow_mut() = app.clone();
        *self.reads.borrow_mut() = Reads::Keys(BTreeSet::new());
        let table = lua.create_table()?;
        table.set_metatable(Some(self.meta.clone()))?;
        Ok(table)
    }

    /// What the view read since [`Lens::open`].
    fn reads(&self) -> Reads {
        self.reads.borrow().clone()
    }
}

/// Where a mod asks to be placed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    /// The region the shell draws this mod in, if any.
    pub region: Option<String>,
    /// Position within the region; lower comes first, then by name.
    pub order: i64,
    pub title: Option<String>,
    /// Offered as a kind of tab: tab bars mount an instance of it per tab.
    pub tab: Option<TabKind>,
}

/// What `main.luau` returns: `mod { init, update, view, ... }`; or, for a
/// component that keeps state, its `init`, `update` and `build`.
struct Definition {
    init: Option<Function>,
    update: Option<Function>,
    view: Function,
    /// A component's `build(props, state, app)`, called in place of a mod's
    /// `view(state, app, props)`.
    component: bool,
    /// Topics it hears: names, or prefixes ending in `*`.
    subscribe: Vec<String>,
    /// Rules for what this mod draws, read when it renders so color names
    /// follow the current stylesheet.
    styles: Option<Value>,
    /// The window the shell asks for; only `main.luau` may set it.
    window: Option<Chrome>,
    /// The shell's menus and About panel; `main.luau` only, too.
    menus: Option<Menus>,
    about: Option<About>,
    meta: Meta,
}

impl Read for Definition {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        if !matches!(value, Value::Table(_)) {
            return Err(cx.fail("main.luau must return mod { ... }"));
        }
        let fields = Fields::read(value, cx, &[&[
            "init",
            "update",
            "view",
            "region",
            "order",
            "title",
            "styles",
            "window",
            "subscribe",
            "tab",
            "menus",
            "about",
        ]])?;
        Ok(Self {
            init: fields.get("init")?,
            update: fields.get("update")?,
            view: fields.need("view")?,
            component: false,
            subscribe: fields.get("subscribe")?.unwrap_or_default(),
            styles: fields.raw("styles").cloned(),
            window: fields.get("window")?,
            menus: fields.get("menus")?,
            about: fields.get("about")?,
            meta: Meta {
                region: fields.get("region")?,
                order: fields.get("order")?.unwrap_or(0),
                title: fields.get("title")?,
                tab: fields.get("tab")?,
            },
        })
    }
}

/// A message `update` sends on: `{ to = "mod", msg = ... }`.
pub(crate) struct Send {
    pub(crate) to: String,
    pub(crate) msg: Data,
}

impl Read for Send {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&["to", "msg"]])?;
        Ok(Self {
            to: fields.need("to")?,
            msg: fields.get("msg")?.unwrap_or(Data::Nil),
        })
    }
}

pub(crate) struct Module {
    lua: Lua,
    meter: Meter,
    lens: Lens,
    definition: Definition,
    state: Value,
    /// What an instance was mounted with; nil for a mod in its own place.
    props: Value,
    /// What `init` sent, for the workspace to deliver once it is loaded.
    pending: Vec<Send>,
}

impl Module {
    /// Runs `source` and its `init`. `previous` is the state the module had
    /// before a reload, so an edit does not lose what the user was doing.
    pub(crate) fn load(
        name: &str,
        source: &str,
        previous: Option<&Data>,
        props: Option<&Data>,
        library: &Library,
    ) -> Result<Self, Error> {
        let meter = Meter::default();
        let lua = vm::open(name, &meter, library)?;
        let value = vm::text(&lua, source, name).eval::<Value>()?;
        let definition = Definition::read(&value, &Cx::root(&NO_NAMES))?;
        if definition.window.is_some() && name != SHELL {
            return Err(Error::new(
                "window: only the shell (main.luau) sets the window",
            ));
        }
        if (definition.menus.is_some() || definition.about.is_some()) && name != SHELL {
            return Err(Error::new(
                "menus and about: only the shell (main.luau) sets them",
            ));
        }
        Self::start(lua, meter, definition, previous, props)
    }

    /// An instance of the component `component`, which keeps state: it has
    /// `update`, and its `build` takes `(props, state, app)`.
    pub(crate) fn load_component(
        name: &str,
        component: &str,
        previous: Option<&Data>,
        props: Option<&Data>,
        library: &Library,
    ) -> Result<Self, Error> {
        let meter = Meter::default();
        let lua = vm::open(name, &meter, library)?;
        let table = library.definition(&lua, component)?;
        let update = table.get::<Option<Function>>("update")?;
        if update.is_none() {
            return Err(Error::new(format!(
                "component `{component}` keeps no state (it has no `update`), so it is not mounted with an id"
            )));
        }
        let definition = Definition {
            init: table.get("init")?,
            update,
            view: table.get("build")?,
            component: true,
            subscribe: table
                .get::<Option<Vec<String>>>("subscribe")?
                .unwrap_or_default(),
            styles: table.get::<Option<Value>>("styles")?,
            window: None,
            menus: None,
            about: None,
            meta: Meta::default(),
        };
        Self::start(lua, meter, definition, previous, props)
    }

    fn start(
        lua: Lua,
        meter: Meter,
        definition: Definition,
        previous: Option<&Data>,
        props: Option<&Data>,
    ) -> Result<Self, Error> {
        let previous = match previous {
            Some(previous) => previous.to_lua(&lua)?,
            None => Value::Nil,
        };
        let props = match props {
            Some(props) => props.to_lua(&lua)?,
            None => Value::Nil,
        };
        // `init` returns the state, and optionally sends, as `update` does:
        // a tab bar announces its tabs as it starts.
        let (state, pending) = match &definition.init {
            Some(init) => {
                meter.reset();
                let (state, sends) = init
                    .call::<(Value, Value)>((previous, props.clone()))
                    .map_err(|error| Error::from(error).context("init"))?;
                let sends = Option::<Vec<Send>>::read(&sends, &Cx::root(&NO_NAMES).key("sends"))
                    .map_err(|error| error.context("init"))?;
                (state, sends.unwrap_or_default())
            },
            None => (previous, Vec::new()),
        };
        Ok(Self {
            pending,
            lens: Lens::new(&lua)?,
            lua,
            meter,
            definition,
            state,
            props,
        })
    }

    /// Whether it hears the topic `name`: listed by name, or under a prefix
    /// ending in `*` (`tabs/*`).
    pub(crate) fn subscribes(&self, name: &str) -> bool {
        self.definition
            .subscribe
            .iter()
            .any(|pattern| match pattern.strip_suffix('*') {
                Some(prefix) => name.starts_with(prefix),
                None => pattern == name,
            })
    }

    /// What `init` sent and is not yet delivered.
    pub(crate) fn take_pending(&mut self) -> Vec<Send> {
        std::mem::take(&mut self.pending)
    }

    /// What the instance is drawn with now, for its next `update` and view.
    pub(crate) fn set_props(&mut self, props: &Data) -> Result<(), Error> {
        self.props = props.to_lua(&self.lua)?;
        Ok(())
    }

    pub(crate) fn meta(&self) -> &Meta {
        &self.definition.meta
    }

    /// The window the shell asks for, if it says.
    pub(crate) fn window(&self) -> Option<Chrome> {
        self.definition.window
    }

    /// The menus the shell asks for.
    pub(crate) fn menus(&self) -> &[AppMenu] {
        self.definition.menus.as_ref().map_or(&[], |menus| &menus.0)
    }

    /// What the shell's About panel says.
    pub(crate) fn about(&self) -> Option<&About> {
        self.definition.about.as_ref()
    }

    /// Runs `update(state, msg)`. It returns the new state, or nothing to
    /// keep the state it changed in place, and optionally a list of sends.
    pub(crate) fn update(&mut self, msg: &Data) -> Result<Vec<Send>, Error> {
        let Some(update) = &self.definition.update else {
            return Ok(Vec::new());
        };
        self.meter.reset();
        let (state, sends) = update
            .call::<(Value, Value)>((
                self.state.clone(),
                msg.to_lua(&self.lua)?,
                self.props.clone(),
            ))
            .map_err(|error| Error::from(error).context("update"))?;
        if !state.is_nil() {
            self.state = state;
        }
        let sends = Option::<Vec<Send>>::read(&sends, &Cx::root(&NO_NAMES).key("sends"))
            .map_err(|error| error.context("update"))?;
        Ok(sends.unwrap_or_default())
    }

    /// Runs `view(state, app)` and checks the tree it returns: the view,
    /// the problems of children marked in it, and the keys of `app` it read.
    /// `windows` are the rows each of its lists builds.
    pub(crate) fn view(
        &self,
        app: &Rc<AppData>,
        sheet: &Stylesheet,
        windows: &BTreeMap<String, ListWindow>,
    ) -> Result<(Node, Vec<Error>, Reads), Error> {
        self.meter.reset();
        let app = self.lens.open(&self.lua, app)?;
        let call = if self.definition.component {
            self.definition
                .view
                .call::<Value>((self.props.clone(), self.state.clone(), app))
        } else {
            self.definition
                .view
                .call::<Value>((self.state.clone(), app, self.props.clone()))
        };
        let tree = call.map_err(|error| Error::from(error).context("view"))?;
        // A list's rows are built as it is read, and may read `app` too.
        let (node, problems) =
            node::read(&tree, sheet, windows).map_err(|error| error.context("view"))?;
        Ok((
            node,
            problems
                .into_iter()
                .map(|problem| problem.context("view"))
                .collect(),
            self.lens.reads(),
        ))
    }

    /// The mod's own `styles`, read against the stylesheet's names.
    pub(crate) fn rules(&self, sheet: &Stylesheet) -> Result<Rules, Error> {
        let cx = Cx::root(sheet.names()).key("styles");
        Ok(
            Option::<Rules>::read(self.definition.styles.as_ref().unwrap_or(&Value::Nil), &cx)?
                .unwrap_or_default(),
        )
    }

    /// The state as plain data, to carry over a reload; `None` when it holds
    /// something that cannot cross, such as a function.
    pub(crate) fn state(&self) -> Option<Data> {
        Data::read(&self.state).ok()
    }
}
