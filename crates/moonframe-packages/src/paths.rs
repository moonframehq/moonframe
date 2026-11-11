//! Where the app's folders are. The built-in app is read-only (inside a
//! macOS bundle it is signed, and each update replaces it); what the user
//! and their agents write goes in `~/.config/<name>`, laid over it.
//!
//! An app on the library (`moonframe::Moonframe`) has its own folder over
//! the toolbox, the starter's `app/`: from its bundle, from this checkout,
//! or compiled in (the moonframe crate's `embed` and `launch` find it).

mod editable;

use {
    crate::packages,
    moonframe_core::{AppLayer, Roots},
    moonframe_providers::Installed,
    std::{
        env,
        path::{Path, PathBuf},
    },
};

/// The app's name: its folders, and its document in herdr-core.
pub fn name() -> &'static str {
    &crate::identity().name
}

/// What keeps this run's things apart from every other app's and
/// profile's: its document in herdr-core, its tmux server, its terminals'
/// label. The app's name, with the profile's after it.
pub fn namespace(profile: Option<&str>) -> String {
    match profile {
        Some(profile) => format!("{}-{profile}", name()),
        None => name().to_owned(),
    }
}

/// The bundle's folder for the app's files: `Contents/Resources` inside a
/// macOS bundle, `share/<binary>` beside `bin/` in a Linux one (each bundled
/// app's binary has its own name), or the executable's own folder in the
/// Linux and Windows release archives. Only when it holds an `app` folder.
pub fn resources() -> Option<PathBuf> {
    let exe = env::current_exe().and_then(|exe| exe.canonicalize()).ok()?;
    let name = exe.file_name()?.to_str()?.to_owned();
    let dir = exe.parent()?;
    let root = dir.parent()?;
    [
        root.join("Resources"),
        root.join("share").join(name),
        dir.to_owned(),
    ]
    .into_iter()
    .find(|folder| folder.join("app").is_dir())
}

/// All that ships around `resources`: the `.app` it is in, else itself.
fn installation(resources: &Path) -> PathBuf {
    resources
        .ancestors()
        .nth(2)
        .filter(|folder| folder.extension().is_some_and(|found| found == "app"))
        .unwrap_or(resources)
        .to_owned()
}

/// The path from the user's folder that replaces `source`, a file of the
/// app under `resources`: the same path, whichever of the bundle's layers
/// it is in. None for a file that does not ship, or one with no such path.
fn overlay_of(source: &Path, resources: &Path) -> Option<PathBuf> {
    let source = source.canonicalize().ok()?;
    let resources = resources.canonicalize().ok()?;
    let shipped = source.strip_prefix(&resources).ok()?;
    if let Ok(path) = shipped
        .strip_prefix("app")
        .or_else(|_| shipped.strip_prefix("bundle/app"))
    {
        return Some(path.to_owned());
    }
    // A package's folder, then its path; a single-mod package has none.
    let path: PathBuf = shipped
        .strip_prefix("bundle/packages")
        .ok()?
        .components()
        .skip(1)
        .collect();
    (path.components().count() > 1).then_some(path)
}

/// Where in the user's folder a copy of the shipped file `source` goes, to
/// win over it.
pub fn overlay(source: &Path) -> Option<PathBuf> {
    overlay_of(source, &resources()?)
}

/// Whether the binary runs from a bundle.
pub fn in_bundle() -> bool {
    resources().is_some()
}

#[cfg(all(test, feature = "sealed"))]
#[allow(clippy::unwrap_used)]
mod sealed_tests {
    use super::*;

