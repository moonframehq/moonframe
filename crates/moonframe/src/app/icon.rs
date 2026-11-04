//! `app:icon`: a mod changes the app's icon in the Dock while it runs.

use super::*;

/// Largest image `app:icon` shows.
const MAX_ICON_BYTES: u64 = 4 * 1024 * 1024;

impl MoonframeApp {
    /// The file `src` names for the mod `from`, as `ui.image` finds one:
    /// relative to the mod's folder, `~/...`, or absolute, of at most
    /// `max_bytes` and inside the app's folders.
    pub(super) fn mod_file(&self, from: &str, src: &str, max_bytes: u64) -> Option<PathBuf> {
        let folder = self
            .workspace
            .slot(from)
            .and_then(|slot| slot.source())
            .and_then(Path::parent)
            .unwrap_or(Path::new("."));
        let allowed: Vec<PathBuf> = self
            .workspace
            .roots()
            .layers()
            .map(Path::to_owned)
            .collect();
        let home = std::env::home_dir();
        moonframe_core::resolve_file(src, folder, home.as_deref(), &allowed, max_bytes)
    }

    /// Shows the icon the mod `from` asked for, or the app's own again.
    pub(super) fn show_icon(&mut self, from: &str, icon: &AppIcon) {
        let file = match icon {
            AppIcon::Default => self.folders.icon(),
            AppIcon::File(src) => match self.mod_file(from, src, MAX_ICON_BYTES) {
                Some(file) => Some(file),
                None => {
                    self.workspace.refuse(
                        from,
                        Error::new(format!(
                            "`app:icon`: no image `{src}` of at most 4 MiB in the app's folders"
                        )),
                    );
                    return;
                },
            },
        };
        if let Err(problem) = crate::dock::set_icon(file.as_deref()) {
            self.workspace
                .refuse(from, Error::new(format!("`app:icon`: {problem}")));
        }
    }
}
