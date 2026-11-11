//! Where agents work and what they may write. The host says, when it can
//! ([`Editable`]): a provider's own folder may be the toolbox's, under an
//! app whose sources are in another repository. Without it, the provider's
//! folder is taken to be inside the app, as in the starter.

use {
    moonframe_providers::{Editable, Installed},
    std::path::{Path, PathBuf},
};

/// Where a provider's runs work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Place {
    /// The folder agents start in.
    pub(crate) cwd: PathBuf,
    /// The folder they change, which [`crate::Allow::App`] lets them edit.
    pub(crate) writable: Option<PathBuf>,
    /// `writable` from `cwd` (`app`, `examples/ledger`, `.`), when inside it.
    pub(crate) app: Option<String>,
}

/// The place for a provider in `dir`, with `[settings] cwd` as `setting`:
/// the setting, else the installed app's user folder, else the host's
/// account of the app, else the repository around the provider's app.
pub(crate) fn place(
    dir: &Path,
    setting: Option<&str>,
    home: Option<&Path>,
    installed: Option<&Installed>,
    editable: Option<&Editable>,
) -> Place {
    let cwd = match (setting, installed, editable) {
        (Some(_), ..) | (None, None, None) => workdir(dir, setting, home),
        // Whatever folder the provider was copied to, and even with a
        // repository around it: the window runs the bundle and this.
        (None, Some(installed), _) => installed.folder.clone(),
        (None, None, Some(editable)) => editable.workdir.clone(),
    };
    let writable = match (installed, editable) {
        (Some(installed), _) => Some(installed.folder.clone()),
        (None, Some(editable)) => Some(editable.writable.clone()),
        (None, None) => {
            let app = dir.join("..").join("..");
            let app = std::path::absolute(&app).unwrap_or(app);
            app.canonicalize().ok()
        },
    };
    let app = writable
        .as_deref()
        .and_then(|writable| app_folder(writable, &cwd));
    Place { cwd, writable, app }
}

/// Where agents work without the host's account: `[settings] cwd`, else
/// the repository holding the app folder this provider is in (its edits
/// reload the window), else the user's folder.
pub(crate) fn workdir(dir: &Path, setting: Option<&str>, home: Option<&Path>) -> PathBuf {
    if let Some(setting) = setting {
        return match (setting.strip_prefix("~/"), home) {
            (Some(rest), Some(home)) => home.join(rest),
            _ => PathBuf::from(setting),
        };
    }
    let start = dir.join("..").join("..");
    let start = std::path::absolute(&start).unwrap_or(start);
    let start = start.canonicalize().unwrap_or(start);
    if let Some(repo) = start
        .ancestors()
        .find(|folder| folder.join(".git").exists())
    {
        return repo.to_owned();
    }
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join(".config")))
        .unwrap_or_default();
    config.join("moonframe")
}

/// The app folder `app` from `cwd`; none when it is not inside `cwd`.
pub(crate) fn app_folder(app: &Path, cwd: &Path) -> Option<String> {
    let app = app.canonicalize().ok()?;
    let cwd = cwd.canonicalize().ok()?;
    let rest = app.strip_prefix(&cwd).ok()?;
    if rest.as_os_str().is_empty() {
        return Some(".".to_owned());
    }
    // Joined with `/` on every system: the mod shows it, and the agent is
    // told it, as a path from the repository.
    let parts: Vec<String> = rest
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_app_folder_is_named_from_the_workdir() {
        let repo = tempfile::tempdir().unwrap();
        let repo = repo.path().canonicalize().unwrap();
        let provider = repo.join("examples/ledger/providers/acp");
        std::fs::create_dir_all(&provider).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let place = place(&provider, None, None, None, None);
        assert_eq!(place.cwd, repo);
        assert_eq!(place.app.as_deref(), Some("examples/ledger"));
        assert_eq!(place.writable, Some(repo.join("examples/ledger")));
        assert_eq!(
            app_folder(&repo.join("examples/ledger"), &repo.join("elsewhere")),
            None
        );
    }

    /// An app on the library keeping `feedback`: the provider is the
    /// toolbox's, in Moonframe's checkout, and the app is in another
    /// repository. Its runs work there, not in Moonframe.
    #[test]
    fn a_hosted_app_works_in_its_own_repository() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let toolbox_provider = root.join("moonframe/app/providers/acp");
        let host_repo = root.join("notes");
        let host_app = host_repo.join("crates/notes/app");
        std::fs::create_dir_all(&toolbox_provider).unwrap();
        std::fs::create_dir_all(root.join("moonframe/.git")).unwrap();
        std::fs::create_dir_all(&host_app).unwrap();
        std::fs::create_dir_all(host_repo.join(".git")).unwrap();
        let editable = Editable {
            app: host_app.clone(),
            writable: host_app.clone(),
            workdir: host_repo.clone(),
            check: "notes --check".to_owned(),
            ..Editable::default()
        };

        // Without the host's account, it would work in Moonframe's checkout.
        let guessed = place(&toolbox_provider, None, None, None, None);
        assert_eq!(guessed.cwd, root.join("moonframe"));
        assert_eq!(guessed.app.as_deref(), Some("app"));

        let place = place(&toolbox_provider, None, None, None, Some(&editable));
        assert_eq!(place.cwd, host_repo);
        assert_eq!(place.writable, Some(host_app));
        assert_eq!(place.app.as_deref(), Some("crates/notes/app"));
    }

    #[test]
    fn a_setting_or_an_installed_app_wins() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let editable = Editable {
            app: root.join("app"),
            writable: root.join("app"),
            workdir: root.clone(),
            ..Editable::default()
        };
        let set = place(
            &root,
            Some("~/code/x"),
            Some(Path::new("/home/u")),
            None,
            Some(&editable),
        );
        assert_eq!(set.cwd, PathBuf::from("/home/u/code/x"));
        assert_eq!(set.writable, Some(root.join("app")));
        let installed = Installed {
            bundle: root.join("A.app"),
            files: root.join("A.app/Contents/Resources/app"),
            folder: root.join("user"),
            check: "a --check".to_owned(),
            skill: None,
        };
        let place = place(&root, None, None, Some(&installed), Some(&editable));
        assert_eq!(place.cwd, root.join("user"));
        assert_eq!(place.writable, Some(root.join("user")));
    }
}
