//! Providers: modules that produce data for Luau views, where Luau cannot.
//! A provider is a folder `providers/<name>/` in one of the app's
//! [`Roots`], holding `provider.toml`:
//!
//! - **wasm**: a WebAssembly component built against `wit/provider.wit`,
//!   run sandboxed: no files, network, or environment, a memory cap, and an
//!   instruction budget per call. What it may ask the host for is listed
//!   under `capabilities`. It ships as `provider.wasm`, or as the Rust crate
//!   it is built from: when the folder has a `Cargo.toml` and Cargo is
//!   installed, the host builds the crate, and rebuilds it as it changes.
//!   The sandbox covers the running component only: building runs Cargo,
//!   and the crate's proc macros, with the user's access, so a wasm
//!   provider's source is as trusted as a process provider. The host
//!   refuses the provider's own build hooks (a build script, in it or in a
//!   local crate it uses, and a toolchain named by path) and runs Cargo
//!   outside its folders, so no Cargo config there applies; proc macros and
//!   published dependencies' build scripts still run. A provider from
//!   elsewhere ships its source, which can be reviewed; packages are
//!   refused with any `provider.wasm`. The runtime is the `wasm` feature:
//!   without it a wasm provider publishes nothing and fails only when a mod
//!   messages it.
//! - **native**: Rust built into the app (see [`native`]), for git, forges
//!   and the like: the user's own access, real libraries, and replies to
//!   the mod that asked.
//! - **static**: a JSON file in the folder (`file = "sample.json"`),
//!   published as it is, again when it changes and on any message: sample
//!   data with no code at all.
//! - **process**: any command that prints one JSON value per line and reads
//!   messages from stdin, each with the mod that sent it. It runs with the user's full access, for data a
//!   sandbox cannot reach. It sees stdin close when it should leave.
//! - **socket**: the same lines over a Unix socket, to a program that runs
//!   on its own (see [`socket`]): the app connects, connects again when the
//!   link drops, and leaves the program running when it goes.
//!
//! How a process's or socket's queue stands, and whether a socket is
//! linked, appears to every view as `app.links.<name>`.
//!
//! What a provider publishes appears to every view as `app.<name>`; a mod
//! sends it a message with `to = "provider:<name>"`. In the app a provider
//! starts once something wants it (see [`Providers::want`]), unless its
//! manifest says `start = "always"`. Each provider runs on a
//! thread of its own, so a slow one never holds up the window, and a reload
//! restarts only the providers whose folders changed.

#[cfg(feature = "tokio")]
mod asynchronous;
mod fingerprint;
mod fixed;
mod json;
pub mod lines;
mod manifest;
mod mods;
pub mod native;
mod outbox;
mod process;
pub mod program;
#[cfg(unix)]
mod socket;
mod stream;
#[cfg(feature = "wasm")]
mod system;
#[cfg(feature = "wasm")]
mod wasm;
#[cfg(not(feature = "wasm"))]
#[path = "wasm_left_out.rs"]
mod wasm;

#[cfg(feature = "tokio")]
pub use {
    asynchronous::{AsyncNative, native as async_native, on_runtime, runtime, set_runtime},
    tokio,
};
pub use {
    json::{from_json, to_json},
    manifest::{Capability, Kind, Manifest, PREBUILT, Publish, Restart, Start},
    mods::{Mods, owner},
    native::{Handle, Native, Natives},
};

use {
    fingerprint::Fingerprint,
    moonframe_core::{Data, Error, Roots},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::{Path, PathBuf},
        sync::{
            Arc, Mutex, PoisonError,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    },
};

/// A message for a provider, and the mod that sent it: a process provider
/// sees both, so it can do what only one mod may ask for.
#[derive(Debug)]
pub(crate) struct Message {
    pub(crate) from: String,
    pub(crate) data: Data,
}

/// What a provider reports to the app.
#[derive(Debug)]
pub enum Event {
    /// Data to share as `app.<provider>`.
    Published { provider: String, data: Data },
    /// A `publish = "latest"` provider has something new: what to share is
    /// whatever [`Newest::take`] gives when this is handled.
    Newest { provider: String, newest: Newest },
    /// How a process or socket provider's link stands now.
    Link { provider: String, link: Link },
    /// The provider stopped or could not start; it shares nothing until it
    /// is reloaded.
    Failed { provider: String, problem: Error },
    /// A native provider's message for the mod `to`: a reply to an action.
    Sent {
        provider: String,
        to: String,
        msg: Data,
    },
}

