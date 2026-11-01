//! One loaded module: its own sealed Luau state, its definition, and its
//! state between calls.

use {
    crate::{
        About, AppMenu, Chrome, Data, Error, Node, Rules, SHELL, Stylesheet,
        component::Library,
        menus::Menus,
        node,
        read::{Cx, Fields, NO_NAMES, Read},
        vm::{self, Meter},
    },
    mlua::{Function, Lua, Value},
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
            "init", "update", "view", "region", "order", "title", "styles", "window", "menus",
            "about",
        ]])?;
        Ok(Self {
            init: fields.get("init")?,
            update: fields.get("update")?,
            view: fields.need("view")?,
            styles: fields.raw("styles").cloned(),
            window: fields.get("window")?,
            menus: fields.get("menus")?,
            about: fields.get("about")?,
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
    /// `app` as a table in this module's Luau, and the generation of the
    /// workspace's `app` it was made from: views after a message see the
    /// same `app`, so it is converted once per change, not once per view.
    app: std::cell::RefCell<Option<(u64, Value)>>,
    /// The keys of `app` the last view read (`app.system`, `app.window`),
    /// so a provider's publish redraws only the views that read it. `None`
    /// until a view has run to the end.
    reads: std::cell::RefCell<Option<std::collections::BTreeSet<String>>>,
}

/// Wraps `app` so reading `app.<key>` notes the key in `reads`; iterating
/// and `#` see the real table. Views get it, never the table itself.
const WATCHED: &str = r#"
local real, reads = ...
return setmetatable({}, {
	__index = function(_, key)
		reads[key] = true
		return real[key]
	end,
	__iter = function()
		for key in real do
			reads[key] = true
		end
		return next, real
	end,
	__len = function()
		return #real
	end,
})
"#;

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
        if (definition.menus.is_some() || definition.about.is_some()) && name != SHELL {
            return Err(Error::new(
                "menus and about: only the shell (main.luau) sets them",
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
            app: std::cell::RefCell::new(None),
            reads: std::cell::RefCell::new(None),
        })
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
    pub(crate) fn view(
        &self,
        app: &Data,
        generation: u64,
        sheet: &Stylesheet,
    ) -> Result<(Node, Vec<Error>), Error> {
        self.meter.reset();
        let started = std::time::Instant::now();
        let cached = self
            .app
            .borrow()
            .as_ref()
            .filter(|(made, _)| *made == generation)
            .map(|(_, table)| table.clone());
        let table = match cached {
            Some(table) => table,
            None => {
                let table = app.to_lua(&self.lua)?;
                *self.app.borrow_mut() = Some((generation, table.clone()));
                table
            },
        };
        let converted = started.elapsed();
        let reads = self.lua.create_table()?;
        let watched = self
            .lua
            .load(WATCHED)
            .set_name("=app")
            .call::<Value>((table, reads.clone()))?;
        *self.reads.borrow_mut() = None;
        let tree = self
            .definition
            .view
            .call::<Value>((self.state.clone(), watched))
            .map_err(|error| Error::from(error).context("view"))?;
        *self.reads.borrow_mut() = Some(
            reads
                .pairs::<String, Value>()
                .filter_map(Result::ok)
                .map(|(key, _)| key)
                .collect(),
        );
        let ran = started.elapsed();
        let (node, problems) = node::read(&tree, sheet).map_err(|error| error.context("view"))?;
        tracing::trace!(
            target: "moonframe::perf",
            "view: app {converted:.1?}, luau {:.1?}, tree {:.1?}",
            ran - converted,
            started.elapsed() - ran,
        );
        Ok((
            node,
            problems
                .into_iter()
                .map(|problem| problem.context("view"))
                .collect(),
        ))
    }

    /// Whether the last view read `app.<key>`; true when it is not known.
    pub(crate) fn reads(&self, key: &str) -> bool {
        self.reads
            .borrow()
            .as_ref()
            .is_none_or(|reads| reads.contains(key))
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
