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
        Action, Chrome, Command, Data, Error, Instance, InstanceOf, ListWindow, Node, Roots, Rules,
        Stylesheet, WindowState,
        component::{Component, Demo, Library},
        module::{AppData, Meta, Module, Reads},
        persist::{Persist, state_key},
        style,
        vm::{self, Meter},
    },
    mlua::{Table, Value},
    std::{
        collections::{BTreeMap, BTreeSet, VecDeque},
        fs,
        path::{Path, PathBuf},
        rc::Rc,
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
    /// Mod and component instances drawn in the last render, by key
    /// ([`Instance::key`]).
    instances: BTreeMap<String, Mounted>,
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
    outbox: Vec<(String, String, Data)>,
    /// Where state outlives the app; nothing is saved without it.
    persist: Option<Box<dyn Persist>>,
    /// The last value published on each topic (`to = "topic:<name>"`).
    topics: BTreeMap<String, Data>,
    /// What views last got as `app`.
    app: Rc<AppData>,
    /// When each key of `app` last changed, for views to tell whether
    /// what they read is still current.
    versions: Versions,
    /// Counts stylesheet loads: every view draws again after one.
    style_generation: u64,
}

/// A count for each key of `app`, raised when its value changes, and one
/// raised when any does.
#[derive(Default)]
struct Versions {
    keys: BTreeMap<String, u64>,
    any: u64,
}

impl Versions {
    /// Raises the count of every key whose value differs from `old` to `new`.
    fn compare(&mut self, old: &AppData, new: &AppData) {
        let gone = old.keys().filter(|key| !new.contains_key(*key));
        let changed = new
            .iter()
            .filter(|(key, value)| old.get(*key) != Some(value))
            .map(|(key, _)| key);
        let keys: Vec<String> = gone.chain(changed).cloned().collect();
        for key in keys {
            self.any += 1;
            self.keys.insert(key, self.any);
        }
    }

    /// Where `reads` stand now.
    fn stamp(&self, reads: &Reads) -> Stamp {
        match reads {
            Reads::All => Stamp::All(self.any),
            Reads::Keys(keys) => Stamp::Keys(
                keys.iter()
                    .map(|key| (key.clone(), self.keys.get(key).copied().unwrap_or(0)))
                    .collect(),
            ),
        }
    }

    /// Whether nothing `stamp` read has changed since.
    fn current(&self, stamp: &Stamp) -> bool {
        match stamp {
            Stamp::All(any) => *any == self.any,
            Stamp::Keys(keys) => keys
                .iter()
                .all(|(key, at)| self.keys.get(key).copied().unwrap_or(0) == *at),
        }
    }
}

/// The counts of the keys of `app` a view read, when it was drawn.
#[derive(Debug)]
enum Stamp {
    All(u64),
    Keys(Vec<(String, u64)>),
}

/// What a view was drawn from, besides its state and props.
#[derive(Debug)]
struct Drawn {
    style_generation: u64,
    stamp: Stamp,
}

/// An instance the app keeps: its module, and what it is an instance of.
struct Mounted {
    slot: Slot,
    of: InstanceOf,
    /// The module that drew it, which `to = "parent"` reaches.
    owner: String,
}

/// Instances one render may hold: each is a Luau state of its own.
const MAX_INSTANCES: usize = 256;

/// What an instance calls the module that drew it in a send's `to`.
pub const PARENT: &str = "parent";

/// How a message names a topic: `to = "topic:tabs/main"` publishes `msg`
/// to every module subscribed to `tabs/main`.
pub const TOPIC: &str = "topic:";

/// The topic a provider's data is published on: `provider/git`.
const PROVIDER_TOPIC: &str = "provider/";