/// Where events go: called on provider threads.
pub type Events = Arc<dyn Fn(Event) + Send + Sync>;

/// How a process or socket provider's link stands, for `app.links.<name>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Link {
    /// Whether a process or socket provider has its program.
    pub connected: Option<bool>,
    /// Messages not yet written.
    pub waiting: usize,
    /// Messages dropped since it started: the queue was full or, for a
    /// socket, there was no one to hear them.
    pub dropped: u64,
}

impl Link {
    /// As views read it.
    pub fn data(&self) -> Data {
        let mut map = BTreeMap::from([
            (
                "waiting".to_owned(),
                Data::Integer(i64::try_from(self.waiting).unwrap_or(i64::MAX)),
            ),
            (
                "dropped".to_owned(),
                Data::Integer(i64::try_from(self.dropped).unwrap_or(i64::MAX)),
            ),
        ]);
        if let Some(connected) = self.connected {
            map.insert("connected".to_owned(), Data::Bool(connected));
        }
        Data::Map(map)
    }
}

/// The newest of a `publish = "latest"` provider's data.
#[derive(Clone, Debug, Default)]
pub struct Newest(Arc<Mutex<Option<Data>>>);

impl Newest {
    /// Takes it, if a publish left any.
    pub fn take(&self) -> Option<Data> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).take()
    }

    /// Keeps `data` over what was there; whether that was nothing, so
    /// the app has yet to hear of it.
    fn put(&self, data: Data) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(data)
            .is_none()
    }
}

/// Publishes for one provider, from any of its threads.
#[derive(Clone)]
pub(crate) struct Publisher {
    pub(crate) name: String,
    events: Events,
    /// Where the data goes if the provider is `publish = "latest"`.
    newest: Option<Newest>,
}

impl Publisher {
    fn new(name: String, events: Events, publish: Publish) -> Self {
        Self {
            name,
            events,
            newest: (publish == Publish::Latest).then(Newest::default),
        }
    }

    pub(crate) fn of(context: &Context) -> Self {
        context.publisher.clone()
    }

    pub(crate) fn publish(&self, data: Data) {
        let provider = self.name.clone();
        match &self.newest {
            None => (self.events)(Event::Published { provider, data }),
            Some(newest) => {
                if newest.put(data) {
                    (self.events)(Event::Newest {
                        provider,
                        newest: newest.clone(),
                    });
                }
            },
        }
    }
}

/// The providers of an app, each running on its own thread.
/// The profile this run serves, if any, which process providers see as
/// `MOONFRAME_PROFILE`; set once at launch with [`set_profile`].
static PROFILE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Tells process providers which profile (app) they serve.
pub fn set_profile(name: &str) {
    let _ = PROFILE.set(name.to_owned());
}

/// The profile this run serves, if any: for a native provider keeping
/// state of its own per app.
pub fn profile() -> Option<&'static str> {
    PROFILE.get().map(String::as_str)
}

/// The folder of what a bundled app carries besides itself, which process
/// providers see as `MOONFRAME_PAYLOAD`; set once at launch.
static PAYLOAD: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Tells process providers where the bundle's `payload` folder is.
pub fn set_payload(dir: &Path) {
    if dir.is_dir() {
        let _ = PAYLOAD.set(dir.to_owned());
    }
}

/// The folder the app was started from, which process providers see as
/// `MOONFRAME_LAUNCH_DIR`; set once at launch with [`set_launch_dir`].
static LAUNCH_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Records the folder the app was started from. `/` (an app opened from
/// the Finder or the Dock) names none and is not kept.
pub fn set_launch_dir(dir: &Path) {
    if dir.is_dir() && dir.parent().is_some() {
        let _ = LAUNCH_DIR.set(dir.to_owned());
    }
}

/// The folder the app was started from (`moonframe` run in a shell), if
/// any: where a terminal starts when nothing names another folder.
pub fn launch_dir() -> Option<&'static Path> {
    LAUNCH_DIR.get().map(PathBuf::as_path)
}

