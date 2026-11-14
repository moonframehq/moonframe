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

// keep-sorted start
mod context;
mod diagnose;
mod load;
mod messages;
mod render;
mod scope;
mod slot;
mod windows;
// keep-sorted end

pub(crate) use scope::read_style;
pub use {context::ViewContext, diagnose::ViewDiagnostics};

use {
    crate::{
        Appearance, Chrome, Data, Error, Instance, InstanceOf, ListWindow, Node, Request, Roots,
        Rules, Stylesheet, WindowState,
        component::{Component, Library},
        module::{AppData, Meta, Module, Reads},
        persist::{Persist, state_key},
    },
    std::{
        collections::{BTreeMap, BTreeSet, VecDeque},
        path::{Path, PathBuf},
        rc::Rc,
        time::Instant,
    },
};

/// The mod the Developer tools are: docked beside the window or in a
/// window of its own, with the source and state of what is picked.
pub const DEVTOOLS: &str = "devtools";
/// The mod that asks agents for a change (the sparkle), whose runs are the
/// Developer tools' agent sessions.
pub const FEEDBACK: &str = "feedback";
/// The window `feedback` opens its dialog in: an app that keeps `feedback`
/// gets it too.
pub const FEEDBACK_WINDOW: &str = "feedback-window";

/// The name messages use for the shell.
pub const SHELL: &str = "shell";

/// Messages one click may cause, counting those modules send on, so two mods
/// that answer each other cannot hang the window.
const MAX_MESSAGES: usize = 64;

/// Every message one click may cause, including those to providers and
/// topics, which do not run an `update` here but still cost: a provider's
/// queue fills, a topic is saved.
const MAX_SENDS: usize = 1024;

/// The most messages one `update` may send.
const MAX_SENDS_PER_UPDATE: usize = 256;

pub struct Workspace {
    entry_statuses: Option<crate::entries::EntryStatuses>,
    /// Hears each new element problem, any module's.
    view_diagnostics: Option<ViewDiagnostics>,
    /// What was told of each module's view problems.
    told: BTreeMap<String, diagnose::Told>,
    /// Set while reloading, which reports every entry at its end.
    reloading: bool,
    /// Whether views are read strictly, as a check reads them.
    strict: bool,
    /// Where entries' views are published as they draw, and the draw each
    /// was published at.
    shared: Option<(crate::entries::Entries, BTreeMap<String, u64>)>,
    roots: Roots,
    /// The components mods can `use`.
    library: Library,
    /// Each miniapp entry's own components and stylesheet, by mod name.
    scopes: BTreeMap<String, scope::Scope>,
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
    /// The languages the user prefers, most preferred first, and the
    /// app's words in the best of them.
    languages: Vec<unic_langid::LanguageIdentifier>,
    words: crate::words::Words,
    /// Why some of the app's messages did not load.
    words_errors: Vec<Error>,
    dark: bool,
    /// The appearance the user chose, and what the app adds about it
    /// (the themes), as views see them in `app.appearance`.
    appearance: (Appearance, BTreeMap<String, Data>),
    /// The window as views see it, `app.window`.
    window: WindowState,
    /// Open mod windows, whose views see their own state instead of the shell's.
    mod_windows: BTreeMap<String, WindowState>,
    /// Compile-time host capabilities; separate from provider-published data.
    capabilities: BTreeMap<String, bool>,
    /// What the host shares with every view under `app`, such as
    /// `app.system`; modules cannot read the machine themselves.
    host: BTreeMap<String, Data>,
    /// Why a source of shared data, such as a provider, has nothing to share.
    host_problems: BTreeMap<String, Error>,
    /// Messages for providers (`to = "provider:<name>"`), for the app to
    /// forward: providers run outside the workspace.
    outbox: Vec<(String, String, Data)>,
    /// What modules asked of the app itself (`to = "app:<what>"`), with
    /// who asked, for the app to carry out.
    requests: Vec<(String, Request)>,
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
    /// Counts the views run, so a caller can tell whether any was.
    views_run: usize,
    /// Whether the component demos are drawn (the Components window is
    /// open); otherwise they are not run.
    demos_shown: bool,
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

impl Stamp {
    fn reads(&self, key: &str) -> bool {
        match self {
            Self::All(_) => true,
            Self::Keys(keys) => keys.iter().any(|(name, _)| name == key),
        }
    }
}

/// What a view was drawn from, besides its state and props.
#[derive(Debug)]
struct Drawn {
    style_generation: u64,
    stamp: Stamp,
    window: Option<WindowState>,
}

/// An instance the app keeps: its module, and what it is an instance of.
struct Mounted {
    slot: Slot,
    of: InstanceOf,
    /// The module that drew it, which `to = "parent"` reaches.
    owner: String,
}

/// Why a module's state was not kept: it holds what cannot be saved.
fn lost_state(error: Error) -> Error {
    error.context("state is not kept across reloads or launches")
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
/// Topics only the app itself publishes: `app/inspect`.
const APP_TOPIC: &str = "app/";

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
    /// The last working module until the new module's first view succeeds.
    rollback: Option<Module>,
    module: Option<Module>,
    /// The file it was last loaded from.
    source: Option<PathBuf>,
    view: Option<Node>,
    /// The mod's own `styles`, which apply only to what it draws, over
    /// `scoped`.
    rules: Rules,
    /// Its miniapp entry's `style.luau` rules, when it belongs to one.
    scoped: Rules,
    load_error: Option<Error>,
    update_error: Option<Error>,
    view_error: Option<Error>,
    /// Children of the view that could not be read, each marked in place.
    broken: Vec<Error>,
    /// What the view drew that is noted but not marked.
    notes: Vec<Error>,
    /// Whether its view is read strictly, as a check reads it.
    strict: bool,
    /// Counts its loads, so problems told of an older source are told
    /// again.
    loads: u64,
    /// Counts the views it drew.
    draws: u64,
    /// The keys of `app` its last view read.
    reads: Reads,
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
        Self::open_with_capabilities(roots, dark, persist, BTreeMap::new())
    }