/// Longest a topic name can be.
const MAX_TOPIC: usize = 128;

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
    /// The state saved from an earlier run, for its first load.
    restored: Option<Data>,
    /// The state last saved, so an unchanged one is not saved again.
    saved: Option<Data>,
    /// What an instance is drawn with; nil for a mod in its own place.
    props: Data,
    /// Loaded since its topics were last handed to it.
    fresh: bool,
    /// Its state, props or lists changed since it was last drawn, or it
    /// was never drawn: its view must run again.
    dirty: bool,
    /// What its view read when last drawn; while that is current and it is
    /// not `dirty`, drawing again would draw the same.
    drawn: Option<Drawn>,
    /// Which rows each of its lists builds, by the list's `id`.
    lists: BTreeMap<String, ListWindow>,
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
        // A reload keeps the running state; a first load takes the saved one.
        let previous = self
            .module
            .as_ref()
            .and_then(Module::state)
            .or_else(|| self.restored.take());
        let props = (self.props != Data::Nil).then_some(&self.props);
        let loaded = Module::load(name, source, previous.as_ref(), props, library);
        match loaded {
            Ok(module) => {
                self.module = Some(module);
                self.fresh = true;
                self.dirty = true;
                self.load_error = None;
                self.update_error = None;
            },
            Err(error) => self.load_error = Some(error),
        }
    }

    /// Loads an instance of the component `component`, keeping its state
    /// across reloads like a mod's.
    fn load_component(&mut self, key: &str, component: &str, library: &Library) {
        self.source = library.path(component);
        let previous = self
            .module
            .as_ref()
            .and_then(Module::state)
            .or_else(|| self.restored.take());
        let props = (self.props != Data::Nil).then_some(&self.props);
        match Module::load_component(key, component, previous.as_ref(), props, library) {
            Ok(module) => {
                self.module = Some(module);
                self.fresh = true;
                self.dirty = true;
                self.load_error = None;
                self.update_error = None;
            },
            Err(error) => self.load_error = Some(error),
        }
    }

    /// Draws it with `props` from now on.
    fn set_props(&mut self, props: &Data) {
        if self.props == *props {
            return;
        }
        self.props = props.clone();
        self.dirty = true;
        if let Some(module) = &mut self.module
            && let Err(error) = module.set_props(props)
        {
            self.view_error = Some(error);
        }
    }

    /// Runs its view, unless nothing it was drawn from has changed.
    fn render(&mut self, app: &Rc<AppData>, versions: &Versions, sheet: &Stylesheet, style: u64) {
        let Some(module) = &self.module else {
            return;
        };
        let current = self
            .drawn
            .as_ref()
            .is_some_and(|drawn| drawn.style_generation == style && versions.current(&drawn.stamp));
        if current && !self.dirty {
            return;
        }
        self.dirty = false;
        match module
            .rules(sheet)
            .and_then(|rules| Ok((rules, module.view(app, sheet, &self.lists)?)))
        {
            Ok((rules, (view, broken, reads))) => {
                self.rules = rules;
                self.view = Some(view);
                self.broken = broken;
                self.view_error = None;
                self.drawn = Some(Drawn {
                    style_generation: style,
                    stamp: versions.stamp(&reads),
                });
            },
            Err(error) => {
                self.view_error = Some(error);
                // Tried again on the next render, whatever changes.
                self.drawn = None;
            },
        }
    }
}

impl Workspace {
    /// Loads the app from `roots`. Never fails: problems are kept on each
    /// module's [`Slot`] for the window to show.
    pub fn open(roots: impl Into<Roots>, dark: bool) -> Self {
        Self::open_with(roots, dark, None)
    }

    /// The same, restoring each mod's state from `persist` and saving it
    /// there as it changes.
    pub fn open_with(
        roots: impl Into<Roots>,
        dark: bool,
        persist: Option<Box<dyn Persist>>,
    ) -> Self {
        let mut workspace = Self {
            roots: roots.into(),
            library: Library::default(),
            components: Vec::new(),
            shell: Slot::default(),
            mods: BTreeMap::new(),
            demos: BTreeMap::new(),
            instances: BTreeMap::new(),
            stylesheet: Stylesheet::default(),
            style_error: None,
            dark,
            window: WindowState::default(),
            host: BTreeMap::new(),
            host_problems: BTreeMap::new(),
            outbox: Vec::new(),
            topics: persist
                .as_ref()
                .map(|persist| persist.topics())
                .unwrap_or_default(),
            persist,
            app: Rc::new(AppData::new()),
            versions: Versions::default(),
            style_generation: 0,
        };
        workspace.shell.restored = workspace.restore(SHELL);
        workspace.reload();
        workspace
    }

