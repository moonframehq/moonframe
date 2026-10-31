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

use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roots {
    layers: Vec<Layer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Layer {
    path: PathBuf,
    package: bool,
}

impl Layer {
    /// Made absolute: providers run in folders of their own, where a
    /// relative path would mean something else.
    fn new(path: PathBuf, package: bool) -> Self {
        Self {
            path: std::path::absolute(&path).unwrap_or(path),
            package,
        }
    }

    /// A package that is one mod rather than an app folder.
    fn single_mod(&self) -> bool {
        self.package && self.path.join("main.luau").is_file()
    }
}

impl Roots {
    /// App folders, `layers` lowest first.
    pub fn new(layers: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            layers: layers
                .into_iter()
                .map(|path| Layer::new(path, false))
                .collect(),
        }
    }

    /// `builtin`, each package over it in order, and `user` on top.
    pub fn with_packages(
        builtin: PathBuf,
        packages: impl IntoIterator<Item = PathBuf>,
        user: Option<PathBuf>,
    ) -> Self {
        let layers = [Layer::new(builtin, false)]
            .into_iter()
            .chain(packages.into_iter().map(|path| Layer::new(path, true)))
            .chain(user.map(|path| Layer::new(path, false)))
            .collect();
        Self { layers }
    }

    /// Every folder, lowest first.
    pub fn layers(&self) -> impl Iterator<Item = &Path> {
        self.layers.iter().map(|layer| layer.path.as_path())
    }

    /// The highest app layer's `relative` file.
    pub fn file(&self, relative: &str) -> Option<PathBuf> {
        self.layers
            .iter()
            .rev()
            .filter(|layer| !layer.package)
            .map(|layer| layer.path.join(relative))
            .find(|path| path.is_file())
    }

    /// Every layer's `relative` file, highest first: what to fall back on.
    pub fn candidates(&self, relative: &str) -> Vec<PathBuf> {
        let mut files = self.files(relative);
        files.reverse();
        files
    }

    /// Every layer's `main.luau` for the mod `name`, highest first: under
    /// `mods/<name>/`, or the root of a single-mod package of that name.
    pub fn mod_candidates(&self, name: &str) -> Vec<PathBuf> {
        let relative = format!("mods/{name}/main.luau");
        self.layers
            .iter()
            .rev()
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
        self.folder_layers()
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
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    found.insert(name.to_owned(), path.clone());
                }
            }
        }
        found
    }

    /// Each file `dir/<name>.<extension>`, by name; a higher layer's file
    /// replaces a lower one's.
    pub fn entries(&self, dir: &str, extension: &str) -> BTreeMap<String, PathBuf> {
        let mut found = BTreeMap::new();
        for layer in self.folder_layers() {
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

    /// The layers laid out like an app folder: all but single-mod packages.
    fn folder_layers(&self) -> impl Iterator<Item = &Path> {
        self.layers
            .iter()
            .filter(|layer| !layer.single_mod())
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
