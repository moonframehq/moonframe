//! Following `settings.toml` as it changes, in a bundle too: the app's
//! folders may not be watched there, but this is the user's own file. Only
//! reads, so what the app saves itself comes back as no change.

use {
    super::*,
    notify::{RecursiveMode, Watcher},
};

/// Watches the folders holding the settings: an editor's save may replace
/// the file, which a watch on the file itself would lose.
pub(super) fn watching(
    folders: &Folders,
    cx: &mut Context<MoonframeApp>,
) -> Option<RecommendedWatcher> {
    // The watcher reports from a thread of its own, which the deterministic
    // scheduler of `e2e` refuses.
    if cfg!(test) {
        return None;
    }
    watch(folders, cx)
        .inspect_err(|error| tracing::warn!("not following settings.toml: {error}"))
        .ok()
}

fn watch(folders: &Folders, cx: &mut Context<MoonframeApp>) -> notify::Result<RecommendedWatcher> {
    let (sender, mut changes) = futures::channel::mpsc::unbounded();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let touched = event.is_ok_and(|event| {
            !event.kind.is_access()
                && event
                    .paths
                    .iter()
                    .any(|path| path.file_name().is_some_and(|name| name == "settings.toml"))
        });
        if touched {
            let _ = sender.unbounded_send(());
        }
    })?;
    for path in folders.settings() {
        if let Some(folder) = path.parent().filter(|folder| folder.is_dir()) {
            watcher.watch(folder, RecursiveMode::NonRecursive)?;
        }
    }
    cx.spawn(async move |this, cx| {
        while changes.next().await.is_some() {
            cx.background_executor().timer(SETTLE).await;
            while changes.try_recv().is_ok() {}
            if this
                .update(cx, |this, cx| this.reload_settings(cx))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
    Ok(watcher)
}

impl MoonframeApp {
    /// Takes up what `settings.toml` says now. A key removed from it keeps
    /// its value until the next launch; a value that is no good is reported
    /// and changes nothing.
    fn reload_settings(&mut self, cx: &mut Context<Self>) {
        self.load_measure();
        self.load_inspect_shortcut(cx);
        let mut changed = self.reload_theme(settings::LIGHT_THEME, false);
        changed |= self.reload_theme(settings::DARK_THEME, true);
        if let Some(name) = self.saved_setting(settings::APPEARANCE) {
            match Appearance::named(&name) {
                Some(appearance) if appearance != self.appearance => {
                    self.appearance = appearance;
                    show_appearance(appearance);
                    crate::host_menus::install(self.workspace.menus(), cx);
                    changed = true;
                },
                Some(_) => {},
                None => tracing::warn!("no appearance called {name:?}"),
            }
        }
        if changed {
            self.preview = None;
            self.apply_theme(cx);
        }
        cx.notify();
    }

    fn saved_setting(&self, key: &str) -> Option<String> {
        self.folders
            .settings()
            .iter()
            .find_map(|path| settings::get(path, key))
    }

    /// Takes up the theme saved at `key`, for dark or light windows: whether
    /// it is another one. Omarchy's theme follows the desktop on its own.
    fn reload_theme(&mut self, key: &str, dark: bool) -> bool {
        let Some(name) = self.saved_setting(key) else {
            return false;
        };
        let current = if dark {
            self.dark_scheme
        } else {
            self.light_scheme
        };
        if name == themes::omarchy::NAME || name == name_of(current) {
            return false;
        }
        match themes::fits(&name, dark) {
            Ok(scheme) => {
                let slot = usize::from(dark);
                self.pending_omarchy[slot] = false;
                if dark {
                    self.dark_scheme = scheme;
                } else {
                    self.light_scheme = scheme;
                }
                true
            },
            Err(problem) => {
                tracing::warn!("{key}: {problem}");
                false
            },
        }
    }
}
