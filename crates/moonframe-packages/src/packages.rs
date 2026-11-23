//! Packages: mods, providers, components, and styles others publish as git
//! repositories. `moonframe mod install github.com/owner/repo` clones
//! one into `packages/<host>/<owner>/<repo>` in the user's folder and lists
//! it in `packages.toml` there; each enabled package is laid between the
//! built-in app and the user's own folder (see [`Roots`]).
//!
//! A repository is either one mod, with `main.luau` at its root, or laid
//! out like an app folder: `mods/`, `providers/`, `components/`, and
//! `style.luau`. A package never replaces the shell, and ships source only:
//! no `provider.wasm` or other binaries, so the user, or an agent, can
//! review all it runs.
//!
//! A repository with a `moonframe.toml` naming an `[app]` is a whole app:
//! its own shell, mods, and the packages it requires. It installs into a
//! profile of its own (`moonframe app install`, see `profiles`), never over
//! the user's starter app.
//!
//! [`Roots`]: moonframe_core::Roots

// keep-sorted start
mod cli;
mod contents;
mod create;
mod git;
mod install;
mod manifest;
mod source;
// keep-sorted end

pub use {
    cli::{approve, run, update as print_update},
    contents::Contents,
    create::git_init,
    install::{install, installed, link},
    manifest::{App, valid_name, valid_scheme},
    source::beside,
};
pub(crate) use {
    create::create,
    install::{Approve, Updated, remove, set_enabled, update},
    manifest::valid_title,
};

use {
    moonframe_core::AppLayer,
    serde::{Deserialize, Serialize},
    source::Source,
    std::{
        collections::BTreeMap,
        fs,
        io::ErrorKind,
        path::{Path, PathBuf},
    },
};

/// The list of installed packages, in the user's folder.
pub const FILE: &str = "packages.toml";
/// Where packages are cloned, in the user's folder.
const DIR: &str = "packages";
/// What makes a package a whole app, at its root.
pub const MANIFEST: &str = "moonframe.toml";

/// `packages.toml`.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    #[serde(default)]
    package: BTreeMap<String, Entry>,
}

/// A package, by its id: `github.com/owner/repo`, or `local/<folder>` for
/// a linked folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    /// Where it is cloned from, or the linked folder.
    url: String,
    /// The branch, tag, or commit it follows; the remote's default branch
    /// if none.
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    reference: Option<String>,
    /// A folder used in place rather than a clone, for developing a package.
    #[serde(default, skip_serializing_if = "is_false")]
    link: bool,
    #[serde(default = "enabled")]
    enabled: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn enabled() -> bool {
    true
}

impl Entry {
    fn folder(&self, user: &Path, id: &str) -> PathBuf {
        if self.link {
            PathBuf::from(&self.url)
        } else {
            user.join(DIR).join(id)
        }
    }
}

impl List {
    fn read(user: &Path) -> Result<Self, String> {
        let path = user.join(FILE);
        match fs::read_to_string(&path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
            },
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("cannot read {}: {error}", path.display())),
        }
    }

    fn write(&self, user: &Path) -> Result<(), String> {
        let path = user.join(FILE);
        let text = toml::to_string(self).map_err(|error| error.to_string())?;
        fs::create_dir_all(user).map_err(|error| format!("{}: {error}", user.display()))?;
        fs::write(&path, format!("# Written by `moonframe mod`.\n\n{text}"))
            .map_err(|error| format!("{}: {error}", path.display()))
    }

    /// The id of the package linked from `folder`, if one is.
    fn linked(&self, folder: &Path) -> Option<String> {
        self.package
            .iter()
            .find(|(_, entry)| entry.link && Path::new(&entry.url) == folder)
            .map(|(id, _)| id.clone())
    }

    /// Where linked packages sit inside `folder`, relative to it: what an
    /// app shipped beside it and the profile linked, whether or not its
    /// new version still requires them, since they stay live layers.
    fn links_inside(&self, folder: &Path) -> Vec<PathBuf> {
        let root = folder.canonicalize().unwrap_or_else(|_| folder.to_owned());
        self.package
            .values()
            .filter(|entry| entry.link)
            .filter_map(|entry| Path::new(&entry.url).strip_prefix(&root).ok())
            .filter(|inside| !inside.as_os_str().is_empty())
            .map(Path::to_owned)
            .collect()
    }

    /// The id `query` names: an id, a source, or the end of one id, such as
    /// `my-mod` for `github.com/someone/my-mod`.
    fn find(&self, query: &str) -> Result<String, String> {
        if self.package.contains_key(query) {
            return Ok(query.to_owned());
        }
        if let Ok(source) = Source::parse(query)
            && self.package.contains_key(&source.id)
        {
            return Ok(source.id);
        }
        let suffix = format!("/{}", query.trim_matches('/')).to_lowercase();
        let found: Vec<&String> = self
            .package
            .keys()
            .filter(|id| id.to_lowercase().ends_with(&suffix))
            .collect();
        match found.as_slice() {
            [id] => Ok((*id).clone()),
            [] => Err(format!(
                "no package `{query}` is installed; see `moonframe mod list`"
            )),
            many => Err(format!(
                "`{query}` could be {}",
                many.iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(" or ")
            )),
        }
    }
}

