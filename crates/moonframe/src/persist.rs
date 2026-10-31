//! Keeping what mods ask to keep (`persist`) across launches, in the app's
//! document: herdr-core's document store when herdr-core runs here, else a
//! file (`moonframe_mux::Backends::store`).
//!
//! The window checks every few seconds whether that state changed and
//! hands it to a thread that saves it, so a slow store never holds up the
//! window; quitting saves once more, waiting for it.

use {
    moonframe_core::{Data, Workspace},
    moonframe_mux::{Document, Store},
    moonframe_providers::{from_json, to_json},
    std::{
        collections::BTreeMap,
        sync::{Arc, mpsc},
        thread,
    },
};

/// What each mod kept, by name.
pub(crate) type Kept = BTreeMap<String, Data>;

/// The document's state for the workspace to start from, and whether it
/// may be written: one the store can't read, or from a newer version, is
/// left alone.
pub(crate) fn load(store: &dyn Store) -> (Kept, bool) {
    match store.load() {
        Ok(Some(document)) => {
            tracing::info!(store = store.name(), "restoring what mods kept");
            let kept = document
                .mods
                .iter()
                .filter_map(|(name, value)| match from_json(value) {
                    Ok(data) => Some((name.clone(), data)),
                    Err(problem) => {
                        tracing::warn!("{name}'s kept state is unusable: {problem}");
                        None
                    },
                })
                .collect();
            (kept, true)
        },
        Ok(None) => (Kept::new(), true),
        Err(problem) => {
            tracing::warn!(store = store.name(), "not keeping state: {problem}");
            (Kept::new(), false)
        },
    }
}

/// Saves the workspace's kept state when it changes.
pub(crate) struct Persistence {
    store: Arc<dyn Store>,
    last: Kept,
    saves: mpsc::Sender<Kept>,
}

impl Persistence {
    /// Starts the saving thread; `last` is what the store holds now.
    pub(crate) fn start(store: Arc<dyn Store>, last: Kept) -> Self {
        let (saves, pending) = mpsc::channel::<Kept>();
        let saver = store.clone();
        let started = thread::Builder::new()
            .name("persist".into())
            .spawn(move || {
                while let Ok(first) = pending.recv() {
                    // Only the newest of several waiting saves matters.
                    let kept = pending.try_iter().last().unwrap_or(first);
                    save(&*saver, &kept);
                }
            });
        if let Err(error) = started {
            tracing::warn!("cannot start saving state: {error}");
        }
        Self { store, last, saves }
    }

    /// Saves, on the saving thread, if the kept state changed.
    pub(crate) fn check(&mut self, workspace: &Workspace) {
        let kept = workspace.persisted();
        if kept != self.last {
            self.last = kept.clone();
            let _ = self.saves.send(kept);
        }
    }

    /// Saves now, if it changed, waiting for it: for quitting.
    pub(crate) fn flush(&mut self, workspace: &Workspace) {
        let kept = workspace.persisted();
        if kept != self.last {
            save(&*self.store, &kept);
            self.last = kept;
        }
    }
}

fn save(store: &dyn Store, kept: &Kept) {
    let document = Document {
        mods: kept
            .iter()
            .map(|(name, data)| (name.clone(), to_json(data)))
            .collect(),
    };
    if let Err(problem) = store.save(&document) {
        tracing::warn!(
            store = store.name(),
            "cannot save what mods kept: {problem}"
        );
    }
}
