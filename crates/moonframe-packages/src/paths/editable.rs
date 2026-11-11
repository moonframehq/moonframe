//! The app as an agent changes it ([`Editable`]), from the folders a window
//! runs on: the starter from its checkout, an app package as a profile, an
//! installed app, or an app on the library from its own repository.

use {
    super::Folders,
    moonframe_providers::{Editable, repository},
    std::{
        env,
        path::{Path, PathBuf},
    },
};

impl Folders {
    /// What an agent needs to change this window's app. The app's folder is
    /// written in when it is someone's source (in a git checkout, outside
    /// the bundle); otherwise the user's folder over it is, as an installed
    /// app's is. `check`, when given, replaces the command that checks it.
    pub fn editable(&self, guide: Option<String>, check: Option<String>) -> Editable {
        let installed = self.installed();
        let app = self
            .bundled
            .as_ref()
            .map(|bundled| bundled.app.path.clone())
            .or_else(|| self.app())
            .unwrap_or_else(|| self.builtin.clone());
        let app = app.canonicalize().unwrap_or(app);
        // A package installed in the user's folder is a clone, and what
        // ships in a bundle only an update changes: neither is source,
        // whatever repository is around it (the user's dotfiles, say).
        let resolved = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_owned());
        let cloned = self
            .user
            .as_deref()
            .is_some_and(|user| app.starts_with(resolved(user)));
        let shipped =
            super::resources().is_some_and(|resources| app.starts_with(resolved(&resources)));
        let source = repository(&app).filter(|_| installed.is_none() && !cloned && !shipped);
        let own = self.own();
        let (writable, workdir) = match (&source, &own) {
            (Some(repo), _) => (app.clone(), repo.clone()),
            (None, Some(own)) => (own.clone(), own.clone()),
            (None, None) => (app.clone(), app.clone()),
        };
        let check = check
            .or_else(|| installed.as_ref().map(|installed| installed.check.clone()))
            .unwrap_or_else(|| self.check_command());
        Editable {
            title: self.title(),
            layers: self.labelled_layers(&app),
            app,
            writable,
            workdir,
            check,
            guide,
            state: Some(super::state(self.profile.as_deref())),
        }
    }

    /// The command that checks this window's app headlessly: this very
    /// executable on the same folders, so it needs no build and no recipe.
    fn check_command(&self) -> String {
        let exe = env::current_exe()
            .and_then(|exe| exe.canonicalize())
            .unwrap_or_else(|_| PathBuf::from(super::name()));
        let mut words = vec![quoted(&exe)];
        match &self.user {
            Some(user) => words.extend(["--user".to_owned(), quoted(user)]),
            None => words.push("--no-user".to_owned()),
        }
        if let Some(profile) = &self.profile {
            words.extend(["--profile".to_owned(), profile.clone()]);
        }
        words.push("--check".to_owned());
        // DIR is the built-in app, or a host's own folder; a profile's app
        // comes from the profile, and a bundle runs only its own.
        if self.bundled.is_none() {
            let dir = self.host.as_ref().map_or(&self.builtin, |host| &host.path);
            words.push(quoted(dir));
        }
        words.join(" ")
    }

    /// Every folder the window runs on, lowest first, named for an agent.
    fn labelled_layers(&self, app: &Path) -> Vec<(String, PathBuf)> {
        let resolved = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_owned());
        let builtin = resolved(&self.builtin);
        let user = self.user.as_deref().map(resolved);
        let own = self.own().map(|own| resolved(&own));
        self.roots()
            .layers()
            .map(|layer| {
                let layer = resolved(layer);
                let role = if layer == app {
                    "app"
                } else if layer == builtin {
                    "toolbox"
                } else if Some(&layer) == own.as_ref() || Some(&layer) == user.as_ref() {
                    "user"
                } else {
                    "package"
                };
                (role.to_owned(), layer)
            })
            .collect()
    }
}

