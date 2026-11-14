//! `app:icon`: a mod changes the app's icon in the Dock while it runs.

use {super::*, std::io::Read as _};

/// Largest image `app:icon` shows, in bytes and pixels a side: what a mod
/// hands AppKit to decode on the main thread.
const MAX_ICON_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ICON_SIDE: usize = 4096;

/// The image in `file`, read once and at most [`MAX_ICON_BYTES`] of it,
/// whatever the file has become since it was found.
fn read_icon(file: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .and_then(|opened| opened.take(MAX_ICON_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
    if bytes.len() as u64 > MAX_ICON_BYTES {
        return Err(format!("{} is over 4 MiB", file.display()));
    }
    check_icon(&bytes).map_err(|problem| format!("{}: {problem}", file.display()))?;
    Ok(bytes)
}

/// Whether `bytes` start as an image of a size to show, from its header
/// alone, before anything decodes it.
fn check_icon(bytes: &[u8]) -> Result<(), String> {
    let size = imagesize::blob_size(bytes).map_err(|_| "not an image".to_owned())?;
    if size.width > MAX_ICON_SIDE || size.height > MAX_ICON_SIDE {
        return Err(format!(
            "{}x{} is over {MAX_ICON_SIDE} pixels a side",
            size.width, size.height
        ));
    }
    Ok(())
}

impl MoonframeApp {
    /// The file `src` names for the mod `from`, as `ui.image` finds one:
    /// relative to the mod's folder, `~/...`, or absolute, of at most
    /// `max_bytes` and inside the app's folders.
    pub(super) fn mod_file(&self, from: &str, src: &str, max_bytes: u64) -> Option<PathBuf> {
        let allowed: Vec<PathBuf> = self
            .workspace
            .roots()
            .layers()
            .map(Path::to_owned)
            .collect();
        let home = std::env::home_dir();
        moonframe_core::resolve_file(
            src,
            self.mod_folder(from),
            home.as_deref(),
            &allowed,
            max_bytes,
        )
    }

    /// The folder of the mod `from`, which names files relative to it.
    pub(super) fn mod_folder(&self, from: &str) -> &Path {
        self.workspace
            .slot(from)
            .and_then(|slot| slot.source())
            .and_then(Path::parent)
            .unwrap_or(Path::new("."))
    }

    /// Shows the icon the mod `from` asked for, or the app's own again.
    /// Refused rather than dropped when too soon, so a mod whose icon
    /// shows a state knows it did not change.
    pub(super) fn show_icon(&mut self, from: &str, icon: &AppIcon) {
        if let Err(problem) = self.change_icon(from, icon) {
            self.workspace
                .refuse(from, Error::new(format!("`app:icon`: {problem}")));
        }
    }

    fn change_icon(&mut self, from: &str, icon: &AppIcon) -> Result<(), String> {
        if !self.alerts.may_change_icon() {
            return Err("requests closer than 100 ms apart are refused".to_owned());
        }
        match icon {
            AppIcon::Default => crate::dock::set_icon(self.folders.icon().as_deref()),
            AppIcon::File(src) => {
                let file = self.mod_file(from, src, MAX_ICON_BYTES).ok_or_else(|| {
                    format!("no image `{src}` of at most 4 MiB in the app's folders")
                })?;
                crate::dock::show_icon(&read_icon(&file)?)
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{Alerts, MAX_ICON_BYTES, check_icon, read_icon};

    /// The start of a PNG `width` by `height`, as far as its header goes.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes.extend([8, 6, 0, 0, 0]);
        bytes
    }

    #[test]
    fn an_icon_is_a_real_image_of_a_sane_size() {
        assert!(check_icon(&png(1024, 1024)).is_ok());
        assert!(check_icon(&png(4096, 4096)).is_ok());
        assert!(
            check_icon(&png(4097, 16))
                .unwrap_err()
                .contains("over 4096")
        );
        assert!(
            check_icon(&png(16, 100_000))
                .unwrap_err()
                .contains("over 4096")
        );
        assert_eq!(check_icon(b"plain text").unwrap_err(), "not an image");
    }

    #[test]
    fn an_icon_grown_past_the_limit_is_not_read_whole() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.png");
        let mut bytes = png(16, 16);
        bytes.resize(MAX_ICON_BYTES as usize + 1, 0);
        std::fs::write(&file, bytes).unwrap();
        assert!(read_icon(&file).unwrap_err().contains("over 4 MiB"));
    }

    #[test]
    fn a_second_icon_change_too_soon_is_refused() {
        let mut alerts = Alerts::default();
        assert!(alerts.may_change_icon());
        assert!(!alerts.may_change_icon());
    }
}
