//! Where an app's files come from: folders laid over each other, lowest
//! first. The app ships one (its built-in mods, providers, and stylesheet),
//! the packages the user installed go over it, and the user's
//! `~/.config/moonframe` goes on top, so an update to the app or a
//! package never touches what the user wrote, and the user can replace any
//! part of either by giving it the same name.
//!
//! - `main.luau`: the highest app layer's; packages never replace the shell.
//! - `style.luau`: every layer's, merged (see `Workspace`).
//! - `mods/<name>/`, `providers/<name>/`, `components/<name>.luau`: by name,
//!   the highest layer's replaces the others whole.
//!
//! A package is either laid out like an app folder, or is a single mod: a
//! folder with `main.luau` at its root, named after the folder.
//!
//! A profile ([`Roots::profile`]) is another app run from the same binary,
//! often one installed whole: an app package brings its own shell and mods,
//! and the built-in app under it is only a toolbox of providers,
//! components, and styles, with none of its mods unless the app keeps them.

use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roots {
    layers: Vec<Layer>,
    entries: BTreeMap<String, crate::entries::Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Layer {
    path: PathBuf,
    role: Role,
}

/// What a layer may bring.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Role {
    /// Everything, the shell included: the built-in app, the user's
    /// folder, or a profile's app package.
    App,
    /// Everything but the shell: a package someone published.
    Package,
    /// The built-in app under another app: its providers, components, and
    /// stylesheet, and of its mods only these.
    Toolbox(Vec<String>),
    /// Only its stylesheet: the user's own folder under a profile, so what
    /// they style everywhere follows them into every app.
    Style,
}

/// An app package for [`Roots::profile`]: its folder, and the built-in mods
/// it keeps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppLayer {
    pub path: PathBuf,
    pub keep: Vec<String>,
}

impl Layer {
    /// Made absolute: providers run in folders of their own, where a
    /// relative path would mean something else.
    fn new(path: PathBuf, role: Role) -> Self {
        Self {
            path: std::path::absolute(&path).unwrap_or(path),
            role,
        }
    }

    /// A package that is one mod rather than an app folder.
    fn single_mod(&self) -> bool {
        self.role == Role::Package && self.path.join("main.luau").is_file()
    }

    /// Whether it brings anything under `dir`: `mods`, `providers`,
    /// `components`, and so on.
    fn brings(&self, dir: &str) -> bool {
        self.role != Role::Style || dir.is_empty()
    }

    /// Whether it brings the mod `name`.
    fn brings_mod(&self, name: &str) -> bool {
        match &self.role {
            Role::App | Role::Package => true,
            // Every app gets the Developer tools: they show only in
            // Developer Mode, and are how its Luau is changed as it runs.
            // Keeping the sparkle keeps the window it opens.
            Role::Toolbox(keep) => {
                name == crate::DEVTOOLS
                    || keep.iter().any(|kept| {
                        kept == name || (kept == crate::FEEDBACK && name == crate::FEEDBACK_WINDOW)
                    })
            },
            Role::Style => false,
        }
    }
}

impl Roots {
    /// A sealed profile: only the toolbox, required packages and app it ships.
    /// Settings and saved state can still live outside these source layers.
    pub fn bundled(toolbox: PathBuf, packages: Vec<PathBuf>, app: AppLayer) -> Self {
        let layers = [Layer::new(toolbox, Role::Toolbox(app.keep))]
            .into_iter()
            .chain(
                packages
                    .into_iter()
                    .map(|path| Layer::new(path, Role::Package)),
            )
            .chain([Layer::new(app.path, Role::App)])
            .collect();
        Self {
            layers,
            entries: BTreeMap::new(),
        }
    }

