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
//!   refuses the most direct build hooks, a `build.rs` or a folder-local
//!   `.cargo/`. A provider from elsewhere ships its source, which can be
//!   reviewed; packages are refused with any `provider.wasm`.
//! - **native**: Rust built into the app (see [`native`]), for git, forges
//!   and the like: the user's own access, real libraries, and replies to
//!   the mod that asked.
//! - **process**: any command that prints one JSON value per line and reads
//!   messages from stdin, each with the mod that sent it. It runs with the user's full access, for data a
//!   sandbox cannot reach.
//!
//! What a provider publishes appears to every view as `app.<name>`; a mod
//! sends it a message with `to = "provider:<name>"`. Each provider runs on a
//! thread of its own, so a slow one never holds up the window, and a reload
//! restarts only the providers whose folders changed.

mod fingerprint;
mod json;
mod manifest;
pub mod native;
mod process;
mod system;
mod wasm;

pub use {
    json::{from_json, to_json},
    manifest::{Capability, Kind, Manifest, PREBUILT},
    native::{Handle, Native, Natives},
};

use {
    fingerprint::Fingerprint,
    moonframe_core::{Data, Error, Roots},
    std::{
        collections::BTreeMap,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
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

pub struct Providers {
    roots: Roots,
    /// Where wasm providers are built.
    build_dir: PathBuf,
    events: Events,
    running: BTreeMap<String, Running>,
    /// The native providers built into the app, by name.
    natives: Natives,
}

/// A provider's thread, and how to reach and stop it.
struct Running {
    messages: mpsc::Sender<Message>,
    stop: Arc<AtomicBool>,
    /// Its folder when it started: a reload restarts it only if this changed.
    fingerprint: Fingerprint,
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
    pub(crate) build_dir: PathBuf,
    pub(crate) messages: mpsc::Receiver<Message>,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) events: Events,
}

impl Context {
    pub(crate) fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub(crate) fn publish(&self, data: Data) {
        (self.events)(Event::Published {
            provider: self.name.clone(),
            data,
        });
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
        let build_dir = build_dir.into();
        let mut providers = Self {
            roots,
            build_dir: std::path::absolute(&build_dir).unwrap_or(build_dir),
            events,
            running: BTreeMap::new(),
            natives,
        };
        providers.reload();
        providers
    }

    /// Starts providers that appeared or whose folders changed, and stops
    /// those that went. The others keep running, and keep their state.
    pub fn reload(&mut self) {
        let found = discover(&self.roots);
        self.running.retain(|name, _| found.contains_key(name));
        for (name, (dir, manifest)) in found {
            let fingerprint = Fingerprint::of(&dir);
            if self
                .running
                .get(&name)
                .is_some_and(|running| running.fingerprint == fingerprint)
            {
                continue;
            }
            // Stops the old one, if any.
            self.running.remove(&name);
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
            let (sender, messages) = mpsc::channel();
            let stop = Arc::new(AtomicBool::new(false));
            let context = Context {
                name: name.clone(),
                dir,
                manifest,
                build_dir: self.build_dir.clone(),
                messages,
                stop: stop.clone(),
                events: self.events.clone(),
            };
            let native = self.natives.get(name.as_str()).copied();
            let spawned = thread::Builder::new()
                .name(format!("provider-{name}"))
                .spawn(move || match context.manifest.kind {
                    Kind::Wasm => wasm::run(&context),
                    Kind::Process { .. } => process::run(&context),
                    Kind::Native => native::run(&context, native),
                });
            match spawned {
                Ok(_) => {
                    self.running.insert(name, Running {
                        messages: sender,
                        stop,
                        fingerprint,
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

    /// Hands `message`, from the mod `from`, to the provider `name`.
    pub fn send(&self, name: &str, from: &str, message: Data) -> Result<(), Error> {
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
mod tests;
