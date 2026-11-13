//! Live reload: a change in the app's folders loads the mods again, as on
//! GPUI, so editing Luau shows at once.

use {
    crate::Ui,
    notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher},
    rune::Waker,
    std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// What Luau's look and behaviour come from.
const WATCHED: &[&str] = &["luau", "toml"];

pub(crate) struct Watching {
    _watcher: RecommendedWatcher,
    changed: Arc<AtomicBool>,
}

/// Watches every layer of `ui`'s app; `None`, with a warning, if it can't.
pub(crate) fn watching(ui: &Ui, wake: Waker) -> Option<Watching> {
    let changed = Arc::new(AtomicBool::new(false));
    let told = Arc::clone(&changed);
    let mut watcher = recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        let relevant = matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
        ) && event.paths.iter().any(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| WATCHED.contains(&extension))
        });
        if relevant {
            told.store(true, Ordering::Release);
            wake();
        }
    })
    .inspect_err(|error| tracing::warn!("not watching for changes: {error}"))
    .ok()?;
    let workspace = ui.workspace();
    for layer in workspace.roots().layers() {
        if let Err(error) = watcher.watch(layer, RecursiveMode::Recursive) {
            tracing::warn!("not watching {}: {error}", layer.display());
        }
    }
    Some(Watching {
        _watcher: watcher,
        changed,
    })
}

impl Ui {
    /// Loads the mods again if a file changed, and says whether it did.
    pub(crate) fn reload_changed(&mut self) -> bool {
        let changed = self
            .watching
            .as_ref()
            .is_some_and(|watching| watching.changed.swap(false, Ordering::AcqRel));
        if changed {
            self.workspace_mut().reload();
        }
        changed
    }
}
