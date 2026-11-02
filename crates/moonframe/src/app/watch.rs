//! Watching the app's folders, and reloading what changed.

use super::*;

/// What a burst of file events touched.
#[derive(Clone, Copy, Default)]
struct Changes {
    modules: bool,
    providers: bool,
    /// `packages.toml`: a package was installed, removed, or turned on or off.
    packages: bool,
}

/// Watches every layer, and the list of packages in the window's own
/// folder. Other profiles, inside the user's folder, are none of its
/// business.
pub(super) fn watch(
    workspace: &Workspace,
    folders: &Folders,
    cx: &mut Context<MoonframeApp>,
) -> notify::Result<RecommendedWatcher> {
    let (sender, mut changes) = futures::channel::mpsc::unbounded();
    // Events name absolute, resolved paths; compare against the same.
    let layers: Vec<PathBuf> = workspace
        .roots()
        .layers()
        .filter(|layer| layer.is_dir())
        .map(Path::to_owned)
        .collect();
    let resolved = |folder: &Path| folder.canonicalize().unwrap_or_else(|_| folder.to_owned());
    let own = folders.own().map(|own| resolved(&own));
    let list = own.as_ref().map(|own| own.join(packages::FILE));
    let others = folders
        .user
        .as_deref()
        .map(|user| resolved(&paths::profiles_folder(user)));
    let providers: Vec<PathBuf> = layers
        .iter()
        .map(|layer| {
            layer
                .canonicalize()
                .unwrap_or_else(|_| layer.clone())
                .join("providers")
        })
        .collect();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        if event.kind.is_access() {
            return;
        }
        let mut found = Changes::default();
        for path in &event.paths {
            if others
                .as_ref()
                .is_some_and(|others| path.starts_with(others))
                && !own.as_ref().is_some_and(|own| path.starts_with(own))
            {
                continue;
            }
            // Git writes its own files as a package is fetched; the
            // checkout it makes is what counts.
            if path.components().any(|part| part.as_os_str() == ".git") {
                continue;
            }
            if list.as_deref() == Some(path.as_path()) {
                found.packages = true;
            } else if providers
                .iter()
                .any(|providers| path.starts_with(providers))
            {
                // Builds write their lock and output; those must not loop
                // back into a rebuild.
                let built = path.components().any(|part| part.as_os_str() == "target")
                    || path.file_name().is_some_and(|name| name == "Cargo.lock");
                found.providers |= !built && !path.is_dir();
            } else {
                found.modules |= path
                    .extension()
                    .is_some_and(|extension| extension == "luau")
                    || path.is_dir();
            }
        }
        if found.modules || found.providers || found.packages {
            let _ = sender.unbounded_send(found);
        }
    })?;
    // Packages are cloned inside the user's folder, already watched.
    for (index, layer) in layers.iter().enumerate() {
        let nested = layers
            .iter()
            .enumerate()
            .any(|(other, outer)| other != index && layer.starts_with(outer) && layer != outer);
        if !nested {
            watcher.watch(layer, RecursiveMode::Recursive)?;
        }
    }
    cx.spawn(async move |this, cx| {
        while let Some(first) = changes.next().await {
            cx.background_executor().timer(SETTLE).await;
            let mut burst = first;
            while let Ok(more) = changes.try_recv() {
                burst.modules |= more.modules;
                burst.providers |= more.providers;
                burst.packages |= more.packages;
            }
            let reloaded = this.update(cx, |this, cx| {
                // Kept before it shows, so it can be undone.
                this.snapshot("changed", cx);
                // Lays the app over again, reloading everything in it.
                if burst.packages && this.reload_packages(cx) {
                    return;
                }
                if burst.modules {
                    this.reload(cx);
                }
                if burst.providers {
                    this.reload_providers(None, cx);
                }
            });
            if reloaded.is_err() {
                break;
            }
        }
    })
    .detach();
    Ok(watcher)
}
