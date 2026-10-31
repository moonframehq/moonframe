//! An app's folders: the shell and its mods, loaded, reloaded, and
//! messaged. Each folder ([`Roots`]) is laid out as:
//!
//! ```text
//! app/
//!   main.luau            the shell: lays out the whole window
//!   style.luau           optional stylesheet: colors, radii, rules
//!   mods/
//!     explorer/main.luau a mod; the shell places it by name or region
//! ```

use {
    crate::{
        Action, Chrome, Command, Data, Error, Node, Roots, Rules, Stylesheet, WindowState,
        component::{Component, Demo, Library},
        module::{Meta, Module},
        style,
        vm::{self, Meter},
    },
    mlua::{Table, Value},
    std::{
        collections::{BTreeMap, BTreeSet, VecDeque},
        fs,
        path::{Path, PathBuf},
    },
};

/// The name messages use for the shell.
pub const SHELL: &str = "shell";

/// Messages one click may cause, counting those modules send on, so two mods
/// that answer each other cannot hang the window.
const MAX_MESSAGES: usize = 64;

pub struct Workspace {
    roots: Roots,
    /// The components mods can `use`.
    library: Library,
    /// The same, loaded on their own for the window's list.
    components: Vec<Component>,
    shell: Slot,
    mods: BTreeMap<String, Slot>,
    /// Each component's live preview, by its module name: `demo:<name>`.
    demos: BTreeMap<String, Slot>,
    stylesheet: Stylesheet,
    style_error: Option<Error>,
    dark: bool,
    /// The window as views see it, `app.window`.
    window: WindowState,
    /// What the host shares with every view under `app`, such as
    /// `app.system`; modules cannot read the machine themselves.
    host: BTreeMap<String, Data>,
    /// Why a source of shared data, such as a provider, has nothing to share.
    host_problems: BTreeMap<String, Error>,
    /// Messages for providers (`to = "provider:<name>"`), for the app to
    /// forward: providers run outside the workspace.
    outbox: Vec<(String, Data)>,
    /// State saved by an earlier launch, by module name, not yet loaded.
    saved: BTreeMap<String, Data>,
}

/// How a message names a provider rather than a mod.
pub const PROVIDER: &str = "provider:";

/// How a component's live preview is named among modules: `demo:tabs`. A
/// demo only talks to itself: its messages to other mods and providers, and
/// its app commands, go nowhere.
pub const DEMO: &str = "demo:";

/// A module and what it last drew. A module that fails to load keeps
/// running its previous version, so a broken edit shows its error beside
/// the last good view instead of blanking the window.
#[derive(Default)]
pub struct Slot {
    module: Option<Module>,
    /// State saved by an earlier launch, for `init` when the module first
    /// loads; dropped once it has.
    restored: Option<Data>,
    /// The file it was last loaded from.
    source: Option<PathBuf>,
    view: Option<Node>,
    /// The mod's own `styles`, which apply only to what it draws.
    rules: Rules,
    load_error: Option<Error>,
    update_error: Option<Error>,
    view_error: Option<Error>,
    /// Children of the view that could not be read, each marked in place.
    broken: Vec<Error>,
}

impl Slot {
    pub fn view(&self) -> Option<&Node> {
        self.view.as_ref()
    }

    pub fn error(&self) -> Option<&Error> {
        self.load_error
            .as_ref()
            .or(self.update_error.as_ref())
            .or(self.view_error.as_ref())
    }

