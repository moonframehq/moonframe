//! What runs the app's terminals, chosen by what this machine has:
//!
//! 1. **herdr-core** (`hc`), when installed: it keeps terminals running
//!    and streams their output; started if it isn't running.
//! 2. **tmux**, when installed: a session per terminal on the app's own
//!    server (`tmux -L <app>`).
//! 3. **pty**: the login shell on a local pseudo-terminal, always there;
//!    it ends with the app.
//!
//! The first two outlive the app, and a terminal attaches to the same
//! program again on the next launch. Each is a
//! `moonframe_terminal::Backend`, so the app never depends on one.
//! Without `terminal`, only document storage and backend discovery remain.

pub mod document;
#[cfg(feature = "herdr-core")]
mod herdr_core;
#[cfg(any(feature = "terminal", feature = "herdr-core"))]
mod program;
#[cfg(feature = "terminal")]
mod tmux;

pub use document::Store;
#[cfg(feature = "herdr-core")]
pub use herdr_core::{CoreStore, HerdrCore};
#[cfg(feature = "terminal")]
pub use tmux::Tmux;

#[cfg(feature = "terminal")]
use moonframe_terminal::{Backend, Open, Opened, Pty};
use std::sync::Arc;
#[cfg(any(feature = "terminal", feature = "herdr-core"))]
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

/// How long a backend found missing stays missing before it is looked for
/// again, so one installed while the app runs is picked up.
#[cfg(any(feature = "terminal", feature = "herdr-core"))]
const LOOK_AGAIN: Duration = Duration::from_secs(10);

/// Which backend a terminal asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg(feature = "terminal")]
pub enum Preference {
    /// The best one this machine has.
    #[default]
    Auto,
    HerdrCore,
    Tmux,
    /// A local shell that ends with the app.
    Pty,
}

/// One backend, once found; looked for again a while after it was missing.
#[cfg(any(feature = "terminal", feature = "herdr-core"))]
struct Found<T> {
    value: Mutex<(Option<Arc<T>>, Option<Instant>)>,
}

#[cfg(any(feature = "terminal", feature = "herdr-core"))]
impl<T> Default for Found<T> {
    fn default() -> Self {
        Self {
            value: Mutex::new((None, None)),
        }
    }
}

#[cfg(any(feature = "terminal", feature = "herdr-core"))]
impl<T> Found<T> {
    fn get(&self, find: impl FnOnce() -> Option<T>) -> Option<Arc<T>> {
        let mut value = self.value.lock().ok()?;
        let (found, looked) = &mut *value;
        if found.is_none() && looked.is_none_or(|looked| looked.elapsed() >= LOOK_AGAIN) {
            *found = find().map(Arc::new);
            *looked = Some(Instant::now());
        }
        found.clone()
    }
}

/// The namespace of the starter app, whose terminals predate namespaces.
pub const STARTER: &str = "moonframe";

/// The backends this machine has, found on first use, for one app's
/// terminals: each app (and each profile) keeps its own, apart from any
/// other's, on a tmux server of its own and under its own label in
/// herdr-core.
pub struct Backends {
    namespace: String,
    #[cfg(feature = "herdr-core")]
    herdr_core: Found<HerdrCore>,
    #[cfg(feature = "terminal")]
    tmux: Found<Tmux>,
}

impl Default for Backends {
    fn default() -> Self {
        Self::new()
    }
}

impl Backends {
    /// The starter's.
    pub fn new() -> Self {
        Self::for_app(STARTER)
    }

    /// The terminals of the app `namespace` (`notes`, or `moonframe-ledger`
    /// for a profile): its tmux server is `tmux -L <namespace>`.
    pub fn for_app(namespace: &str) -> Self {
        Self {
            namespace: namespace.to_owned(),
            #[cfg(feature = "herdr-core")]
            herdr_core: Found::default(),
            #[cfg(feature = "terminal")]
            tmux: Found::default(),
        }
    }

    /// Whose terminals these are.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    #[cfg(feature = "herdr-core")]
    fn core(&self) -> Option<Arc<HerdrCore>> {
        self.herdr_core
            .get(|| HerdrCore::connect().map(|core| core.for_app(&self.namespace)))
    }

    /// herdr-core, when it is installed and starts. Blocks on first use.
    #[cfg(feature = "terminal")]
    pub fn herdr_core(&self) -> Option<Arc<dyn Backend>> {
        #[cfg(feature = "herdr-core")]
        {
            self.core().map(|core| core as Arc<dyn Backend>)
        }
        #[cfg(not(feature = "herdr-core"))]
        None
    }

    /// tmux, when it is installed. Blocks on first use.
    #[cfg(feature = "terminal")]
    pub fn tmux(&self) -> Option<Arc<dyn Backend>> {
        self.tmux
            .get(|| Tmux::find().map(|tmux| tmux.on_socket(&self.namespace)))
            .map(|tmux| tmux as Arc<dyn Backend>)
    }

    /// The backends to try for `preference`, best first; the local shell is
    /// always last, so a terminal opens whatever this machine lacks.
    #[cfg(feature = "terminal")]
    pub fn candidates(&self, preference: Preference) -> Vec<Arc<dyn Backend>> {
        let mut candidates = match preference {
            Preference::Pty => Vec::new(),
            Preference::Tmux => [self.tmux(), self.herdr_core()]
                .into_iter()
                .flatten()
                .collect(),
            Preference::HerdrCore | Preference::Auto => [self.herdr_core(), self.tmux()]
                .into_iter()
                .flatten()
                .collect(),
        };
        candidates.push(Arc::new(Pty));
        candidates
    }

    /// Opens `open` on the best backend that manages it, and says which.
    #[cfg(feature = "terminal")]
    pub fn open(
        &self,
        preference: Preference,
        open: &Open,
    ) -> Result<(Opened, &'static str), String> {
        let mut problems = Vec::new();
        for backend in self.candidates(preference) {
            match backend.open(open) {
                Ok(opened) => {
                    if !problems.is_empty() {
                        tracing::warn!("terminal {}: {}", open.key, problems.join("; "));
                    }
                    return Ok((opened, backend.name()));
                },
                Err(problem) => problems.push(format!("{}: {problem}", backend.name())),
            }
        }
        Err(problems.join("; "))
    }

    /// herdr-core's document `name`, when herdr-core runs here (started
    /// if it is installed). Blocks on first use.
    pub fn store(&self, name: &str) -> Option<Arc<dyn Store>> {
        #[cfg(feature = "herdr-core")]
        if let Some(core) = self.core() {
            let store = CoreStore::new(core, name);
            match store.load() {
                Ok(_) => return Some(Arc::new(store)),
                Err(problem) => tracing::warn!("herdr-core keeps no document: {problem}"),
            }
        }
        #[cfg(not(feature = "herdr-core"))]
        let _ = name;
        None
    }
}

/// Whether herdr-core is installed here, running or not.
pub fn herdr_core_installed() -> bool {
    #[cfg(feature = "herdr-core")]
    {
        herdr_core::installed().is_some()
    }
    #[cfg(not(feature = "herdr-core"))]
    false
}

#[cfg(all(test, feature = "terminal"))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