    /// The state `module` saved in an earlier run.
    fn restore(&self, module: &str) -> Option<Data> {
        self.persist.as_ref()?.state(&state_key(module))
    }

    /// Saves every module's state that changed since it was last saved.
    fn save(&mut self) {
        let Some(persist) = &self.persist else {
            return;
        };
        let slots = std::iter::once((SHELL, &mut self.shell))
            .chain(
                self.mods
                    .iter_mut()
                    .map(|(name, slot)| (name.as_str(), slot)),
            )
            // A preview's instances are as passing as the preview.
            .chain(
                self.instances
                    .iter_mut()
                    .filter(|(_, mounted)| !mounted.owner.starts_with(DEMO))
                    .map(|(key, mounted)| (key.as_str(), &mut mounted.slot)),
            );
        for (name, slot) in slots {
            let Some(state) = slot.module.as_ref().and_then(Module::state) else {
                continue;
            };
            if slot.saved.as_ref() != Some(&state) {
                persist.save_state(&state_key(name), &state);
                slot.saved = Some(state);
            }
        }
    }

    /// Saves what changed and waits until it is kept: for quitting.
    pub fn flush(&mut self) {
        self.save();
        if let Some(persist) = &self.persist {
            persist.flush();
        }
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
        self.style_generation += 1;
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
                self.shell.load_layered(SHELL, &candidates, &self.library);
            },
            None => {
                self.shell.load_error = Some(Error::new(format!("no main.luau in {}", self.roots)));
            },
        }
        let found = self.roots.folders("mods", "main.luau");
        for name in found.keys() {
            if !self.mods.contains_key(name) {
                let restored = self.restore(name);
                self.mods.entry(name.clone()).or_default().restored = restored;
            }
            let Some(slot) = self.mods.get_mut(name) else {
                continue;
            };
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
        // Instances run the new source too, keeping their state.
        let keys: Vec<String> = self.instances.keys().cloned().collect();
        for key in keys {
            self.load_instance(&key);
        }
        self.settle();
    }

    /// Loads (or reloads) the instance at `key` from its mod or component.
    fn load_instance(&mut self, key: &str) {
        let Some(mounted) = self.instances.get_mut(key) else {
            return;
        };
        match &mounted.of {
            InstanceOf::Mod(name) => {
                let candidates = self.roots.mod_candidates(name);
                if candidates.is_empty() {
                    mounted.slot.load_error = Some(Error::new(format!("no mod named `{name}`")));
                } else {
                    mounted.slot.load_layered(key, &candidates, &self.library);
                }
            },
            InstanceOf::Component(name) => {
                let name = name.clone();
                mounted.slot.load_component(key, &name, &self.library);
            },
        }
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
        // Pushed to whoever subscribes to `provider/<key>`, then drawn.
        let topic = format!("{PROVIDER_TOPIC}{key}");
        let deliveries = self.deliveries(&topic, &value);
        self.host.insert(key.to_owned(), value);
        self.host_problems.remove(key);
        self.process(deliveries.into());
        self.settle();
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
    /// `(provider, sending module, message)`.
    pub fn take_outbox(&mut self) -> Vec<(String, String, Data)> {
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
        } else if let Some(mounted) = self.instances.get(name) {
            Some(&mounted.slot)
        } else if name.starts_with(DEMO) {
            self.demos.get(name)
        } else {
            self.mods.get(name)
        }
    }

    fn slot_mut(&mut self, name: &str) -> Option<&mut Slot> {
        if name == SHELL {
            Some(&mut self.shell)
        } else if let Some(mounted) = self.instances.get_mut(name) {
            Some(&mut mounted.slot)
        } else if name.starts_with(DEMO) {
            self.demos.get_mut(name)
        } else {
            self.mods.get_mut(name)
        }
    }

    /// The instance `instance`, drawn by `owner`, and its key as the app
    /// keeps it: what the window draws in its place.
    pub fn instance(&self, owner: &str, instance: &Instance) -> Option<(&str, &Slot)> {
        self.instances
            .get_key_value(&instance.key(owner))
            .map(|(key, mounted)| (key.as_str(), &mounted.slot))
    }

    /// Who a send from `from` to `to` reaches: `parent` is the module that
    /// drew an instance, and `tabbed#main` names an instance the sender drew
    /// before an instance of the app's of that name.
    fn resolve(&self, from: &str, to: &str) -> String {
        if to == PARENT
            && let Some(mounted) = self.instances.get(from)
        {
            return mounted.owner.clone();
        }
        if to.contains('#') && !to.contains('/') {
            let own = format!("{from}/{to}");
            if self.instances.contains_key(&own) {
                return own;
            }
        }
        to.to_owned()
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
            .chain(
                self.instances
                    .iter()
                    .filter(|(_, mounted)| !mounted.owner.starts_with(DEMO))
                    .map(|(key, mounted)| (key.as_str(), &mounted.slot)),
            )
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
            // The app runs these on the browser it keeps, and opens windows.
            Action::Browser { .. } | Action::Window(_) => None,
            Action::Send { to, msg } => {
                self.send(from, to.as_deref().unwrap_or(from), msg.clone());
                None
            },
        }
    }

    /// Delivers `msg` to the module `to`, then whatever it sends on, and
    /// draws every module again.
    pub fn send(&mut self, from: &str, to: &str, msg: Data) {
        self.process(VecDeque::from([(from.to_owned(), to.to_owned(), msg)]));
        self.settle();
    }

    /// Delivers each message in `queue`, and what the modules send on.
    fn process(&mut self, mut queue: VecDeque<(String, String, Data)>) {
        let mut delivered = 0;
        while let Some((from, to, msg)) = queue.pop_front() {
            let to = self.resolve(&from, &to);
            // A preview, and every instance it drew at any depth, keeps to
            // itself: no other mod, provider or topic hears it.
            let root = self.root_owner(&from);
            if root.starts_with(DEMO) && to != from && to != root && !self.owned_by(&to, &root) {
                continue;
            }
            if let Some(provider) = to.strip_prefix(PROVIDER) {
                self.outbox.push((provider.to_owned(), from.clone(), msg));
                continue;
            }
            if let Some(topic) = to.strip_prefix(TOPIC) {
                match self.publish(topic, msg) {
                    Ok(deliveries) => queue.extend(deliveries),
                    Err(error) => {
                        if let Some(slot) = self.slot_mut(&from) {
                            slot.update_error = Some(error);
                        }
                    },
                }
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
                dirty,
                ..
            }) = self.slot_mut(&to)
            else {
                if let Some(slot) = self.slot_mut(&from) {
                    slot.update_error = Some(Error::new(format!("no mod named `{to}`")));
                }
                continue;
            };
            // `update` may change the state in place, so the view runs again.
            *dirty = true;
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
    }

    /// Keeps `value` as the topic `name`'s, saves it, and returns a message
    /// for each module that hears it.
    fn publish(&mut self, name: &str, value: Data) -> Result<Vec<(String, String, Data)>, Error> {
        let plain = !name.is_empty()
            && name.len() <= MAX_TOPIC
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'));
        if !plain || name.starts_with(PROVIDER_TOPIC) {
            return Err(Error::new(format!(
                "`topic:{name}`: a topic is 1 to {MAX_TOPIC} of a-z, A-Z, 0-9, - _ . /, and provider/ topics are the providers'"
            )));
        }
        let deliveries = self.deliveries(name, &value);
        if let Some(persist) = &self.persist {
            persist.save_topic(name, &value);
        }
        self.topics.insert(name.to_owned(), value);
        Ok(deliveries)
    }

    /// A message for each module subscribed to the topic `name`, carrying
    /// `value`. Previews hear nothing: they keep to themselves.
    fn deliveries(&self, name: &str, value: &Data) -> Vec<(String, String, Data)> {
        self.listeners()
            .filter(|(_, slot)| {
                slot.module
                    .as_ref()
                    .is_some_and(|module| module.subscribes(name))
            })
            .map(|(key, _)| {
                (
                    format!("{TOPIC}{name}"),
                    key.to_owned(),
                    topic_message(name, value),
                )
            })
            .collect()
    }

    /// Every module that may subscribe: the shell, the mods, and the
    /// instances not drawn by a preview.
    fn listeners(&self) -> impl Iterator<Item = (&str, &Slot)> {
        std::iter::once((SHELL, &self.shell))
            .chain(self.mods.iter().map(|(name, slot)| (name.as_str(), slot)))
            .chain(
                self.instances
                    .iter()
                    .filter(|(_, mounted)| !mounted.owner.starts_with(DEMO))
                    .map(|(key, mounted)| (key.as_str(), &mounted.slot)),
            )
    }

    /// Draws everything, hands each module loaded since the last time the
    /// current value of the topics it hears (so one that joins late starts
    /// in step), draws again if that changed anything, and saves state.
    fn settle(&mut self) {
        // Each round can mount instances, which then hear their topics.
        for _ in 0..4 {
            self.render();
            // What each module loaded since the last round hears now.
            let catch_up: VecDeque<(String, String, Data)> = self
                .listeners()
                .filter(|(_, slot)| slot.fresh)
                .filter_map(|(key, slot)| Some((key, slot.module.as_ref()?)))
                .flat_map(|(key, module)| {
                    let topics = self
                        .topics
                        .iter()
                        .map(|(name, value)| (name.clone(), value));
                    let providers = self
                        .host
                        .iter()
                        .map(|(name, value)| (format!("{PROVIDER_TOPIC}{name}"), value));
                    topics
                        .chain(providers)
                        .filter(|(name, _)| module.subscribes(name))
                        .map(|(name, value)| {
                            (
                                format!("{TOPIC}{name}"),
                                key.to_owned(),
                                topic_message(&name, value),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            let fresh: Vec<String> = self
                .listeners()
                .filter(|(_, slot)| slot.fresh)
                .map(|(key, _)| key.to_owned())
                .collect();
            for key in fresh {
                if let Some(slot) = self.slot_mut(&key) {
                    slot.fresh = false;
                }
            }
            // What modules sent from `init`, after the catch-up.
            let mut catch_up = catch_up;
            catch_up.extend(self.take_pending());
            if catch_up.is_empty() {
                break;
            }
            self.process(catch_up);
        }
        self.save();
    }

    /// What every `view` gets as `app`: what the host shares, the mods,
    /// which regions they asked for, and the theme.
    fn app(&self) -> AppData {
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
        app.insert("kinds".to_owned(), self.kinds());
        app.insert(
            "looks".to_owned(),
            Data::Map(
                self.stylesheet
                    .looks
                    .iter()
                    .map(|(kind, look)| (kind.clone(), Data::from(look.as_str())))
                    .collect(),
            ),
        );
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
        app
    }

    /// What a tab bar's `+` menu offers, `app.kinds`: each component and mod
    /// with a `tab` entry, as `{ kind = "component:terminal" | "mod:explorer",
    /// title, icon, props, report }`, by title.
    fn kinds(&self) -> Data {
        let components = self.components.iter().filter_map(|component| {
            Some((
                format!("component:{}", component.name),
                &component.title,
                component.tab.as_ref()?,
            ))
        });
        let mods = self.mods.iter().filter_map(|(name, slot)| {
            let meta = slot.meta()?;
            Some((
                format!("mod:{name}"),
                meta.title.as_ref().unwrap_or(name),
                meta.tab.as_ref()?,
            ))
        });
        let mut kinds: Vec<(String, Data)> = components
            .chain(mods)
            .map(|(kind, title, tab)| {
                let title = tab.title.clone().unwrap_or_else(|| title.clone());
                let mut fields = BTreeMap::from([
                    ("kind".to_owned(), Data::from(kind.as_str())),
                    ("title".to_owned(), Data::from(title.as_str())),
                    ("props".to_owned(), tab.props.clone()),
                ]);
                if let Some(icon) = &tab.icon {
                    fields.insert("icon".to_owned(), Data::from(icon.name()));
                }
                if let Some(report) = &tab.report {
                    fields.insert("report".to_owned(), Data::from(report.as_str()));
                }
                (title, Data::Map(fields))
            })
            .collect();
        kinds.sort_by(|a, b| a.0.cmp(&b.0));
        Data::List(kinds.into_iter().map(|(_, kind)| kind).collect())
    }

    /// Runs every view whose state, props, lists or `app` keys it read
    /// changed since it was last drawn; the others keep their view.
    fn render(&mut self) {
        let app = self.app();
        if app != *self.app {
            self.versions.compare(&self.app, &app);
            self.app = Rc::new(app);
        }
        let app = self.app.clone();
        let style = self.style_generation;
        self.shell
            .render(&app, &self.versions, &self.stylesheet, style);
        for slot in self.mods.values_mut().chain(self.demos.values_mut()) {
            slot.render(&app, &self.versions, &self.stylesheet, style);
        }
        self.render_instances(&app);
    }

    /// Has the list `id` drawn by `owner` build the rows in `window`, as the
    /// window scrolls it, and draws again. Whether that changed anything.
    pub fn set_list_window(&mut self, owner: &str, id: &str, window: ListWindow) -> bool {
        let Some(slot) = self.slot_mut(owner) else {
            return false;
        };
        if slot.lists.get(id) == Some(&window) {
            return false;
        }
        slot.lists.insert(id.to_owned(), window);
        slot.dirty = true;
        self.settle();
        true
    }

    /// What every module sent from `init` and is not yet delivered.
    fn take_pending(&mut self) -> Vec<(String, String, Data)> {
        let slots = std::iter::once((SHELL, &mut self.shell))
            .chain(
                self.mods
                    .iter_mut()
                    .map(|(name, slot)| (name.as_str(), slot)),
            )
            .chain(
                self.demos
                    .iter_mut()
                    .map(|(name, slot)| (name.as_str(), slot)),
            )
            .chain(
                self.instances
                    .iter_mut()
                    .map(|(key, mounted)| (key.as_str(), &mut mounted.slot)),
            );
        let mut sends = Vec::new();
        for (key, slot) in slots {
            if let Some(module) = &mut slot.module {
                sends.extend(
                    module
                        .take_pending()
                        .into_iter()
                        .map(|send| (key.to_owned(), send.to, send.msg)),
                );
            }
        }
        sends
    }

    /// The module at the top of `key`'s chain of owners: itself, for a mod.
    fn root_owner(&self, key: &str) -> String {
        let mut at = key;
        while let Some(mounted) = self.instances.get(at) {
            at = &mounted.owner;
        }
        at.to_owned()
    }

    /// Whether `key` is an instance drawn, at any depth, by `owner`.
    fn owned_by(&self, key: &str, owner: &str) -> bool {
        let mut at = key;
        while let Some(mounted) = self.instances.get(at) {
            if mounted.owner == owner {
                return true;
            }
            at = &mounted.owner;
        }
        false
    }

    /// Draws every instance the views hold: creating the new ones (with the
    /// state they saved), handing each its props, and following instances
    /// drawn inside instances. Those no longer drawn are dropped; their
    /// state stays saved, so they resume where they were if drawn again.
    fn render_instances(&mut self, app: &Rc<AppData>) {
        let mut queue: Vec<(String, Instance)> = Vec::new();
        let collect = |owner: &str, view: Option<&Node>, queue: &mut Vec<(String, Instance)>| {
            if let Some(view) = view {
                view.instances(&mut |instance| queue.push((owner.to_owned(), instance.clone())));
            }
        };
        collect(SHELL, self.shell.view(), &mut queue);
        for (name, slot) in self.mods.iter().chain(&self.demos) {
            collect(name, slot.view(), &mut queue);
        }
        let mut drawn = BTreeSet::new();
        while let Some((owner, instance)) = queue.pop() {
            let key = instance.key(&owner);
            if drawn.contains(&key) {
                continue;
            }
            if drawn.len() == MAX_INSTANCES {
                if let Some(slot) = self.slot_mut(&owner) {
                    slot.view_error = Some(Error::new(format!(
                        "more than {MAX_INSTANCES} instances; is an id made from data?"
                    )));
                }
                break;
            }
            drawn.insert(key.clone());
            if !self.instances.contains_key(&key) {
                let restored = if owner.starts_with(DEMO) {
                    None
                } else {
                    self.restore(&key)
                };
                let slot = Slot {
                    restored,
                    props: instance.props.clone(),
                    ..Slot::default()
                };
                self.instances.insert(key.clone(), Mounted {
                    slot,
                    of: instance.of.clone(),
                    owner: owner.clone(),
                });
                self.load_instance(&key);
            }
            let Some(mounted) = self.instances.get_mut(&key) else {
                continue;
            };
            mounted.slot.set_props(&instance.props);
            mounted
                .slot
                .render(app, &self.versions, &self.stylesheet, self.style_generation);
            collect(&key, mounted.slot.view(), &mut queue);
        }
        // Instances in hidden `lazy` panes sleep: kept as they are if they
        // were started, with what they mount, but neither drawn nor started.
        let mut asleep: Vec<(String, Instance)> = Vec::new();
        let doze = |owner: &str, view: Option<&Node>, queue: &mut Vec<(String, Instance)>| {
            if let Some(view) = view {
                view.asleep(&mut |instance| queue.push((owner.to_owned(), instance.clone())));
            }
        };
        doze(SHELL, self.shell.view(), &mut asleep);
        for (name, slot) in self.mods.iter().chain(&self.demos) {
            doze(name, slot.view(), &mut asleep);
        }
        for (key, mounted) in &self.instances {
            if drawn.contains(key) {
                doze(key, mounted.slot.view(), &mut asleep);
            }
        }
        let mut kept = BTreeSet::new();
        while let Some((owner, instance)) = asleep.pop() {
            let key = instance.key(&owner);
            if drawn.contains(&key) || !kept.insert(key.clone()) {
                continue;
            }
            if let Some(mounted) = self.instances.get(&key) {
                // What it mounts, awake or asleep in its own view, sleeps too.
                collect(&key, mounted.slot.view(), &mut asleep);
                doze(&key, mounted.slot.view(), &mut asleep);
            }
        }
        self.instances
            .retain(|key, _| drawn.contains(key) || kept.contains(key));
    }
}

/// How deep each stylesheet key merges when one layer goes over another:
/// a color or speed replaces the one of its name, and a rule's properties
/// replace those of the same rule, so a user's `panel = { radius = 4 }`
/// keeps the app's panel background, and `mono = { size = 14 }` keeps its
/// family. Any other key replaces the value below.
const MERGED: &[(&str, usize)] = &[
    ("colors", 1),
    ("speeds", 1),
    ("looks", 1),
    ("mono", 1),
    ("rules", 2),
];

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

/// What a module hears on a topic: `{ topic = "<name>", value = ... }`.
fn topic_message(name: &str, value: &Data) -> Data {
    Data::Map(
        [
            ("topic".to_owned(), Data::from(name)),
            ("value".to_owned(), value.clone()),
        ]
        .into(),
    )
}

fn valid_name(name: &str) -> bool {
    name != SHELL && style::valid_name(name)
}
