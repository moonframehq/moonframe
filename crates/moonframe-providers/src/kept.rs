//! State a wasm provider hands to its next build, kept for this launch.
//!
//! By folder, not name: a provider of the same name from another layer is
//! other code, and never gets this one's state. It stays here until a build
//! has run on it, so a build that fails leaves it for the one that fixes it.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError, Weak},
};

#[derive(Clone, Default)]
pub(crate) struct Kept(Arc<Mutex<BTreeMap<PathBuf, Entry>>>);

#[derive(Default)]
struct Entry {
    state: Option<String>,
    builds: Vec<Weak<()>>,
}

/// Keeps this build pending until its context leaves, even if a replacement
/// fails before it can restore. Weak tokens also expire if spawning fails.
#[cfg_attr(not(feature = "wasm"), allow(dead_code))]
pub(crate) struct Build {
    _alive: Arc<()>,
    previous: Vec<Weak<()>>,
}

#[cfg_attr(not(feature = "wasm"), allow(dead_code))]
impl Build {
    pub(crate) fn waiting(&self) -> bool {
        self.previous.iter().any(|build| build.strong_count() > 0)
    }
}

#[cfg_attr(not(feature = "wasm"), allow(dead_code))]
impl Kept {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<PathBuf, Entry>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Register before spawning, so even rapid replacements see every build
    /// that could still save. A build waits only for its predecessors.
    pub(crate) fn begin(&self, dir: &Path) -> Build {
        let mut entries = self.lock();
        let entry = entries.entry(dir.to_owned()).or_default();
        entry.builds.retain(|build| build.strong_count() > 0);
        let alive = Arc::new(());
        let previous = entry.builds.clone();
        entry.builds.push(Arc::downgrade(&alive));
        Build {
            _alive: alive,
            previous,
        }
    }

    pub(crate) fn keep(&self, dir: &Path, state: String) {
        self.lock().entry(dir.to_owned()).or_default().state = Some(state);
    }

    pub(crate) fn get(&self, dir: &Path) -> Option<String> {
        self.lock().get(dir).and_then(|entry| entry.state.clone())
    }

    pub(crate) fn forget(&self, dir: &Path) {
        if let Some(entry) = self.lock().get_mut(dir) {
            entry.state = None;
        }
    }
}