    #[test]
    fn bundle_layers_work_without_a_user_and_ignore_user_sources() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("toolbox");
        let app = dir.path().join("app");
        let package = dir.path().join("package");
        let user = dir.path().join("user");
        let own = user.join("profiles/test");
        for folder in [&base, &app, &package, &own] {
            std::fs::create_dir_all(folder).unwrap();
            std::fs::write(folder.join("main.luau"), "return mod {}").unwrap();
        }
        std::fs::write(own.join("style.luau"), "error('must not load')").unwrap();
        let mut folders = Folders {
            builtin: base.clone(),
            host: None,
            user: None,
            profile: Some("test".into()),
            bundled: Some(Bundled {
                name: "test".into(),
                app: AppLayer {
                    path: app.clone(),
                    keep: Vec::new(),
                },
                packages: vec![package.clone()],
            }),
        };
        let roots = folders.roots();
        assert_eq!(roots.file("main.luau"), Some(app.join("main.luau")));
        assert_eq!(roots.layers().collect::<Vec<_>>(), [
            base.as_path(),
            package.as_path(),
            app.as_path()
        ]);
        folders.user = Some(user);
        assert_eq!(folders.roots(), roots);
        assert!(folders.settings().contains(&own.join("settings.toml")));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod overlay_tests {
    use super::*;

    #[test]
    fn a_shipped_file_keeps_its_path_in_the_users_folder() {
        let dir = tempfile::tempdir().unwrap();
        let resources = dir.path().join("A.app/Contents/Resources");
        let file = |path: &str| {
            let file = resources.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, "").unwrap();
            file
        };
        let overlaid = |path: &str| overlay_of(&file(path), &resources);
        assert_eq!(
            overlaid("app/mods/status/main.luau"),
            Some("mods/status/main.luau".into())
        );
        assert_eq!(overlaid("bundle/app/main.luau"), Some("main.luau".into()));
        assert_eq!(
            overlaid("bundle/packages/kit/components/card.luau"),
            Some("components/card.luau".into())
        );
        // A single-mod package's main.luau is no shell.
        assert_eq!(overlaid("bundle/packages/kit/main.luau"), None);
        let outside = dir.path().join("mine.luau");
        std::fs::write(&outside, "").unwrap();
        assert_eq!(overlay_of(&outside, &resources), None);
        assert_eq!(
            installation(&resources.canonicalize().unwrap()).extension(),
            Some("app".as_ref())
        );
        assert_eq!(installation(dir.path()), dir.path());
    }
}

/// `$XDG_CACHE_HOME/<name>`, by default `~/.cache/<name>`.
pub fn cache() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache").unwrap_or_else(|| private().join("cache"))
}

/// Where the cache and state go when there is no home to put them in: a
/// folder of this run's own, made with a name no one can guess and only
/// this user's access. Never a fixed path in the shared temporary folder,
/// where another user could leave providers (code the app runs) or state
/// for the app to pick up.
fn private() -> PathBuf {
    static PRIVATE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    PRIVATE
        .get_or_init(|| {
            tempfile::Builder::new()
                .prefix(&format!("{}-", name()))
                .tempdir()
                .map(tempfile::TempDir::keep)
                .unwrap_or_else(|error| {
                    // Nowhere safe to write: the current folder, the user's own.
                    tracing::error!("cannot make a private folder: {error}");
                    PathBuf::from(".").join(format!(".{}", name()))
                })
        })
        .clone()
}

static USER_FOLDER: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// Records the user's folder this run uses (`--user`, or the default; none
/// with `--no-user`), for what starts other runs of the app.
pub fn set_user_folder(folder: Option<PathBuf>) {
    let _ = USER_FOLDER.set(folder.map(|folder| std::path::absolute(&folder).unwrap_or(folder)));
}

/// The user's folder this run uses, once [`set_user_folder`] said.
pub fn user_folder() -> Option<PathBuf> {
    USER_FOLDER.get().cloned().flatten()
}

/// `$XDG_CONFIG_HOME/<name>`, by default `~/.config/<name>`: the user's
/// mods, providers, and stylesheet.
pub fn user() -> Option<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config")
}

/// Where wasm providers are built: `$XDG_CACHE_HOME/<name>/providers`,
/// by default under `~/.cache`.
pub fn builds() -> PathBuf {
    cache().join("providers")
}

/// Where providers save pictures for `ui.image` (avatars, previews):
/// `$XDG_CACHE_HOME/<name>/images`, by default under `~/.cache`. Views
/// may show files from here and from the app's own folders, nowhere else.
pub fn images() -> PathBuf {
    cache().join("images")
}

/// Where the inspector saves the selected element for agents:
/// `$XDG_CACHE_HOME/<name>/selection.md`, by default under `~/.cache`.
pub fn selection() -> PathBuf {
    cache().join("selection.md")
}