    /// App folders, `layers` lowest first.
    pub fn new(layers: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            entries: BTreeMap::new(),
            layers: layers
                .into_iter()
                .map(|path| Layer::new(path, Role::App))
                .collect(),
        }
    }

    /// `builtin`, each package over it in order, and `user` on top.
    pub fn with_packages(
        builtin: PathBuf,
        packages: impl IntoIterator<Item = PathBuf>,
        user: Option<PathBuf>,
    ) -> Self {
        let layers = [Layer::new(builtin, Role::App)]
            .into_iter()
            .chain(
                packages
                    .into_iter()
                    .map(|path| Layer::new(path, Role::Package)),
            )
            .chain(user.map(|path| Layer::new(path, Role::App)))
            .collect();
        Self {
            layers,
            entries: BTreeMap::new(),
        }
    }

    /// A profile: `builtin`, the profile's `packages`, its `app` package
    /// over them, the stylesheet of the user's `global` folder, and the
    /// profile's own folder on top. With an app package, the built-in app
    /// brings only the mods the app keeps, and never its shell; without
    /// one, the profile starts as the built-in app does.
    pub fn profile(
        builtin: PathBuf,
        packages: impl IntoIterator<Item = PathBuf>,
        app: Option<AppLayer>,
        global: Option<PathBuf>,
        profile: PathBuf,
    ) -> Self {
        let base = match &app {
            Some(app) => Role::Toolbox(app.keep.clone()),
            None => Role::App,
        };
        let layers = [Layer::new(builtin, base)]
            .into_iter()
            .chain(
                packages
                    .into_iter()
                    .map(|path| Layer::new(path, Role::Package)),
            )
            .chain(app.map(|app| Layer::new(app.path, Role::App)))
            .chain(global.map(|path| Layer::new(path, Role::Style)))
            .chain([Layer::new(profile, Role::App)])
            .collect();
        Self {
            layers,
            entries: BTreeMap::new(),
        }
    }

    /// An app built on the library: `toolbox` (the built-in app, which
    /// brings only the mods `app` keeps, and never its shell), the host's
    /// own `app` over it, the `packages` the user installed over that, and
    /// the user's folder on top: the host takes the place the built-in app
    /// has in [`Roots::with_packages`].
    pub fn hosted(
        toolbox: PathBuf,
        app: AppLayer,
        packages: impl IntoIterator<Item = PathBuf>,
        user: Option<PathBuf>,
    ) -> Self {
        let layers = [
            Layer::new(toolbox, Role::Toolbox(app.keep)),
            Layer::new(app.path, Role::App),
        ]
        .into_iter()
        .chain(
            packages
                .into_iter()
                .map(|path| Layer::new(path, Role::Package)),
        )
        .chain(user.map(|path| Layer::new(path, Role::App)))
        .collect();
        Self {
            layers,
            entries: BTreeMap::new(),
        }
    }

    /// These roots with the layer at `from` read from `to` instead: to load
    /// a copy of a folder as it was, without touching the folder.
    pub fn moved(&self, from: &Path, to: &Path) -> Self {
        let resolved = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_owned());
        let from = resolved(from);
        let mut moved = self.clone();
        for layer in &mut moved.layers {
            if resolved(&layer.path) == from {
                to.clone_into(&mut layer.path);
            }
        }
        moved
    }

    /// The layers that bring an app of their own, the ones a change to the
    /// app lands in: never a package, which has its own updates.
    pub fn app_layers(&self) -> impl Iterator<Item = &Path> {
        self.layers
            .iter()
            .filter(|layer| matches!(layer.role, Role::App | Role::Toolbox(_)))
            .map(|layer| layer.path.as_path())
    }

    /// Whether `source` belongs to one of [`Roots::app_layers`]: the
    /// deepest layer holding it decides, so a package installed inside the
    /// user's folder (`packages/<name>`) is still a package.
    pub fn in_app_layer(&self, source: &Path) -> bool {
        self.layers
            .iter()
            .filter(|layer| source.starts_with(&layer.path))
            .max_by_key(|layer| layer.path.components().count())
            .is_some_and(|layer| matches!(layer.role, Role::App | Role::Toolbox(_)))
    }

    /// Every folder, lowest first.
    pub fn layers(&self) -> impl Iterator<Item = &Path> {
        self.layers.iter().map(|layer| layer.path.as_path())
    }

    /// The highest app layer's `relative` file.
    pub fn file(&self, relative: &str) -> Option<PathBuf> {
        self.candidates(relative).into_iter().next()
    }

    /// Every app layer's `relative` file, highest first: what to fall back
    /// on.
    pub fn candidates(&self, relative: &str) -> Vec<PathBuf> {
        self.layers
            .iter()
            .rev()
            .filter(|layer| layer.role == Role::App)
            .map(|layer| layer.path.join(relative))
            .filter(|path| path.is_file())
            .collect()
    }

    /// Every layer's `main.luau` for the mod `name`, highest first: under
    /// `mods/<name>/`, or the root of a single-mod package of that name.
    pub fn mod_candidates(&self, name: &str) -> Vec<PathBuf> {
        if let Some(entry) = self.entries.get(name) {
            return vec![entry.mod_file()];
        }
        // Only a mod name, never a path out of the `mods` folders.
        if !crate::style::valid_name(name) {
            return Vec::new();
        }
        let relative = format!("mods/{name}/main.luau");
        self.layers
            .iter()
            .rev()
            .filter(|layer| layer.brings_mod(name))
            .filter_map(|layer| {
                let path = if layer.single_mod() {
                    let folder = layer.path.file_name()?.to_str()?;
                    (mod_name(folder) == name).then(|| layer.path.join("main.luau"))?
                } else {
                    layer.path.join(&relative)
                };
                path.is_file().then_some(path)
            })
            .collect()
    }

    /// Every layer's `relative` file, lowest first.
    pub fn files(&self, relative: &str) -> Vec<PathBuf> {
        let dir = Path::new(relative)
            .parent()
            .and_then(Path::to_str)
            .unwrap_or_default();
        self.folder_layers(dir)
            .map(|layer| layer.join(relative))
            .filter(|path| path.is_file())
            .collect()
    }

    /// Each folder under `dir` holding `marker`, by name; a higher layer's
    /// folder replaces a lower one's of the same name. A single-mod package
    /// is a folder under `mods`.
    pub fn folders(&self, dir: &str, marker: &str) -> BTreeMap<String, PathBuf> {
        let mut found = BTreeMap::new();
        for layer in &self.layers {
            if !layer.brings(dir) {
                continue;
            }
            if layer.single_mod() {
                if dir == "mods"
                    && layer.path.join(marker).is_file()
                    && let Some(name) = layer.path.file_name().and_then(|name| name.to_str())
                {
                    found.insert(mod_name(name), layer.path.clone());
                }
                continue;
            }
            let Ok(entries) = fs::read_dir(layer.path.join(dir)) else {
                continue;
            };
            for path in entries.flatten().map(|entry| entry.path()) {
                // A folder being created has no marker yet.
                if !path.join(marker).is_file() {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|name| name.to_str())
                    && (dir != "mods" || layer.brings_mod(name))
                {
                    found.insert(name.to_owned(), path.clone());
                }
            }
        }
        for (name, entry) in &self.entries {
            let path = match dir {
                "mods" => entry.path.clone(),
                "providers" => entry.provider_dir(),
                _ => continue,
            };
            if path.join(marker).is_file() {
                found.insert(name.clone(), path);
            }
        }
        found
    }

    pub fn set_entries(&mut self, entries: BTreeMap<String, crate::entries::Entry>) {
        self.entries = entries;
    }

    pub fn external_entries(&self) -> &BTreeMap<String, crate::entries::Entry> {
        &self.entries
    }

    /// Each file `dir/<name>.<extension>`, by name; a higher layer's file
    /// replaces a lower one's.
    pub fn entries(&self, dir: &str, extension: &str) -> BTreeMap<String, PathBuf> {
        let mut found = BTreeMap::new();
        for layer in self.folder_layers(dir) {
            let Ok(entries) = fs::read_dir(layer.join(dir)) else {
                continue;
            };
            for path in entries.flatten().map(|entry| entry.path()) {
                if !path.is_file() || path.extension().is_none_or(|found| found != extension) {
                    continue;
                }
                if let Some(name) = path.file_stem().and_then(|name| name.to_str()) {
                    found.insert(name.to_owned(), path.clone());
                }
            }
        }
        found
    }

    /// Each layer's folder `dir`, where it has one, lowest first.
    pub(crate) fn dirs(&self, dir: &str) -> Vec<PathBuf> {
        self.folder_layers(dir)
            .map(|layer| layer.join(dir))
            .filter(|path| path.is_dir())
            .collect()
    }

    /// The layers laid out like an app folder, all but single-mod packages,
    /// that bring what is under `dir` (`""` for the folder's own files).
    fn folder_layers(&self, dir: &str) -> impl Iterator<Item = &Path> {
        self.layers
            .iter()
            .filter(move |layer| !layer.single_mod() && layer.brings(dir))
            .map(|layer| layer.path.as_path())
    }
}

/// The mod a single-mod package's folder `name` holds: lowercased, with
/// anything a mod name cannot hold, such as the dot in `my.mod`, as `-`.
pub fn mod_name(name: &str) -> String {
    name.chars()
        .map(|c| match c.to_ascii_lowercase() {
            c @ ('a'..='z' | '0'..='9' | '-' | '_') => c,
            _ => '-',
        })
        .collect()
}

impl From<&Path> for Roots {
    fn from(root: &Path) -> Self {
        Self::new([root.to_owned()])
    }
}

impl From<PathBuf> for Roots {
    fn from(root: PathBuf) -> Self {
        Self::new([root])
    }
}

impl fmt::Display for Roots {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, layer) in self.layers().enumerate() {
            if index > 0 {
                f.write_str(" + ")?;
            }
            write!(f, "{}", layer.display())?;
        }
        Ok(())
    }
}