/// The project the user works in, as the app last found it: the git
/// repository of the terminal they last used. Set with [`set_project`].
static PROJECT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Every project the app has followed, in this launch or (as the app
/// restores them with [`remember_projects`]) an earlier one.
static PROJECTS: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

/// Records the project the user works in now; returns whether it changed.
/// [`Providers::tell_project`] then tells the running providers.
pub fn set_project(dir: &Path) -> bool {
    remember_projects([dir.to_owned()]);
    let Ok(mut project) = PROJECT.lock() else {
        return false;
    };
    if project.as_deref() == Some(dir) {
        return false;
    }
    *project = Some(dir.to_owned());
    true
}

/// Adds `dirs` to the projects the app has followed: those of earlier
/// launches, which a tab restored from then may still name.
pub fn remember_projects(dirs: impl IntoIterator<Item = PathBuf>) {
    if let Ok(mut projects) = PROJECTS.lock() {
        projects.extend(dirs);
    }
}

/// Whether the app has followed `dir` as a project, now or in a launch it
/// remembers: a folder the user worked in, never one a mod made up.
pub fn followed(dir: &Path) -> bool {
    PROJECTS.lock().is_ok_and(|projects| projects.contains(dir))
}

/// The project the user works in (the git repository of the terminal they
/// last used), once the app found one: what the files, git and docs
/// providers show, unless the user named a folder for them. Process
/// providers see it as `MOONFRAME_PROJECT` when they start, and as a
/// message `{ project = "<path>" }` from the app (an empty `from`, which no
/// mod has) when it changes.
pub fn project() -> Option<PathBuf> {
    PROJECT.lock().ok().and_then(|project| project.clone())
}

pub struct Providers {
    roots: Roots,
    /// Where wasm providers are built.
    build_dir: PathBuf,
    events: Events,
    running: BTreeMap<String, Running>,
    /// The native providers built into the app, by name.
    natives: Natives,
    /// Every provider in the roots, as last found.
    found: BTreeMap<String, (PathBuf, Result<Manifest, Error>)>,
    /// Which of them run, besides those that always do.
    wanted: Wanted,
}

/// Which providers something wants running.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Wanted {
    All,
    /// By name; one ending in `*` stands for every name it starts.
    Names(BTreeSet<String>),
}

impl Wanted {
    fn covers(&self, name: &str) -> bool {
        match self {
            Self::All => true,
            Self::Names(names) => names.iter().any(|wanted| match wanted.strip_suffix('*') {
                Some(prefix) => name.starts_with(prefix),
                None => wanted == name,
            }),
        }
    }
}

/// Past what a process was given to leave, what [`Providers::shutdown`]
/// allows for its thread to see it stop and for the last steps.
const SHUTDOWN_SLACK: Duration = Duration::from_millis(200);

/// A provider's thread, and how to reach and stop it.
struct Running {
    messages: mpsc::Sender<Message>,
    stop: Arc<AtomicBool>,
    /// Its folder when it started: a reload restarts it only if this
    /// changed. `None` once asked to restart, whatever the folder holds.
    fingerprint: Option<Fingerprint>,
    /// A process or native provider, which hears when the project changes.
    follows_project: bool,
    /// A process, and how long it may take to leave; set once it has.
    leaving: Option<(Duration, Arc<AtomicBool>)>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// What a provider's thread gets.
pub(crate) struct Context {
    pub(crate) name: String,
    pub(crate) dir: PathBuf,
    pub(crate) manifest: Manifest,
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))]
    pub(crate) build_dir: PathBuf,
    pub(crate) messages: mpsc::Receiver<Message>,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) events: Events,
    /// Set once the process this one replaces has left.
    pub(crate) replaces: Option<Arc<AtomicBool>>,
    publisher: Publisher,
}

impl Context {
    pub(crate) fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub(crate) fn publish(&self, data: Data) {
        self.publisher.publish(data);
    }

    pub(crate) fn fail(&self, problem: impl Into<String>) {
        (self.events)(Event::Failed {
            provider: self.name.clone(),
            problem: Error::new(problem),
        });
    }
}

impl Providers {
    /// Starts every provider in `roots`. Wasm providers build into
    /// `build_dir`.
    pub fn start(roots: Roots, build_dir: impl Into<PathBuf>, events: Events) -> Self {
        Self::start_with(roots, build_dir, events, Natives::new())
    }

    /// The same, with the native providers this build has.
    pub fn start_with(
        roots: Roots,
        build_dir: impl Into<PathBuf>,
        events: Events,
        natives: Natives,
    ) -> Self {
        Self::start_wanting(roots, build_dir, events, natives, None)
    }

