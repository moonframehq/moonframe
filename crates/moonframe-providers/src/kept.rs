//! State a wasm provider hands to its next build, kept for this launch.
//!
//! By folder, not name: a provider of the same name from another layer is
//! other code, and never gets this one's state. It stays here until a build
//! has run on it, so a build that fails leaves it for the one that fixes it.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

#[derive(Clone, Default)]
pub(crate) struct Kept(Arc<Mutex<BTreeMap<PathBuf, String>>>);

#[cfg_attr(not(feature = "wasm"), allow(dead_code))]
impl Kept {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<PathBuf, String>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn keep(&self, dir: &Path, state: String) {
        self.lock().insert(dir.to_owned(), state);
    }

    pub(crate) fn get(&self, dir: &Path) -> Option<String> {
        self.lock().get(dir).cloned()
    }

    pub(crate) fn forget(&self, dir: &Path) {
        self.lock().remove(dir);
    }
}
