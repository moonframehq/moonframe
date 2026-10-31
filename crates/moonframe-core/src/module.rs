//! One loaded module: its own sealed Luau state, its definition, and its
//! state between calls.

use {
    crate::{
        Chrome, Data, Error, Node, Rules, SHELL, Stylesheet,
        component::Library,
        node,
        read::{Cx, Fields, NO_NAMES, Read},
        vm::{self, Meter},
    },
    mlua::{Function, Lua, Value},
    std::collections::BTreeMap,
};

/// Where a mod asks to be placed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta {
    /// The region the shell draws this mod in, if any.
    pub region: Option<String>,
    /// Position within the region; lower comes first, then by name.
    pub order: i64,
    pub title: Option<String>,
}

/// What `main.luau` returns: `mod { init, update, view, ... }`.
struct Definition {
    init: Option<Function>,
    update: Option<Function>,
    view: Function,
    /// Rules for what this mod draws, read when it renders so color names
    /// follow the current stylesheet.
    styles: Option<Value>,
    /// The window the shell asks for; only `main.luau` may set it.
    window: Option<Chrome>,
    meta: Meta,
    /// What of the state is kept across launches.
    persist: Option<Persist>,
}

/// What `persist` keeps: the whole state, or some of its keys.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Persist {
    All,
    Keys(Vec<String>),
}

impl Read for Persist {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Boolean(true) => Ok(Self::All),
            Value::Table(_) => Ok(Self::Keys(Vec::<String>::read(value, cx)?)),
            _ => Err(cx.fail("expected `true` or a list of state keys")),
        }
    }
}

impl Read for Definition {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        if !matches!(value, Value::Table(_)) {
            return Err(cx.fail("main.luau must return mod { ... }"));
        }
        let fields = Fields::read(value, cx, &[&[
            "init", "update", "view", "region", "order", "title", "styles", "window", "persist",
        ]])?;
        Ok(Self {
            init: fields.get("init")?,
            update: fields.get("update")?,
            view: fields.need("view")?,
            styles: fields.raw("styles").cloned(),
            window: fields.get("window")?,
            persist: match fields.raw("persist") {
                None | Some(Value::Nil | Value::Boolean(false)) => None,
                Some(_) => fields.get("persist")?,
            },
            meta: Meta {
                region: fields.get("region")?,
                order: fields.get("order")?.unwrap_or(0),
                title: fields.get("title")?,
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
    definition: Definition,
    state: Value,
}

impl Module {
    /// Runs `source` and its `init`. `previous` is the state the module had
    /// before a reload, so an edit does not lose what the user was doing.
    pub(crate) fn load(
        name: &str,
        source: &str,
        previous: Option<&Data>,
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
        let previous = match previous {
            Some(previous) => previous.to_lua(&lua)?,
            None => Value::Nil,
        };
        let state = match &definition.init {
            Some(init) => {
                meter.reset();
                init.call::<Value>(previous)
                    .map_err(|error| Error::from(error).context("init"))?
            },
            None => previous,
        };
        Ok(Self {
            lua,
            meter,
            definition,
            state,
        })
    }

    pub(crate) fn meta(&self) -> &Meta {
        &self.definition.meta
    }

    /// The window the shell asks for, if it says.
    pub(crate) fn window(&self) -> Option<Chrome> {
        self.definition.window
    }

    /// Runs `update(state, msg)`. It returns the new state, or nothing to
    /// keep the state it changed in place, and optionally a list of sends.
    pub(crate) fn update(&mut self, msg: &Data) -> Result<Vec<Send>, Error> {
        let Some(update) = &self.definition.update else {
            return Ok(Vec::new());
        };
        self.meter.reset();
        let (state, sends) = update
            .call::<(Value, Value)>((self.state.clone(), msg.to_lua(&self.lua)?))
            .map_err(|error| Error::from(error).context("update"))?;
        if !state.is_nil() {
            self.state = state;
        }
        let sends = Option::<Vec<Send>>::read(&sends, &Cx::root(&NO_NAMES).key("sends"))
            .map_err(|error| error.context("update"))?;
        Ok(sends.unwrap_or_default())
    }

    /// Runs `view(state, app)` and checks the tree it returns: the view, and
    /// the problems of children marked in it.
    pub(crate) fn view(&self, app: &Data, sheet: &Stylesheet) -> Result<(Node, Vec<Error>), Error> {
        self.meter.reset();
        let tree = self
            .definition
            .view
            .call::<Value>((self.state.clone(), app.to_lua(&self.lua)?))
            .map_err(|error| Error::from(error).context("view"))?;
        let (node, problems) = node::read(&tree, sheet).map_err(|error| error.context("view"))?;
        Ok((
            node,
            problems
                .into_iter()
                .map(|problem| problem.context("view"))
                .collect(),
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

    /// What of the state to keep across launches, if the mod asks for it
    /// with `persist`: all of it, or the keys it names. `None` when it
    /// does not ask, or its state cannot cross.
    pub(crate) fn persisted(&self) -> Option<Data> {
        let persist = self.definition.persist.as_ref()?;
        let state = self.state()?;
        match (persist, state) {
            (Persist::All, state) => Some(state),
            (Persist::Keys(keys), Data::Map(fields)) => Some(Data::Map(
                fields
                    .into_iter()
                    .filter(|(key, _)| keys.contains(key))
                    .collect::<BTreeMap<_, _>>(),
            )),
            (Persist::Keys(_), _) => None,
        }
    }
}