    /// Starts only the providers in `wanted` (all of them when `None`) and
    /// those whose manifest says `start = "always"`; the others start once
    /// [`Providers::want`] names them, or a mod sends them a message.
    pub fn start_wanting(
        roots: Roots,
        build_dir: impl Into<PathBuf>,
        events: Events,
        natives: Natives,
        wanted: Option<BTreeSet<String>>,
    ) -> Self {
        let build_dir = build_dir.into();
        let mut providers = Self {
            roots,
            build_dir: std::path::absolute(&build_dir).unwrap_or(build_dir),
            events,
            running: BTreeMap::new(),
            natives,
            found: BTreeMap::new(),
            wanted: wanted.map_or(Wanted::All, Wanted::Names),
        };
        providers.reload();
        providers
    }

    /// Starts the providers in `wanted` that are not running yet (every
    /// one when `None`). Those started keep running: a pane hidden for a
    /// moment does not stop its data.
    pub fn want(&mut self, wanted: Option<BTreeSet<String>>) {
        let before = self.wanted.clone();
        match (&mut self.wanted, wanted) {
            (Wanted::All, _) => {},
            (all, None) => *all = Wanted::All,
            (Wanted::Names(names), Some(more)) => names.extend(more),
        }
        if self.wanted != before {
            self.sync();
        }
    }

    /// Whether the provider `name` is running.
    pub fn running(&self, name: &str) -> bool {
        self.running.contains_key(name)
    }

    /// Starts providers that appeared or whose folders changed, and stops
    /// those that went. The others keep running, and keep their state.
    pub fn reload(&mut self) {
        self.found = discover(&self.roots);
        self.sync();
    }

    /// Runs the providers found that are wanted, or always run, each from
    /// its folder as it is now.
    fn sync(&mut self) {
        let found = &self.found;
        self.running.retain(|name, _| found.contains_key(name));
        let found: Vec<(String, PathBuf, Result<Manifest, Error>)> = self
            .found
            .iter()
            .filter(|(name, (_, manifest))| {
                self.wanted.covers(name)
                    || manifest
                        .as_ref()
                        .is_ok_and(|manifest| manifest.start == Start::Always)
            })
            .map(|(name, (dir, manifest))| (name.clone(), dir.clone(), manifest.clone()))
            .collect();
        for (name, dir, manifest) in found {
            let fingerprint = Fingerprint::of(&dir);
            if self
                .running
                .get(&name)
                .is_some_and(|running| running.fingerprint == Some(fingerprint))
            {
                continue;
            }
            // Stops the old one, if any. Its process may take a while to
            // leave, and the new one must not start beside it.
            let replaces = self
                .running
                .remove(&name)
                .and_then(|running| running.leaving.as_ref().map(|(_, left)| left.clone()));
            let manifest = match manifest {
                Ok(manifest) => manifest,
                Err(problem) => {
                    (self.events)(Event::Failed {
                        provider: name,
                        problem,
                    });
                    continue;
                },
            };
            tracing::info!(provider = %name, dir = %dir.display(), "starting");
            let follows_project = matches!(manifest.kind, Kind::Process { .. } | Kind::Native);
            let (sender, messages) = mpsc::channel();
            let stop = Arc::new(AtomicBool::new(false));
            let left = Arc::new(AtomicBool::new(false));
            let leaving = matches!(manifest.kind, Kind::Process { .. })
                .then(|| (manifest.stop_grace + process::TERM_GRACE, left.clone()));
            let publisher = Publisher::new(name.clone(), self.events.clone(), manifest.publish);
            let context = Context {
                name: name.clone(),
                dir,
                manifest,
                build_dir: self.build_dir.clone(),
                messages,
                stop: stop.clone(),
                events: self.events.clone(),
                replaces,
                publisher,
            };
            let native = self.natives.get(name.as_str()).cloned();
            let spawned = thread::Builder::new()
                .name(format!("provider-{name}"))
                .spawn(move || {
                    match context.manifest.kind {
                        Kind::Wasm => wasm::run(&context),
                        Kind::Process { .. } => process::run(&context),
                        #[cfg(unix)]
                        Kind::Socket { .. } => socket::run(&context),
                        #[cfg(not(unix))]
                        Kind::Socket { .. } => {
                            context.fail("socket providers need a Unix socket");
                        },
                        Kind::Native => native::run(&context, native),
                        Kind::Static { .. } => fixed::run(&context),
                    }
                    left.store(true, Ordering::Relaxed);
                });
            match spawned {
                Ok(_) => {
                    self.running.insert(name, Running {
                        messages: sender,
                        stop,
                        fingerprint: Some(fingerprint),
                        follows_project,
                        leaving,
                    });
                },
                Err(error) => (self.events)(Event::Failed {
                    provider: name,
                    problem: Error::new(format!("cannot start a thread: {error}")),
                }),
            }
        }
    }