    /// What is wrong with the children marked broken in the view.
    pub fn broken(&self) -> &[Error] {
        &self.broken
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    pub fn meta(&self) -> Option<&Meta> {
        self.module.as_ref().map(Module::meta)
    }

    /// The file the module comes from.
    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    /// Loads the first of `paths` (highest layer first) that works. A module
    /// that never loaded falls back to the same file in a lower layer, the
    /// app's own, so a broken copy in the user's folder shows the built-in
    /// version under its error rather than nothing. One that loaded before
    /// keeps its last good version, as always.
    fn load_layered(&mut self, name: &str, paths: &[PathBuf], library: &Library) {
        let Some((top, lower)) = paths.split_first() else {
            return;
        };
        self.load(name, top, library);
        if self.module.is_some() || self.load_error.is_none() {
            return;
        }
        let Some(error) = self.load_error.take() else {
            return;
        };
        for path in lower {
            self.load(name, path, library);
            if self.module.is_some() {
                self.load_error = Some(Error::new(format!(
                    "{}: {error} (showing {} until it loads)",
                    top.display(),
                    path.display()
                )));
                return;
            }
        }
        self.load_error = Some(error);
    }

    fn load(&mut self, name: &str, path: &Path, library: &Library) {
        match fs::read_to_string(path) {
            Ok(source) => self.load_source(name, &source, Some(path), library),
            Err(error) => {
                self.source = Some(path.to_owned());
                self.load_error = Some(Error::new(format!(
                    "cannot read {}: {error}",
                    path.display()
                )));
            },
        }
    }

    /// Loads `source`, which came from `path` if it is a file's.
    fn load_source(&mut self, name: &str, source: &str, path: Option<&Path>, library: &Library) {
        self.source = path.map(Path::to_owned);
        let previous = self
            .module
            .as_ref()
            .and_then(Module::state)
            .or_else(|| self.restored.clone());
        let loaded = Module::load(name, source, previous.as_ref(), library);
        match loaded {
            Ok(module) => {
                self.module = Some(module);
                self.restored = None;
                self.load_error = None;
                self.update_error = None;
            },
            Err(error) => self.load_error = Some(error),
        }
    }

    fn render(&mut self, app: &Data, sheet: &Stylesheet) {
        let Some(module) = &self.module else {
            return;
        };
        match module
            .rules(sheet)
            .and_then(|rules| Ok((rules, module.view(app, sheet)?)))
        {
            Ok((rules, (view, broken))) => {
                self.rules = rules;
                self.view = Some(view);
                self.broken = broken;
                self.view_error = None;
            },
            Err(error) => self.view_error = Some(error),
        }
    }
}

impl Workspace {
    /// Loads the app from `roots`. Never fails: problems are kept on each
    /// module's [`Slot`] for the window to show.
    pub fn open(roots: impl Into<Roots>, dark: bool) -> Self {
        Self::open_saved(roots, dark, BTreeMap::new())
    }

    /// The same, giving each module in `saved` (by name, the shell as
    /// [`SHELL`]) the state an earlier launch kept, as [`Self::persisted`]
    /// returned it: its `init` gets it as `previous`.
    pub fn open_saved(roots: impl Into<Roots>, dark: bool, saved: BTreeMap<String, Data>) -> Self {
        let mut workspace = Self {
            roots: roots.into(),
            library: Library::default(),
            components: Vec::new(),
            shell: Slot::default(),
            mods: BTreeMap::new(),
            demos: BTreeMap::new(),
            stylesheet: Stylesheet::default(),
            style_error: None,
            dark,
            window: WindowState::default(),
            host: BTreeMap::new(),
            host_problems: BTreeMap::new(),
            outbox: Vec::new(),
            saved,
        };
        workspace.reload();
        workspace
    }

    /// What the shell and each mod ask to keep across launches (`persist`),
    /// by module name, for [`Self::open_saved`] next time. Saved state of a
    /// mod that has not loaded yet is kept, so a broken mod loses nothing.
    pub fn persisted(&self) -> BTreeMap<String, Data> {
        let mut persisted: BTreeMap<String, Data> = self
            .mods
            .iter()
            .filter_map(|(name, slot)| match &slot.module {
                Some(module) => Some((name.clone(), module.persisted()?)),
                None => Some((name.clone(), slot.restored.clone()?)),
            })
            .collect();
        match &self.shell.module {
            Some(module) => {
                if let Some(state) = module.persisted() {
                    persisted.insert(SHELL.to_owned(), state);
                }
            },
            None => {
                if let Some(state) = &self.shell.restored {
                    persisted.insert(SHELL.to_owned(), state.clone());
                }
            },
        }
        // Mods not found this launch keep what they saved.
        for (name, state) in &self.saved {
            persisted
                .entry(name.clone())
                .or_insert_with(|| state.clone());
        }
        persisted
    }

    pub fn roots(&self) -> &Roots {
        &self.roots
    }

    /// Loads the app from `roots` instead, such as when a package is
    /// installed, keeping the state of mods that stay.
    pub fn set_roots(&mut self, roots: Roots) {
        self.roots = roots;
        self.reload();
    }