/// Where state outlives the app (each mod's state, and topics):
/// `$XDG_STATE_HOME/<name>/state.sqlite`, by default under
/// `~/.local/state`; outside the user's folder, which the window watches.
/// Each profile keeps its own, `state-<profile>.sqlite`.
pub fn state(profile: Option<&str>) -> PathBuf {
    let file = match profile {
        Some(name) => format!("state-{name}.sqlite"),
        None => "state.sqlite".to_owned(),
    };
    xdg("XDG_STATE_HOME", ".local/state")
        .unwrap_or_else(|| private().join("state"))
        .join(file)
}

/// Where profiles live in the user's folder: `profiles/<name>`.
const PROFILES: &str = "profiles";

/// An app bundled whole (`just bundle-app NAME`): `Contents/Resources/bundle`
/// holds the app package in `app/` and what it requires in `packages/`, as
/// they ship, so it runs without anything installed.
#[derive(Clone, Debug)]
pub struct Bundled {
    /// The app's name, which names its profile in the user's folder.
    pub name: String,
    app: AppLayer,
    packages: Vec<PathBuf>,
}

/// The app bundled beside the binary, if any.
pub fn bundled() -> Option<Bundled> {
    let bundle = resources()?.join("bundle");
    let app = bundle.join("app");
    let manifest = packages::App::read(&app)
        .inspect_err(|problem| tracing::warn!("the bundled app: {problem}"))
        .ok()??;
    let mut packages: Vec<PathBuf> = std::fs::read_dir(bundle.join("packages"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    packages.sort();
    Some(Bundled {
        name: manifest.name,
        app: AppLayer {
            path: app,
            keep: manifest.keep,
        },
        packages,
    })
}

/// The folders one window runs on.
#[derive(Clone, Debug)]
pub struct Folders {
    /// The built-in app: the starter's, or the toolbox under a host's.
    pub builtin: PathBuf,
    /// An app on the library's own folder, over the toolbox.
    pub host: Option<AppLayer>,
    /// The user's folder, unless `--no-user`.
    pub user: Option<PathBuf>,
    /// The profile, if any: another app, kept in `profiles/<name>` in the
    /// user's folder.
    pub profile: Option<String>,
    /// The app bundled with the binary, which the profile runs.
    pub bundled: Option<Bundled>,
}

impl Folders {
    /// Where this window's own files go: its mods, packages, and
    /// settings. A profile's folder, else the user's.
    pub fn own(&self) -> Option<PathBuf> {
        let user = self.user.as_ref()?;
        Some(match &self.profile {
            Some(name) => profile(user, name),
            None => user.clone(),
        })
    }

    /// The app as installed, when it runs from its bundle's own files: an
    /// installed binary can be pointed at a folder of the user's instead.
    pub fn installed(&self) -> Option<Installed> {
        let resources = resources()?.canonicalize().ok()?;
        let builtin = self.builtin.canonicalize().ok()?;
        if !builtin.starts_with(&resources) {
            return None;
        }
        let binary = env::current_exe().and_then(|exe| exe.canonicalize()).ok()?;
        let mut check = format!("\"{}\"", binary.display());
        if let Some(user) = &self.user {
            check.push_str(&format!(" --user \"{}\"", user.display()));
        }
        if let Some(profile) = &self.profile {
            check.push_str(&format!(" --profile {profile}"));
        }
        check.push_str(" --check");
        let skill = builtin.join("providers/skills/skills/moonframe/SKILL.md");
        Some(Installed {
            bundle: installation(&resources),
            files: self
                .bundled
                .as_ref()
                .map_or_else(|| builtin.clone(), |bundled| bundled.app.path.clone()),
            folder: self.own().or_else(user)?,
            check,
            skill: skill.is_file().then_some(skill),
        })
    }

    /// The layers: the built-in app, under the packages the user enabled
    /// and the user's folder. In a profile, under the profile's packages,
    /// its app, the user's stylesheet, and the profile's folder.
    pub fn roots(&self) -> Roots {
        if cfg!(feature = "sealed")
            && let Some(bundled) = &self.bundled
        {
            return Roots::bundled(
                self.builtin.clone(),
                bundled.packages.clone(),
                bundled.app.clone(),
            );
        }
        match (&self.user, &self.profile) {
            (Some(user), Some(name)) => {
                let own = profile(user, name);
                let (mut packages, mut app) = packages::enabled_layers(&own, true);
                // A bundle brings its app and what it requires; the user's
                // own packages in its profile go over them.
                if let Some(bundled) = &self.bundled {
                    packages.splice(0..0, bundled.packages.iter().cloned());
                    app = Some(bundled.app.clone());
                }
                // A profile with no app of its own runs the host's.
                let app = app.or_else(|| self.host.clone());
                Roots::profile(self.builtin.clone(), packages, app, Some(user.clone()), own)
            },
            (user, _) => {
                let packages = user
                    .as_deref()
                    .map(|user| packages::enabled_layers(user, false).0)
                    .unwrap_or_default();
                match &self.host {
                    Some(host) => {
                        Roots::hosted(self.builtin.clone(), host.clone(), packages, user.clone())
                    },
                    None => Roots::with_packages(self.builtin.clone(), packages, user.clone()),
                }
            },
        }
    }

    /// The `settings.toml` files, highest first: the profile's, then the
    /// user's, so a profile starts in the theme the user picked.
    pub fn settings(&self) -> Vec<PathBuf> {
        self.own()
            .into_iter()
            .chain(self.profile.as_ref().and(self.user.clone()))
            .map(|folder| folder.join("settings.toml"))
            .collect()
    }

    /// The profile's app package, as the layers have it.
    fn app(&self) -> Option<PathBuf> {
        if let Some(bundled) = &self.bundled {
            return Some(bundled.app.path.clone());
        }
        let own = self.own().filter(|_| self.profile.is_some());
        own.and_then(|own| packages::enabled_layers(&own, true).1)
            .or_else(|| self.host.clone())
            .map(|app| app.path)
    }

    /// The profile's app package's `[app]`, if it has one.
    pub(crate) fn manifest(&self) -> Option<packages::App> {
        self.app()
            .and_then(|app| packages::App::read(&app).ok().flatten())
    }

    /// Where Check for Updates looks, if anywhere: in a profile, its app's
    /// `updates` and `version`; else the app's own (the starter's is
    /// Moonframe's repository). Only the bundle running the app being
    /// updated is replaced: the starter's `.app` with a profile in it is
    /// not the profile's app.
    pub fn updates(&self) -> Option<crate::update::Source> {
        let (repo, version) = match &self.profile {
            Some(_) => {
                let manifest = self.manifest()?;
                (manifest.updates?, manifest.version?)
            },
            None => (
                crate::identity().updates.clone()?,
                crate::identity().version.clone(),
            ),
        };
        let replaceable = self.profile.is_none() || self.bundled.is_some();
        Some(crate::update::Source {
            repo,
            version,
            bundle: crate::update::running_bundle().filter(|_| replaceable),
        })
    }

    /// The app's name for people: its manifest's title, else its name, else
    /// the profile's, else the one the app was launched with.
    pub fn title(&self) -> String {
        let manifest = self
            .app()
            .and_then(|app| packages::App::read(&app).ok().flatten());
        manifest
            .and_then(|app| app.title.or(Some(app.name)))
            .or_else(|| self.profile.clone())
            .unwrap_or_else(|| crate::identity().title.clone())
    }

    /// The app's own icon, for the Dock, if it has one: the one its
    /// manifest names, else its `icon.png`.
    pub fn icon(&self) -> Option<PathBuf> {
        let app = self.app()?;
        let named = packages::App::read(&app)
            .ok()
            .flatten()
            .and_then(|app| app.icon);
        Some(app.join(named.as_deref().unwrap_or("icon.png"))).filter(|icon| icon.is_file())
    }

    /// The app id for the window system: each profile is an app of its own.
    pub fn app_id(&self) -> String {
        let id = &crate::identity().id;
        match &self.profile {
            Some(name) => format!("{id}.{name}"),
            None => id.clone(),
        }
    }
}

/// The folder of every profile in the user's folder `user`.
pub fn profiles_folder(user: &Path) -> PathBuf {
    user.join(PROFILES)
}

/// The folder of the profile `name` in the user's folder `user`.
pub fn profile(user: &Path, name: &str) -> PathBuf {
    profiles_folder(user).join(name)
}

/// The profiles in the user's folder `user`, by name.
pub fn profiles(user: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(profiles_folder(user))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| packages::valid_name(name))
        .collect();
    names.sort();
    names
}

fn xdg(variable: &str, fallback: &str) -> Option<PathBuf> {
    env::var_os(variable)
        .map(PathBuf::from)
        // The spec ignores relative values.
        .filter(|dir| dir.is_absolute())
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback)))
        .map(|dir| dir.join(name()))
}