    /// Runs the providers of `roots` instead, restarting only those whose
    /// folders changed.
    pub fn set_roots(&mut self, roots: Roots) {
        self.roots = roots;
        self.reload();
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.running.keys().map(String::as_str)
    }

    /// Stops every provider, as the app quits, and waits for processes to
    /// leave, however long they were given to: without this they would
    /// only see stdin close as the app died, and one that ignores it
    /// would outlive it. All leave at once, so the wait is the longest
    /// one's. A socket provider's program is not the app's, and stays.
    pub fn shutdown(&mut self) {
        let leaving: Vec<(Duration, Arc<AtomicBool>)> = self
            .running
            .values()
            .filter_map(|running| running.leaving.clone())
            .collect();
        self.running.clear();
        let started = Instant::now();
        for (time, left) in leaving {
            while !left.load(Ordering::Relaxed) && started.elapsed() < time + SHUTDOWN_SLACK {
                thread::sleep(Duration::from_millis(5));
            }
        }
    }

    /// Starts the provider `name` again, or for the first time, however
    /// it stands: for a process that went and stays gone, once the user
    /// has dealt with what stopped it.
    pub fn restart(&mut self, name: &str) -> Result<(), Error> {
        if !self.found.contains_key(name) {
            return Err(Error::new(format!("no provider named `{name}`")));
        }
        if let Some(running) = self.running.get_mut(name) {
            running.fingerprint = None;
        }
        self.want(Some(BTreeSet::from([name.to_owned()])));
        self.sync();
        Ok(())
    }

    /// Hands `message`, from the mod `from`, to the provider `name`,
    /// starting it first if it is not running yet.
    pub fn send(&mut self, name: &str, from: &str, message: Data) -> Result<(), Error> {
        if !self.running.contains_key(name) && self.found.contains_key(name) {
            self.want(Some(BTreeSet::from([name.to_owned()])));
        }
        let running = self
            .running
            .get(name)
            .ok_or_else(|| Error::new(format!("no provider named `{name}`")))?;
        running
            .messages
            .send(Message {
                from: from.to_owned(),
                data: message,
            })
            .map_err(|_| Error::new(format!("provider `{name}` has stopped")))
    }
}

impl Providers {
    /// Tells the running process and native providers the project changed
    /// ([`set_project`]), as a message from the app itself: an empty
    /// `from`, which no mod has, so no mod can move a provider elsewhere.
    pub fn tell_project(&self) {
        let Some(project) = project() else {
            return;
        };
        let message = Data::Map(BTreeMap::from([(
            "project".to_owned(),
            Data::String(project.to_string_lossy().into_owned()),
        )]));
        for running in self
            .running
            .values()
            .filter(|running| running.follows_project)
        {
            let _ = running.messages.send(Message {
                from: String::new(),
                data: message.clone(),
            });
        }
    }
}

/// Reads every manifest without starting anything: what `--check` reports.
pub fn check(roots: &Roots) -> Vec<(String, Error)> {
    discover(roots)
        .into_iter()
        .filter_map(|(name, (_, manifest))| manifest.err().map(|problem| (name, problem)))
        .collect()
}

/// The name of every provider in `roots`, valid or not.
pub fn names(roots: &Roots) -> Vec<String> {
    discover(roots).into_keys().collect()
}

/// Each provider folder (one with a provider.toml) and its manifest, by
/// name; a higher layer's folder replaces a lower one's.
fn discover(roots: &Roots) -> BTreeMap<String, (PathBuf, Result<Manifest, Error>)> {
    roots
        .folders("providers", "provider.toml")
        .into_iter()
        .map(|(name, dir)| {
            let manifest = Manifest::load(&name, &dir);
            (name, (dir, manifest))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod link_tests;
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