    /// Reads every module again, adds mods that appeared, drops those that
    /// went, and keeps each module's state across the reload.
    pub fn reload(&mut self) {
        self.load_style();
        self.library = Library::read(self.roots.entries("components", "luau"));
        self.components = self.library.catalogue(&self.stylesheet);
        let mut rules = Rules::default();
        for component in &self.components {
            rules.layer(&component.rules);
        }
        self.stylesheet.components = rules;
        self.load_demos();
        match self.roots.file("main.luau") {
            Some(_) => {
                let candidates = self.roots.candidates("main.luau");
                if self.shell.module.is_none() {
                    self.shell.restored = self.saved.remove(SHELL);
                }
                self.shell.load_layered(SHELL, &candidates, &self.library);
            },
            None => {
                self.shell.load_error = Some(Error::new(format!("no main.luau in {}", self.roots)));
            },
        }
        let found = self.roots.folders("mods", "main.luau");
        for name in found.keys() {
            let slot = self.mods.entry(name.clone()).or_default();
            if slot.module.is_none()
                && let Some(saved) = self.saved.remove(name)
            {
                slot.restored = Some(saved);
            }
            if valid_name(name) {
                // Highest layer first, so the folder found above leads.
                let candidates = self.roots.mod_candidates(name);
                slot.load_layered(name, &candidates, &self.library);
            } else {
                slot.load_error = Some(Error::new(format!(
                    "`{name}` is not a mod name: use a-z, 0-9, - and _, and not `{SHELL}`"
                )));
            }
        }
        self.mods.retain(|name, _| found.contains_key(name));
        self.render();
    }

    /// Reads every layer's `style.luau`, each laid over the ones below it
    /// ([`layer`]); without one the app uses no rules. A broken stylesheet
    /// keeps the previous one, like a broken module.
    fn load_style(&mut self) {
        let files = self.roots.files("style.luau");
        if files.is_empty() {
            self.stylesheet = Stylesheet::default();
            self.style_error = None;
            return;
        }
        let loaded = (|| {
            let meter = Meter::default();
            let lua = vm::open("style", &meter, &Library::default())?;
            let merged = lua.create_table()?;
            for path in &files {
                let within = |error: Error| Error::new(format!("{}: {error}", path.display()));
                let source = fs::read_to_string(path)
                    .map_err(|error| Error::new(format!("cannot read: {error}")))
                    .map_err(within)?;
                let value = vm::text(&lua, &source, "style")
                    .eval::<Value>()
                    .map_err(|error| within(error.into()))?;
                match value {
                    Value::Table(sheet) => layer(&merged, &sheet)?,
                    // Reading it says what is wrong.
                    other => return Stylesheet::read(&other).map_err(within),
                }
            }
            Stylesheet::read(&Value::Table(merged))
        })();
        match loaded {
            Ok(sheet) => {
                self.stylesheet = sheet;
                self.style_error = None;
            },
            Err(error) => self.style_error = Some(error),
        }
    }

    /// Every component, for the window's list.
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    pub fn stylesheet(&self) -> &Stylesheet {
        &self.stylesheet
    }

    /// Shares `value` with every view as `app.<key>`, and draws them again.
    pub fn set_host(&mut self, key: &str, value: Data) {
        self.host.insert(key.to_owned(), value);
        self.host_problems.remove(key);
        self.render();
    }

    /// Stops sharing `app.<key>`, with the reason when there is one.
    pub fn clear_host(&mut self, key: &str, problem: Option<Error>) {
        self.host.remove(key);
        match problem {
            Some(problem) => self.host_problems.insert(key.to_owned(), problem),
            None => self.host_problems.remove(key),
        };
        self.render();
    }

    /// Why each source of shared data has nothing to share.
    pub fn host_problems(&self) -> impl Iterator<Item = (&str, &Error)> {
        self.host_problems
            .iter()
            .map(|(key, problem)| (key.as_str(), problem))
    }

    /// Messages mods sent to providers since the last call, as
    /// `(provider, message)`.
    pub fn take_outbox(&mut self) -> Vec<(String, Data)> {
        std::mem::take(&mut self.outbox)
    }

    pub fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
        self.render();
    }

    /// What the shell asks of the window; the system title bar when it
    /// says nothing or fails to load.
    pub fn chrome(&self) -> Chrome {
        self.shell
            .module
            .as_ref()
            .and_then(Module::window)
            .unwrap_or_default()
    }

    /// Shares the window's state as `app.window`, drawing the views again
    /// when it changed.
    pub fn set_window(&mut self, window: WindowState) {
        if self.window != window {
            self.window = window;
            self.render();
        }
    }

    pub fn shell(&self) -> &Slot {
        &self.shell
    }

