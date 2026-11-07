//! A way back from a run: the app that runs this provider knows which
//! folders a change lands in, and keeps them. An app without one has runs
//! with nothing to undo.

use std::sync::{Arc, OnceLock};

/// Keeps the app's folders across an agent's run, by the run's id.
pub trait Keeper: Send + Sync {
    /// Remembers the folders as they are, before the run writes anything.
    fn before(&self, run: &str);
    /// Notes what the run left; whether it changed anything.
    fn after(&self, run: &str) -> bool;
    /// Takes the folders back to how [`Keeper::before`] found them.
    fn undo(&self, run: &str) -> Result<(), String>;
}

static KEEPER: OnceLock<Arc<dyn Keeper>> = OnceLock::new();

/// Gives runs a way back; set once, as the window opens.
pub fn set_keeper(keeper: Arc<dyn Keeper>) {
    let _ = KEEPER.set(keeper);
}

pub(crate) fn keeper() -> Option<Arc<dyn Keeper>> {
    KEEPER.get().cloned()
}