/// The folders of the enabled packages, in the order they are laid: by id.
/// A broken `packages.toml` is logged and loads none. An app package is
/// laid on its own, above the others, when `apps` allows one (in a
/// profile); otherwise it is left out.
pub fn enabled_layers(user: &Path, apps: bool) -> (Vec<PathBuf>, Option<AppLayer>) {
    let list = match List::read(user) {
        Ok(list) => list,
        Err(problem) => {
            tracing::warn!("no packages: {problem}");
            return (Vec::new(), None);
        },
    };
    let mut folders = Vec::new();
    let mut app: Option<AppLayer> = None;
    for (id, entry) in list.package.iter().filter(|(_, entry)| entry.enabled) {
        let folder = entry.folder(user, id);
        if !folder.is_dir() {
            tracing::warn!("package folder {} is missing", folder.display());
            continue;
        }
        match App::read(&folder) {
            Ok(None) => folders.push(folder),
            Ok(Some(_)) if !apps => {
                tracing::warn!("{id} is an app: it runs in a profile of its own");
            },
            Ok(Some(_)) if app.is_some() => {
                tracing::warn!("{id} is a second app in one profile; left out");
            },
            Ok(Some(manifest)) => {
                app = Some(AppLayer {
                    path: folder,
                    keep: manifest.keep,
                });
            },
            Err(problem) => tracing::warn!("{problem}"),
        }
    }
    (folders, app)
}

/// The id `source` installs as, as `moonframe mod list` shows it.
pub fn source_id(source: &str) -> Result<String, String> {
    Source::parse(source).map(|source| source.id)
}

/// Whether the package `id` is installed in `user`.
pub fn has(user: &Path, id: &str) -> Result<bool, String> {
    Ok(List::read(user)?.package.contains_key(id))
}

/// Whether `folder` is linked as a package in `user`.
pub fn linked(user: &Path, folder: &Path) -> Result<bool, String> {
    Ok(List::read(user)?.linked(folder).is_some())
}

/// Whether every package in `user` is a linked folder: none was cloned.
pub fn links_only(user: &Path) -> Result<bool, String> {
    Ok(List::read(user)?.package.values().all(|entry| entry.link))
}

/// `text`, from a package, safe to print: control characters, such as a
/// terminal escape that could redraw the approval prompt, and invisible
/// ones, such as a right-to-left override that could disguise a name, are
/// shown escaped instead of sent to the terminal.
pub fn printable(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control() || invisible(c) {
            shown.extend(c.escape_unicode());
        } else {
            shown.push(c);
        }
    }
    shown
}

/// Characters that print as nothing but change what is around them:
/// direction marks, overrides and isolates, and zero-width ones.
fn invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

/// The built-in providers that trust the mods named in their `[settings]`
/// with privileged messages, and the keys that name them: start coding
/// agents that run as the user (acp's `starters`, and `allow`, whose keys
/// are mods whose agents' tools go unasked), save files (docs), commit and
/// push (git), ask for a Keychain item or reach an SSH host (usage). Their
/// manifests are compiled in so the names are known wherever the command
/// runs from. A native provider that gates on mod names belongs here.
const GATED: &[(&str, &str, &[&str])] = &[
    // keep-sorted start
    (
        "acp",
        include_str!("../../../app/providers/acp/provider.toml"),
        &["allow", "starters"],
    ),
    (
        "docs",
        include_str!("../../../app/providers/docs/provider.toml"),
        &["writers"],
    ),
    (
        "git",
        include_str!("../../../app/providers/git/provider.toml"),
        &["writers"],
    ),
    (
        "usage",
        include_str!("../../../app/providers/usage/provider.toml"),
        &["writers"],
    ),
    // keep-sorted end
];

/// Every mod a built-in provider here may trust with a privileged message:
/// the names under each [`GATED`] provider's keys, from its built-in
/// manifest, the user's, and every installed package's, enabled or not
/// (one can be enabled without asking). Which layer wins depends on what
/// is installed, so all of them count.
pub fn writers(user: &Path) -> Vec<String> {
    let mut manifests: Vec<(String, &[&str])> = GATED
        .iter()
        .map(|(_, text, keys)| ((*text).to_owned(), *keys))
        .collect();
    let mut folders = vec![user.to_owned()];
    if let Ok(list) = List::read(user) {
        folders.extend(
            list.package
                .iter()
                .map(|(id, entry)| entry.folder(user, id)),
        );
    }
    for folder in &folders {
        manifests.extend(GATED.iter().filter_map(|(name, _, keys)| {
            let path = folder.join("providers").join(name).join("provider.toml");
            fs::read_to_string(path).ok().map(|text| (text, *keys))
        }));
    }
    let mut writers: Vec<String> = manifests
        .iter()
        .filter_map(|(text, keys)| {
            let manifest = text.parse::<toml::Table>().ok()?;
            let settings = manifest.get("settings")?.as_table()?.clone();
            Some(
                keys.iter()
                    .filter_map(move |key| settings.get(*key).cloned()),
            )
        })
        .flatten()
        .flat_map(|value| match value {
            // A list of mod names, or a table keyed by them (acp's `allow`).
            toml::Value::Array(names) => names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect(),
            toml::Value::Table(table) => table.keys().cloned().collect(),
            _ => Vec::new(),
        })
        .collect();
    writers.sort();
    writers.dedup();
    writers
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