    /// The shell for [`SHELL`], a component's preview for `demo:<name>`,
    /// else the mod of that name.
    pub fn slot(&self, name: &str) -> Option<&Slot> {
        if name == SHELL {
            Some(&self.shell)
        } else if name.starts_with(DEMO) {
            self.demos.get(name)
        } else {
            self.mods.get(name)
        }
    }

    fn slot_mut(&mut self, name: &str) -> Option<&mut Slot> {
        if name == SHELL {
            Some(&mut self.shell)
        } else if name.starts_with(DEMO) {
            self.demos.get_mut(name)
        } else {
            self.mods.get_mut(name)
        }
    }

    /// Loads every component's demo, its file or one made from its example,
    /// keeping each one's state across reloads.
    fn load_demos(&mut self) {
        let files = self.roots.entries("components/demos", "luau");
        let mut found = BTreeSet::new();
        for component in &mut self.components {
            let demo = match files.get(&component.name) {
                Some(path) => match fs::read_to_string(path) {
                    Ok(source) => Some(Demo {
                        source,
                        path: Some(path.clone()),
                    }),
                    Err(error) => {
                        component.preview = Err(Error::new(format!(
                            "cannot read {}: {error}",
                            path.display()
                        )));
                        None
                    },
                },
                None => component.demo.take(),
            };
            component.demo = demo;
            let Some(demo) = &component.demo else {
                continue;
            };
            let name = format!("{DEMO}{}", component.name);
            found.insert(name.clone());
            let slot = self.demos.entry(name.clone()).or_default();
            slot.load_source(&name, &demo.source, demo.path.as_deref(), &self.library);
        }
        self.demos.retain(|name, _| found.contains(name));
    }

    /// A component's live preview, and the module name its controls send
    /// from.
    pub fn demo(&self, component: &str) -> Option<(&str, &Slot)> {
        self.demos
            .get_key_value(&format!("{DEMO}{component}"))
            .map(|(name, slot)| (name.as_str(), slot))
    }

    /// The mods that asked for `region`, in order.
    pub fn region(&self, region: &str) -> Vec<&str> {
        let mut members: Vec<(i64, &str)> = self
            .mods
            .iter()
            .filter_map(|(name, slot)| {
                let meta = slot.meta()?;
                (meta.region.as_deref() == Some(region)).then_some((meta.order, name.as_str()))
            })
            .collect();
        members.sort();
        members.into_iter().map(|(_, name)| name).collect()
    }

    /// Every problem: the stylesheet's, then the shell's, then the mods'.
    pub fn problems(&self) -> Vec<(&str, &Error)> {
        let modules = std::iter::once((SHELL, &self.shell))
            .chain(self.mods.iter().map(|(name, slot)| (name.as_str(), slot)))
            .flat_map(|(name, slot)| {
                slot.error()
                    .into_iter()
                    .chain(slot.broken())
                    .map(move |error| (name, error))
            });
        let components = self.components.iter().filter_map(|component| {
            component
                .preview
                .as_ref()
                .err()
                .map(|error| (component.path_name(), error))
        });
        // A demo's problem names its file, or the component it was made for.
        let demos = self.components.iter().filter_map(|component| {
            let (_, slot) = self.demo(&component.name)?;
            let error = slot.error()?;
            let name = match component.demo.as_ref().and_then(|demo| demo.path.as_ref()) {
                Some(_) => component.demo_name(),
                None => component.path_name(),
            };
            Some((name, error))
        });
        self.style_error
            .iter()
            .map(|error| ("style", error))
            .chain(modules)
            .chain(components)
            .chain(demos)
            .chain(self.host_problems())
            .collect()
    }

    pub fn style_error(&self) -> Option<&Error> {
        self.style_error.as_ref()
    }

    pub fn mod_names(&self) -> impl Iterator<Item = &str> {
        self.mods.keys().map(String::as_str)
    }

    /// Runs what a control drawn by the module `from` asks for. Messages go
    /// to modules here; commands are the app's, so they are handed back.
    pub fn dispatch(&mut self, from: &str, action: &Action) -> Option<Command> {
        match action {
            // A preview never quits or closes the app.
            Action::Command(_) if from.starts_with(DEMO) => None,
            Action::Command(command) => Some(*command),
            // The app runs these on the browser it keeps.
            Action::Browser { .. } => None,
            Action::Send { to, msg } => {
                self.send(from, to.as_deref().unwrap_or(from), msg.clone());
                None
            },
        }
    }

