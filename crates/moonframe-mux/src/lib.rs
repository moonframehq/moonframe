//! What runs the app's terminals, chosen by what this machine has:
//!
//! 1. **herdr-core** (`hc`), when installed: it keeps terminals running
//!    and streams their output; started if it isn't running.
//! 2. **tmux**, when installed: a session per terminal on the app's own
//!    server.
//! 3. **pty**: the login shell on a local pseudo-terminal, always there;
//!    it ends with the app.
//!
//! The first two outlive the app, and a terminal attaches to the same
//! program again on the next launch. Each is a
//! [`moonframe_terminal::Backend`], so the app never depends on one.

pub mod document;
#[cfg(feature = "herdr-core")]
mod herdr_core;
mod program;
mod tmux;

#[cfg(feature = "herdr-core")]
pub use herdr_core::{CoreStore, HerdrCore};
pub use {
    document::{Document, FileStore, Store},
    tmux::Tmux,
};

use {
    moonframe_terminal::{Backend, Open, Opened, Pty},
    std::{
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
};

/// How long a backend found missing stays missing before it is looked for
/// again, so one installed while the app runs is picked up.
const LOOK_AGAIN: Duration = Duration::from_secs(10);

/// Which backend a terminal asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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
struct Found<T> {
    value: Mutex<(Option<Arc<T>>, Option<Instant>)>,
}

impl<T> Default for Found<T> {
    fn default() -> Self {
        Self {
            value: Mutex::new((None, None)),
        }
    }
}

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

/// The backends this machine has, found on first use.
#[derive(Default)]
pub struct Backends {
    #[cfg(feature = "herdr-core")]
    herdr_core: Found<HerdrCore>,
    tmux: Found<Tmux>,
}

impl Backends {
    pub fn new() -> Self {
        Self::default()
    }

    /// herdr-core, when it is installed and starts. Blocks on first use.
    pub fn herdr_core(&self) -> Option<Arc<dyn Backend>> {
        #[cfg(feature = "herdr-core")]
        {
            self.herdr_core
                .get(HerdrCore::connect)
                .map(|core| core as Arc<dyn Backend>)
        }
        #[cfg(not(feature = "herdr-core"))]
        None
    }

    /// tmux, when it is installed. Blocks on first use.
    pub fn tmux(&self) -> Option<Arc<dyn Backend>> {
        self.tmux
            .get(Tmux::find)
            .map(|tmux| tmux as Arc<dyn Backend>)
    }

    /// The backends to try for `preference`, best first; the local shell is
    /// always last, so a terminal opens whatever this machine lacks.
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

    /// Where the app's document (`name`) lives: herdr-core, when it runs
    /// here, else the JSON file at `file`. Moving to herdr-core brings the
    /// file's document along the first time. Blocks on first use.
    pub fn store(&self, name: &str, file: PathBuf) -> Arc<dyn Store> {
        let file = FileStore::new(file);
        #[cfg(feature = "herdr-core")]
        if let Some(core) = self.herdr_core.get(HerdrCore::connect) {
            let store = CoreStore::new(core, name);
            match store.load() {
                Ok(None) => {
                    if let Ok(Some(document)) = file.load()
                        && let Err(problem) = store.save(&document)
                    {
                        tracing::warn!("cannot move the document to herdr-core: {problem}");
                    }
                    return Arc::new(store);
                },
                Ok(Some(_)) => return Arc::new(store),
                Err(problem) => tracing::warn!("herdr-core keeps no document: {problem}"),
            }
        }
        #[cfg(not(feature = "herdr-core"))]
        let _ = name;
        Arc::new(file)
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