/// `path` as one shell word.
fn quoted(path: &Path) -> String {
    let text = path.display().to_string();
    if text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+~".contains(c))
    {
        text
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, moonframe_core::AppLayer};

    fn folders(builtin: &Path, host: Option<&Path>, user: &Path) -> Folders {
        Folders {
            builtin: builtin.to_owned(),
            host: host.map(|path| AppLayer {
                path: path.to_owned(),
                keep: vec!["feedback".to_owned()],
            }),
            user: Some(user.to_owned()),
            profile: None,
            bundled: None,
        }
    }

    #[test]
    fn an_app_on_the_library_is_changed_in_its_own_repository() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        // The toolbox in Moonframe's checkout, the host's app in another.
        let toolbox = root.join("moonframe/app");
        let host_repo = root.join("notes");
        let host_app = host_repo.join("crates/notes/app");
        let user = root.join("config/notes");
        for folder in [&toolbox, &host_app, &user] {
            std::fs::create_dir_all(folder).unwrap();
        }
        std::fs::create_dir_all(root.join("moonframe/.git")).unwrap();
        std::fs::create_dir_all(host_repo.join(".git")).unwrap();

        let editable = folders(&toolbox, Some(&host_app), &user).editable(None, None);
        assert_eq!(editable.app, host_app);
        assert_eq!(editable.writable, host_app);
        assert_eq!(editable.workdir, host_repo);
        assert!(!editable.overlay());
        assert_eq!(editable.toolbox(), Some(toolbox.as_path()));
        assert!(
            editable
                .layers
                .iter()
                .any(|(role, path)| role == "app" && *path == host_app)
        );
        // It checks the host's folder, never the toolbox's.
        assert!(
            editable
                .check
                .ends_with(&format!("--check {}", host_app.display())),
            "{}",
            editable.check
        );
        assert!(!editable.check.contains("just "), "{}", editable.check);
    }

    #[test]
    fn an_app_outside_a_checkout_is_changed_from_the_users_folder() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let toolbox = root.join("toolbox");
        let host_app = root.join("cache/app");
        let user = root.join("config/notes");
        for folder in [&toolbox, &host_app, &user] {
            std::fs::create_dir_all(folder).unwrap();
        }
        let editable = folders(&toolbox, Some(&host_app), &user).editable(
            Some("Keep it calm.".to_owned()),
            Some("notes --check".to_owned()),
        );
        assert_eq!(editable.app, host_app);
        assert_eq!(editable.writable, user);
        assert_eq!(editable.workdir, user);
        assert!(editable.overlay());
        assert_eq!(editable.check, "notes --check");
        assert_eq!(editable.guide.as_deref(), Some("Keep it calm."));
    }

    #[test]
    fn the_starter_in_its_checkout_is_changed_there() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let app = root.join("app");
        let user = root.join("config");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let editable = folders(&app, None, &user).editable(None, None);
        assert_eq!(editable.writable, app);
        assert_eq!(editable.workdir, root);
        assert!(editable.check.contains("--check"), "{}", editable.check);
        assert!(
            editable
                .check
                .contains(&format!("--user {}", user.display())),
            "{}",
            editable.check
        );
    }

    #[test]
    fn a_package_cloned_into_the_users_folder_is_not_source() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        // The user's dotfiles hold their config, an installed app with it.
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let toolbox = root.join("toolbox");
        let user = root.join("config/notes");
        let clone = user.join("packages/notes");
        for folder in [&toolbox, &clone] {
            std::fs::create_dir_all(folder).unwrap();
        }
        let editable = folders(&toolbox, Some(&clone), &user).editable(None, None);
        assert_eq!(editable.app, clone);
        assert_eq!(editable.writable, user);
        assert!(editable.overlay());
    }

    #[test]
    fn odd_paths_are_quoted_for_the_shell() {
        assert_eq!(quoted(Path::new("/a/b-c.d")), "/a/b-c.d");
        assert_eq!(quoted(Path::new("/a b/it's")), r"'/a b/it'\''s'");
    }
}