    /// Delivers `msg` to the module `to`, then whatever it sends on, and
    /// draws every module again.
    pub fn send(&mut self, from: &str, to: &str, msg: Data) {
        let mut queue = VecDeque::from([(from.to_owned(), to.to_owned(), msg)]);
        let mut delivered = 0;
        while let Some((from, to, msg)) = queue.pop_front() {
            // A preview keeps to itself.
            if from.starts_with(DEMO) && to != from {
                continue;
            }
            if let Some(provider) = to.strip_prefix(PROVIDER) {
                self.outbox.push((provider.to_owned(), msg));
                continue;
            }
            delivered += 1;
            if delivered > MAX_MESSAGES {
                if let Some(slot) = self.slot_mut(&from) {
                    slot.update_error = Some(Error::new(format!(
                        "more than {MAX_MESSAGES} messages from one action; are mods sending in a loop?"
                    )));
                }
                break;
            }
            let Some(Slot {
                module: Some(module),
                update_error,
                ..
            }) = self.slot_mut(&to)
            else {
                if let Some(slot) = self.slot_mut(&from) {
                    slot.update_error = Some(Error::new(format!("no mod named `{to}`")));
                }
                continue;
            };
            match module.update(&msg) {
                Ok(sends) => {
                    *update_error = None;
                    queue.extend(
                        sends
                            .into_iter()
                            .map(|send| (to.clone(), send.to, send.msg)),
                    );
                },
                Err(error) => *update_error = Some(error),
            }
        }
        self.render();
    }

    /// What every `view` gets as `app`: what the host shares, the mods,
    /// which regions they asked for, and the theme.
    fn app(&self) -> Data {
        let mut regions: BTreeMap<String, Vec<Data>> = BTreeMap::new();
        let names: BTreeSet<&str> = self
            .mods
            .values()
            .filter_map(|slot| slot.meta()?.region.as_deref())
            .collect();
        for region in names {
            regions.insert(
                region.to_owned(),
                self.region(region).into_iter().map(Data::from).collect(),
            );
        }
        let mut app = self.host.clone();
        app.extend([
            ("dark".to_owned(), Data::Bool(self.dark)),
            ("window".to_owned(), self.window.data()),
            (
                "mods".to_owned(),
                Data::List(self.mod_names().map(Data::from).collect()),
            ),
            (
                "regions".to_owned(),
                Data::Map(
                    regions
                        .into_iter()
                        .map(|(name, members)| (name, Data::List(members)))
                        .collect(),
                ),
            ),
        ]);
        Data::Map(app)
    }

    fn render(&mut self) {
        let app = self.app();
        self.shell.render(&app, &self.stylesheet);
        for slot in self.mods.values_mut().chain(self.demos.values_mut()) {
            slot.render(&app, &self.stylesheet);
        }
    }
}

/// How deep each stylesheet key merges when one layer goes over another:
/// a color or speed replaces the one of its name, and a rule's properties
/// replace those of the same rule, so a user's `panel = { radius = 4 }`
/// keeps the app's panel background. Any other key replaces the value below.
const MERGED: &[(&str, usize)] = &[("colors", 1), ("speeds", 1), ("rules", 2)];

/// Lays the stylesheet `sheet` over `merged`.
fn layer(merged: &Table, sheet: &Table) -> mlua::Result<()> {
    for pair in sheet.pairs::<Value, Value>() {
        let (key, value) = pair?;
        let depth = key
            .as_string()
            .and_then(|key| key.to_str().ok())
            .and_then(|key| MERGED.iter().find(|(name, _)| *name == &*key))
            .map_or(0, |(_, depth)| *depth);
        merge(merged, key, value, depth)?;
    }
    Ok(())
}

/// Sets `into[key]` to `value`, merging tables `depth` levels down.
fn merge(into: &Table, key: Value, value: Value, depth: usize) -> mlua::Result<()> {
    match (depth, into.raw_get::<Value>(key.clone())?, value) {
        (1.., Value::Table(below), Value::Table(above)) => {
            for pair in above.pairs::<Value, Value>() {
                let (key, value) = pair?;
                merge(&below, key, value, depth - 1)?;
            }
            Ok(())
        },
        (_, _, value) => into.raw_set(key, value),
    }
}

fn valid_name(name: &str) -> bool {
    name != SHELL && style::valid_name(name)
}
