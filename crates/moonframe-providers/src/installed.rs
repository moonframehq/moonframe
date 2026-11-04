//! The app as installed: what ships in its signed bundle, which nothing may
//! change, and the user's folder laid over it, where changes go instead.
//! Set once at launch; a run from source has none.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// An installed app.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Installed {
    /// All that ships: the `.app`, or the folder of a Linux or Windows
    /// install. A change in it breaks the signature, and the next update
    /// replaces it.
    pub bundle: PathBuf,
    /// The running app's own files, to read and copy from.
    pub files: PathBuf,
    /// The user's folder over it. A file here wins over the one at the same
    /// path in `files`, and removing it brings that one back.
    pub folder: PathBuf,
    /// The command that checks the app, as the user's shell takes it.
    pub check: String,
    /// The moonframe skill, when it shipped.
    pub skill: Option<PathBuf>,
}

impl Installed {
    /// Whether `path` is in the bundle, links followed. A file being
    /// created goes by the nearest folder that exists.
    pub fn holds(&self, path: &Path) -> bool {
        let bundle = self.bundle.canonicalize();
        let bundle = bundle.as_deref().unwrap_or(&self.bundle);
        resolved(path).starts_with(bundle)
    }
}

fn resolved(path: &Path) -> PathBuf {
    path.ancestors()
        .find_map(|folder| {
            let rest = path.strip_prefix(folder).ok()?;
            Some(folder.canonicalize().ok()?.join(rest))
        })
        .unwrap_or_else(|| path.to_owned())
}

static INSTALLED: OnceLock<Installed> = OnceLock::new();

/// Says the app runs installed.
pub fn set_installed(installed: Installed) {
    let _ = INSTALLED.set(installed);
}

/// The app as installed, if it is.
pub fn installed() -> Option<&'static Installed> {
    INSTALLED.get()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_bundle_holds_its_files_new_ones_and_links_to_them() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("A.app");
        let app = bundle.join("Contents/Resources/app");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("main.luau"), "").unwrap();
        let installed = Installed {
            bundle: bundle.clone(),
            files: app.clone(),
            folder: dir.path().join("user"),
            check: String::new(),
            skill: None,
        };
        assert!(installed.holds(&app.join("main.luau")));
        assert!(installed.holds(&app.join("mods/new/main.luau")));
        assert!(!installed.holds(&dir.path().join("user/main.luau")));
        // A folder beginning the same is another folder.
        assert!(!installed.holds(&dir.path().join("A.app.old/main.luau")));
        #[cfg(unix)]
        {
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&app, &link).unwrap();
            assert!(installed.holds(&link.join("main.luau")));
        }
    }
}