    /// Loads with the host's compiled capabilities already available to
    /// the very first view as `app.capabilities`, including headless checks.
    pub fn open_with_capabilities(
        roots: impl Into<Roots>,
        dark: bool,
        persist: Option<Box<dyn Persist>>,
        capabilities: BTreeMap<String, bool>,
    ) -> Self {
        let mut workspace = Self {
            entry_statuses: None,
            view_diagnostics: None,
            told: BTreeMap::new(),
            reloading: false,
            strict: false,
            shared: None,
            roots: roots.into(),
            library: Library::default(),
            scopes: BTreeMap::new(),
            components: Vec::new(),
            shell: Slot::default(),
            mods: BTreeMap::new(),
            demos: BTreeMap::new(),
            instances: BTreeMap::new(),
            stylesheet: Stylesheet::default(),
            style_error: None,
            languages: crate::words::requested(),
            words: crate::words::Words::default(),
            words_errors: Vec::new(),
            dark,
            window: WindowState::default(),
            mod_windows: BTreeMap::new(),
            capabilities,
            host: BTreeMap::new(),
            host_problems: BTreeMap::new(),
            outbox: Vec::new(),
            requests: Vec::new(),
            appearance: (Appearance::System, BTreeMap::new()),
            topics: persist
                .as_ref()
                .map(|persist| persist.topics())
                .unwrap_or_default(),
            persist,
            app: Rc::new(AppData::new()),
            versions: Versions::default(),
            style_generation: 0,
            views_run: 0,
            // Headless checks see every demo; a window turns them off
            // until its Components window opens.
            demos_shown: true,
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
            let state = match slot.module.as_ref().map(Module::state) {
                Some(Ok(state)) => state,
                Some(Err(error)) => {
                    slot.update_error.get_or_insert_with(|| lost_state(error));
                    continue;
                },
                None => continue,
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

    /// Shares `value` with every view as `app.<key>`, and draws again the
    /// views that read it or heard it; how many, so a window can skip a
    /// frame when none did.
    pub fn set_host(&mut self, key: &str, value: Data) -> usize {
        let (started, before) = (Instant::now(), self.views_run);
        // Pushed to whoever subscribes to `provider/<key>`, then drawn.
        let topic = format!("{PROVIDER_TOPIC}{key}");
        let deliveries = self.deliveries(&topic, &value);
        self.host.insert(key.to_owned(), value);
        self.host_problems.remove(key);
        self.process(deliveries.into());
        self.settle();
        let drawn = self.views_run - before;
        let elapsed = started.elapsed();
        crate::perf!(
            elapsed,
            "provider {key}: published, {drawn} views ran in {elapsed:.1?}"
        );
        drawn
    }

    /// Tells the module `to`, if it subscribes to the app's own topic
    /// `app/<name>` (which no mod can publish or forge), and draws again.
    /// Only `to` hears it, and it is not kept.
    pub fn app_topic(&mut self, name: &str, to: &str, value: Data) {
        let deliveries: VecDeque<_> = self
            .deliveries(&format!("{APP_TOPIC}{name}"), &value)
            .into_iter()
            .filter(|(_, key, _)| key == to)
            .collect();
        self.process(deliveries);
        self.settle();
    }

    /// Tells every module that subscribes to the app's own topic
    /// `app/<name>` (which no mod can publish or forge), and draws again.
    /// It is news, not kept: a module loading later does not hear it.
    pub fn app_news(&mut self, name: &str, value: Data) {
        let deliveries = self.deliveries(&format!("{APP_TOPIC}{name}"), &value);
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

    /// The providers something here wants running: those whose
    /// `app.<name>` a view read when last drawn, and those whose topic
    /// (`provider/<name>`) a module hears. `None` when one wants them all:
    /// a view that went through every key of `app`, or a module that hears
    /// every provider's topic. A name ending in `*` stands for every
    /// provider it starts. A provider no one wants need not start.
    pub fn wanted_providers(&self) -> Option<BTreeSet<String>> {
        let mut wanted = BTreeSet::new();
        let slots = std::iter::once(&self.shell)
            .chain(self.mods.values())
            .chain(self.instances.values().map(|mounted| &mounted.slot));
        for slot in slots {
            match slot.drawn.as_ref().map(|drawn| &drawn.stamp) {
                Some(Stamp::All(_)) => return None,
                Some(Stamp::Keys(keys)) => wanted.extend(keys.iter().map(|(key, _)| key.clone())),
                None => {},
            }
            let Some(module) = &slot.module else {
                continue;
            };
            for pattern in module.subscriptions() {
                match pattern.strip_suffix('*') {
                    // `provider/*`, `prov*`, `*`: every provider's topic.
                    Some(prefix) if PROVIDER_TOPIC.starts_with(prefix) => return None,
                    // `provider/gi*`: every provider whose name starts so.
                    Some(prefix) => {
                        if let Some(start) = prefix.strip_prefix(PROVIDER_TOPIC) {
                            wanted.insert(format!("{start}*"));
                        }
                    },
                    None => {
                        if let Some(name) = pattern.strip_prefix(PROVIDER_TOPIC) {
                            wanted.insert(name.to_owned());
                        }
                    },
                }
            }
        }
        Some(wanted)
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

    /// Makes `problem` the module `name`'s update error: what it asked of
    /// the app could not be done.
    pub fn refuse(&mut self, name: &str, problem: Error) {
        if let Some(slot) = self.slot_mut(name) {
            slot.update_error = Some(problem);
        }
    }

    /// What modules asked of the app since the last call, as `(sending
    /// module, request)`, in the order they asked.
    pub fn take_requests(&mut self) -> Vec<(String, Request)> {
        std::mem::take(&mut self.requests)
    }

    /// Shares the appearance the user chose as `app.appearance.mode`,
    /// beside `dark` (what the window is) and the app's `fields`, drawing
    /// the views again when it changed.
    pub fn set_appearance(&mut self, mode: Appearance, fields: BTreeMap<String, Data>) {
        let appearance = (mode, fields);
        if self.appearance != appearance {
            self.appearance = appearance;
            self.render();
        }
    }

    pub fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
        self.render();
    }

    /// The menus the shell adds to the menu bar.
    pub fn menus(&self) -> &[crate::AppMenu] {
        self.shell.module.as_ref().map_or(&[], Module::menus)
    }

    /// What the About panel shows, as the shell says.
    pub fn about(&self) -> crate::About {
        self.shell
            .module
            .as_ref()
            .and_then(Module::about)
            .cloned()
            .unwrap_or_default()
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
            .chain(self.words_errors.iter().map(|error| ("locales", error)))
            .chain(modules)
            .chain(components)
            .chain(demos)
            .chain(self.host_problems())
            .collect()
    }

    pub fn style_error(&self) -> Option<&Error> {
        self.style_error.as_ref()
    }

    /// The language the app's words are shown in, as a BCP 47 tag.
    pub fn locale(&self) -> String {
        self.words.locale()
    }

    /// Shows the app in the first of `tags` (BCP 47, most preferred first)
    /// it has words in, reloading when that changes the list. Tags that do
    /// not parse are skipped; none left means the system's languages.
    pub fn set_languages(&mut self, tags: &[&str]) {
        let mut languages: Vec<unic_langid::LanguageIdentifier> =
            tags.iter().filter_map(|tag| tag.parse().ok()).collect();
        if languages.is_empty() {
            languages = crate::words::requested();
        }
        if languages != self.languages {
            self.languages = languages;
            self.reload();
        }
    }

    /// The shell, each mod and each mounted copy of a mod (a tab's
    /// `editor#1`) with `keys` of its own: who a keystroke nothing nearer
    /// takes goes to. Every copy hears it, so the one on screen does.
    pub fn mod_keys(&self) -> impl Iterator<Item = (&str, &crate::Keys)> {
        std::iter::once((SHELL, &self.shell))
            .chain(self.mods.iter().map(|(name, slot)| (name.as_str(), slot)))
            .chain(
                self.instances
                    .iter()
                    .map(|(key, mounted)| (key.as_str(), &mounted.slot)),
            )
            .filter_map(|(name, slot)| Some((name, slot.keys()?)))
            .filter(|(_, keys)| !keys.is_empty())
    }

    pub fn mod_names(&self) -> impl Iterator<Item = &str> {
        self.mods.keys().map(String::as_str)
    }
}
