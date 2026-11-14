//! Live reload: a change in the app's folders loads the mods again, and one
//! in a provider's folder starts its providers again, as on GPUI, so editing
//! Luau shows at once and a provider an agent adds runs without a restart.

use {
    crate::Ui,
    notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher},
    rune::Waker,
    std::{
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    },
};

/// What a mod's look, behaviour and words come from.
const WATCHED: &[&str] = &["luau", "toml", "ftl"];

pub(crate) struct Watching {
    _watcher: RecommendedWatcher,
    /// A mod's file changed: load the mods again.
    modules: Arc<AtomicBool>,
    /// Something in a provider's folder changed: find the providers again
    /// and restart those whose folder changed.
    providers: Arc<AtomicBool>,
}

/// Watches every layer of `ui`'s app; `None`, with a warning, if it can't.
pub(crate) fn watching(ui: &Ui, wake: Waker) -> Option<Watching> {
    let modules = Arc::new(AtomicBool::new(false));
    let providers = Arc::new(AtomicBool::new(false));
    let (told_modules, told_providers) = (Arc::clone(&modules), Arc::clone(&providers));
    let layers: Vec<PathBuf> = ui
        .workspace()
        .roots()
        .layers()
        .map(Path::to_owned)
        .collect();
    // Events name absolute, resolved paths; compare against the same.
    let provider_dirs: Vec<PathBuf> = layers
        .iter()
        .map(|layer| {
            layer
                .canonicalize()
                .unwrap_or_else(|_| layer.clone())
                .join("providers")
        })
        .collect();
    let mut watcher = recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        if !matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
        ) {
            return;
        }
        let (mut module, mut provider) = (false, false);
        for path in &event.paths {
            match changed(path, &provider_dirs) {
                Some(Changed::Module) => module = true,
                Some(Changed::Provider) => provider = true,
                None => {},
            }
        }
        if module {
            told_modules.store(true, Ordering::Release);
        }
        if provider {
            told_providers.store(true, Ordering::Release);
        }
        if module || provider {
            wake();
        }
    })
    .inspect_err(|error| tracing::warn!("not watching for changes: {error}"))
    .ok()?;
    for layer in &layers {
        if let Err(error) = watcher.watch(layer, RecursiveMode::Recursive) {
            tracing::warn!("not watching {}: {error}", layer.display());
        }
    }
    Some(Watching {
        _watcher: watcher,
        modules,
        providers,
    })
}

/// What a change at `path` touched.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Changed {
    Module,
    Provider,
}

/// What a change at `path` touches, as GPUI's watcher sorts it: anything
/// in a layer's `providers/` but its builds (`target/`, `Cargo.lock`, which
/// would loop back into a rebuild), else a mod's file. Git's own files, as
/// a package is fetched, are neither.
pub(crate) fn changed(path: &Path, provider_dirs: &[PathBuf]) -> Option<Changed> {
    if path.components().any(|part| part.as_os_str() == ".git") {
        return None;
    }
    if provider_dirs.iter().any(|dir| path.starts_with(dir)) {
        let built = path.components().any(|part| part.as_os_str() == "target")
            || path.file_name().is_some_and(|name| name == "Cargo.lock");
        return (!built && !path.is_dir()).then_some(Changed::Provider);
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| WATCHED.contains(&extension))
        .then_some(Changed::Module)
}

impl Ui {
    /// Loads the mods again if one of their files changed, and restarts the
    /// providers whose folders changed; says whether anything did.
    pub(crate) fn reload_changed(&mut self) -> bool {
        let Some(watching) = &self.watching else {
            return false;
        };
        let modules = watching.modules.swap(false, Ordering::AcqRel);
        let providers = watching.providers.swap(false, Ordering::AcqRel);
        if modules {
            self.workspace_mut().reload();
        }
        if providers && let Some(running) = &mut self.providers {
            let before: Vec<String> = running.names().map(str::to_owned).collect();
            running.reload();
            let gone: Vec<String> = before
                .into_iter()
                .filter(|name| !running.names().any(|now| now == name.as_str()))
                .collect();
            let mut workspace = self.workspace_mut();
            for name in &gone {
                workspace.clear_host(name, None);
            }
        }
        modules || providers
    }
}
