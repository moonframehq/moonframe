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

mod cli;

pub(crate) use cli::{approve, run, update as print_update};

use {
    moonframe_core::{AppLayer, mod_name},
    moonframe_providers::{Kind, Manifest},
    serde::{Deserialize, Serialize},
    std::{
        collections::BTreeMap,
        env, fmt, fs,
        io::ErrorKind,
        path::{Path, PathBuf},
        process::Command,
    },
};

/// The list of installed packages, in the user's folder.
pub(crate) const FILE: &str = "packages.toml";
/// Where packages are cloned, in the user's folder.
const DIR: &str = "packages";
/// What makes a package a whole app, at its root.
pub(crate) const MANIFEST: &str = "moonframe.toml";

/// `moonframe.toml`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    app: App,
}

/// An app package's `[app]`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct App {
    /// The profile it installs as, unless the user names another.
    pub(crate) name: String,
    /// Its name for people: the menu bar's, and a bundle's. The name if
    /// none.
    #[serde(default)]
    pub(crate) title: Option<String>,
    /// Its icon, a PNG in the app's folder (best 1024 pixels square): the
    /// Dock's and a bundle's. `icon.png` if none is named.
    #[serde(default)]
    pub(crate) icon: Option<String>,
    /// Built-in mods it keeps; it brings every other mod it shows.
    #[serde(default)]
    pub(crate) keep: Vec<String>,
    /// Packages it needs, as `moonframe mod install` takes them.
    #[serde(default)]
    pub(crate) requires: Vec<String>,
}

impl App {
    /// The `[app]` of the package in `folder`, if it is an app.
    pub(crate) fn read(folder: &Path) -> Result<Option<Self>, String> {
        let path = folder.join(MANIFEST);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
        };
        let app = toml::from_str::<ManifestFile>(&text)
            .map_err(|error| format!("{}: {error}", path.display()))?
            .app;
        if !valid_name(&app.name) {
            return Err(format!(
                "{}: `{}` is not an app name: use a-z, 0-9, - and _",
                path.display(),
                printable(&app.name)
            ));
        }
        // A bundle is a folder named after the title, and its plist and
        // .desktop file hold it, so it must be a plain file name.
        if let Some(title) = &app.title
            && !valid_title(title)
        {
            return Err(format!(
                "{}: `{}` is not an app title: up to 64 characters, without \
                 control characters, / \\ < > & or a leading dot",
                path.display(),
                printable(title)
            ));
        }
        // The icon must stay inside the app's folder.
        if let Some(icon) = &app.icon
            && !valid_icon(icon)
        {
            return Err(format!(
                "{}: `{}` is not an icon path: a file inside the app's folder",
                path.display(),
                printable(icon)
            ));
        }
        Ok(Some(app))
    }
}

/// A name for a profile or app: lowercase letters, digits, `-` and `_`.
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'))
}

/// Whether `title` can name a bundle folder and sit in a plist unescaped.
pub(crate) fn valid_title(title: &str) -> bool {
    !title.trim().is_empty()
        && title.chars().count() <= 64
        && !title.starts_with('.')
        && !title
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '<' | '>' | '&' | ':'))
}

/// Whether `icon` is a relative path that stays inside the app's folder.
fn valid_icon(icon: &str) -> bool {
    !icon.is_empty()
        && !icon.chars().any(char::is_control)
        && Path::new(icon)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

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
pub(crate) fn enabled_layers(user: &Path, apps: bool) -> (Vec<PathBuf>, Option<AppLayer>) {
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
pub(crate) fn source_id(source: &str) -> Result<String, String> {
    Source::parse(source).map(|source| source.id)
}

/// Whether the package `id` is installed in `user`.
pub(crate) fn has(user: &Path, id: &str) -> Result<bool, String> {
    Ok(List::read(user)?.package.contains_key(id))
}

/// Whether `folder` is linked as a package in `user`.
pub(crate) fn linked(user: &Path, folder: &Path) -> Result<bool, String> {
    Ok(List::read(user)?.linked(folder).is_some())
}

/// Whether every package in `user` is a linked folder: none was cloned.
pub(crate) fn links_only(user: &Path) -> Result<bool, String> {
    Ok(List::read(user)?.package.values().all(|entry| entry.link))
}

/// What `install` was given: where to clone from, and the id it goes by.
#[derive(Debug, PartialEq)]
struct Source {
    id: String,
    url: String,
    reference: Option<String>,
}

impl Source {
    /// `github.com/owner/repo`, `owner/repo` (on GitHub), an `https://`,
    /// `ssh://` or `git@host:path` URL, or a folder; any of them may end in
    /// `@<ref>`: a branch, tag, or commit.
    fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (text, reference) = split_reference(text);
        let reference = match reference {
            Some("") => return Err(format!("`{text}@` names no ref")),
            // Git would read it as one of its options.
            Some(reference) if reference.starts_with('-') => {
                return Err(format!("`{}` is not a ref", printable(reference)));
            },
            reference => reference.map(str::to_owned),
        };
        if is_folder(text) {
            let folder = expand_home(text);
            let folder = std::path::absolute(&folder).unwrap_or(folder);
            let name = folder
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| format!("`{text}` names no folder"))?;
            return Ok(Self {
                id: format!("local/{}", strip_git(name)),
                url: folder.display().to_string(),
                reference,
            });
        }
        let (url, path) = if let Some((scheme, rest)) = text.split_once("://") {
            if !["https", "http", "ssh", "git", "file"].contains(&scheme) {
                return Err(format!("cannot clone from {scheme}:// URLs"));
            }
            // Without `user@` and `:port`.
            let rest = rest.rsplit_once('@').map_or(rest, |(_, rest)| rest);
            let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
            let host = host.split(':').next().unwrap_or(host);
            (text.to_owned(), format!("{host}/{path}"))
        } else if let Some((login, path)) = text.split_once(':')
            && let Some((_, host)) = login.split_once('@')
        {
            (text.to_owned(), format!("{host}/{path}"))
        } else if text.split('/').count() == 2 && !text.contains('.') {
            (
                format!("https://github.com/{text}"),
                format!("github.com/{text}"),
            )
        } else {
            (format!("https://{text}"), text.to_owned())
        };
        let id = strip_git(path.trim_matches('/'));
        let parts: Vec<&str> = id.split('/').collect();
        let valid = parts.len() >= 2
            && parts.iter().all(|part| {
                !part.is_empty()
                    && *part != "."
                    && *part != ".."
                    && part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            });
        if !valid {
            return Err(format!(
                "`{text}` is not a repository: try github.com/owner/repo, a git URL, or a folder"
            ));
        }
        Ok(Self {
            id: id.to_owned(),
            url,
            reference,
        })
    }

    /// The folder name the package is cloned to, which names a single mod.
    fn name(&self) -> &str {
        package_name(&self.id)
    }
}

/// The last part of a package's id: the folder name it is cloned to.
fn package_name(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

/// Whether `text` names a folder rather than a repository. `C:\` and
/// `\\server\` are folders too, on Windows.
fn is_folder(text: &str) -> bool {
    text.starts_with(['/', '.', '~']) || Path::new(text).is_absolute()
}

/// `text` without its `@<ref>`, and the ref. A repository's ref follows the
/// first `@` in its path, so a branch may hold `/` (`owner/repo@feature/x`),
/// unlike `git@host:` or `ssh://user@host/`, whose `@` comes before it. A
/// folder's is in its last part, since a folder may hold `@` anywhere.
fn split_reference(text: &str) -> (&str, Option<&str>) {
    let at = if is_folder(text) {
        let last = text.rfind(['/', '\\']).map_or(0, |slash| slash + 1);
        text[last..].rfind('@').map(|at| last + at)
    } else {
        let body = text.find("://").map_or(0, |scheme| scheme + 3);
        text[body..].find('/').and_then(|slash| {
            let path = body + slash;
            text[path..].find('@').map(|at| path + at)
        })
    };
    match at {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    }
}

/// Where `source`, a requirement in the moonframe.toml of the app in
/// `folder`, points when it is a package shipped beside the app: a `./` or
/// `../` path, which must stay inside the app's git repository (its folder,
/// outside one), so a downloaded app cannot reach the user's other folders.
/// `None` for a repository to clone. Other folders, `~` and `file://` are
/// refused: what an app requires must come with it, or from a repository.
pub(crate) fn beside(folder: Option<&Path>, source: &str) -> Result<Option<PathBuf>, String> {
    let source = source.trim();
    let shown = printable(source);
    if !(source.starts_with("./") || source.starts_with("../")) {
        if is_folder(source) || source.starts_with("file:") {
            return Err(format!(
                "`{shown}`: an app requires repositories, or packages shipped inside its own \
                 repository as ./ or ../ paths"
            ));
        }
        return Ok(None);
    }
    let folder = folder.ok_or_else(|| format!("`{shown}`: no app folder to find it beside"))?;
    let folder = folder
        .canonicalize()
        .map_err(|error| format!("{}: {error}", folder.display()))?;
    let root = git(&folder, &["rev-parse", "--show-toplevel"])
        .ok()
        .and_then(|root| PathBuf::from(root).canonicalize().ok())
        .filter(|root| folder.starts_with(root))
        .unwrap_or_else(|| folder.clone());
    let path = normalize(&folder.join(source));
    // Through any link it holds, where it really is.
    let path = path.canonicalize().unwrap_or(path);
    if !path.starts_with(&root) {
        return Err(format!(
            "`{shown}` leaves the app's repository, {}",
            root.display()
        ));
    }
    if path == folder {
        return Err(format!("`{shown}` is the app itself"));
    }
    Ok(Some(path))
}

/// `path` with its `.` and `..` parts worked out, without touching disk.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {},
            std::path::Component::ParentDir => {
                normal.pop();
            },
            part => normal.push(part),
        }
    }
    normal
}

fn strip_git(path: &str) -> &str {
    path.strip_suffix(".git").unwrap_or(path)
}

fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            env::var_os("HOME").map(PathBuf::from).map_or_else(
                || PathBuf::from(path),
                |home| home.join(rest.trim_start_matches('/')),
            )
        },
        _ => PathBuf::from(path),
    }
}

/// What a package holds.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Contents {
    /// An app's `[app]`: it brings a shell, and runs in a profile.
    pub(crate) app: Option<App>,
    mods: Vec<String>,
    providers: Vec<(String, Provider)>,
    components: Vec<String>,
    style: bool,
    /// What an app ships beside it and requires (`./shared`), by the
    /// requirement: approved with the app, since a new version of the app
    /// brings a new version of them.
    beside: Vec<(String, Contents)>,
}

/// How a provider in a package would run.
#[derive(Debug, PartialEq)]
pub(crate) enum Provider {
    /// A command, with the user's access.
    Process(Vec<String>),
    /// A Rust crate the app builds with Cargo, which runs its build code
    /// with the user's access; the component it builds runs sandboxed.
    Source,
    /// The app's own built-in provider of that name, run with the
    /// package's `[settings]`, which may widen what it does (git's
    /// `writers`).
    Native(String),
    /// A JSON file it publishes as it is; nothing runs.
    Static(String),
    Broken(String),
}

impl Contents {
    /// Reads the package in `folder`, which is installed as `name`, and an
    /// app's, what it ships beside it.
    fn read(folder: &Path, name: &str) -> Result<Self, String> {
        let mut contents = Self::read_one(folder, name)?;
        let requires = contents
            .app
            .as_ref()
            .map(|app| app.requires.clone())
            .unwrap_or_default();
        for source in requires {
            if let Some(path) = beside(Some(folder), &source)?
                && path.is_dir()
            {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&source)
                    .to_owned();
                let shipped = Self::read_one(&path, &name)?;
                contents.beside.push((source, shipped));
            }
        }
        Ok(contents)
    }

    /// Adds the linked packages at `links` inside `folder` that it does not
    /// already ship beside it: an update must not change what they run
    /// unasked just because it stopped requiring them.
    fn add_links(&mut self, folder: &Path, links: &[PathBuf]) -> Result<(), String> {
        for inside in links {
            let source = format!("./{}", inside.display());
            let path = folder.join(inside);
            let listed = self
                .beside
                .iter()
                .any(|(shipped, _)| Path::new(shipped.trim_start_matches("./")) == inside);
            if listed || !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&source)
                .to_owned();
            let linked = Self::read_one(&path, &name)?;
            self.beside.push((source, linked));
        }
        Ok(())
    }

    fn read_one(folder: &Path, name: &str) -> Result<Self, String> {
        refuse_binaries(folder)?;
        let mut contents = Self {
            app: App::read(folder)?,
            ..Self::default()
        };
        if contents.app.is_none() && folder.join("main.luau").is_file() {
            contents.mods.push(mod_name(name));
            return Ok(contents);
        }
        contents.mods = names(&folder.join("mods"), |path| {
            path.join("main.luau").is_file()
        });
        for provider in names(&folder.join("providers"), |path| {
            path.join("provider.toml").is_file()
        }) {
            let dir = folder.join("providers").join(&provider);
            let how = match Manifest::load(&provider, &dir) {
                Ok(Manifest {
                    kind: Kind::Process { command },
                    ..
                }) => Provider::Process(command),
                Ok(Manifest {
                    kind: Kind::Native,
                    settings,
                    ..
                }) => Provider::Native(settings.to_string()),
                Ok(Manifest {
                    kind: Kind::Static { file },
                    ..
                }) => Provider::Static(file),
                Ok(_) => Provider::Source,
                Err(problem) => Provider::Broken(problem.to_string()),
            };
            contents.providers.push((provider, how));
        }
        contents.components = names(&folder.join("components"), |path| {
            path.extension()
                .is_some_and(|extension| extension == "luau")
        })
        .into_iter()
        .map(|name| name.trim_end_matches(".luau").to_owned())
        .collect();
        contents.style = folder.join("style.luau").is_file();
        if contents.app.is_some() && !folder.join("main.luau").is_file() {
            return Err(format!(
                "{} is an app without a shell: it needs a main.luau",
                folder.display()
            ));
        }
        if contents == Self::default() {
            return Err(format!(
                "{} is not a Moonframe package: it has no main.luau, mods/, providers/, \
                 components/, or style.luau",
                folder.display()
            ));
        }
        Ok(contents)
    }

    /// Its mods named in `writers`, the mods the git provider lets commit
    /// and push: a package's mod of that name takes the place of the
    /// user's, and with it that right.
    pub(crate) fn writers<'a>(&'a self, writers: &'a [String]) -> impl Iterator<Item = &'a str> {
        self.mods
            .iter()
            .filter(|name| writers.contains(name))
            .map(String::as_str)
    }

    /// The providers that would run code with the user's access.
    pub(crate) fn trusted(&self) -> impl Iterator<Item = (&str, &Provider)> {
        self.providers
            .iter()
            .filter(|(_, how)| {
                matches!(
                    how,
                    Provider::Process(_) | Provider::Source | Provider::Native(_)
                )
            })
            .map(|(name, how)| (name.as_str(), how))
    }

    /// This package, then each one it ships beside it, with the requirement
    /// that names it.
    pub(crate) fn parts(&self) -> impl Iterator<Item = (Option<&str>, &Self)> {
        std::iter::once((None, self)).chain(
            self.beside
                .iter()
                .map(|(source, contents)| (Some(source.as_str()), contents)),
        )
    }
}

/// Files a package may hold that are not text: images, which a reviewer can
/// look at, known by how they start rather than by their name, so a
/// program cannot pass as `icon.png`.
const IMAGES: &[(&[&str], &[u8])] = &[
    (&["png"], b"\x89PNG\r\n\x1a\n"),
    (&["jpg", "jpeg"], b"\xff\xd8\xff"),
    (&["gif"], b"GIF8"),
    (&["webp"], b"RIFF"),
    (&["ico"], b"\0\0\x01\0"),
    (&["icns"], b"icns"),
];

/// Whether `bytes`, a file named with `extension`, are the image its name
/// says.
fn is_image(bytes: &[u8], extension: &str) -> bool {
    let Some((_, magic)) = IMAGES
        .iter()
        .find(|(extensions, _)| extensions.contains(&extension))
    else {
        return false;
    };
    bytes.starts_with(magic) && (extension != "webp" || bytes.get(8..12) == Some(b"WEBP"))
}

/// Refuses a package holding anything but source and images, such as a
/// prebuilt `provider.wasm` or an executable a provider's command could
/// run: everything a package runs must be readable, by its users and their
/// agents, before it is installed. A wasm provider ships its crate, which
/// the app builds. A link is refused too: what it points to, outside the
/// package, is not what was reviewed.
fn refuse_binaries(folder: &Path) -> Result<(), String> {
    let mut opaque: Vec<String> = files(folder)
        .into_iter()
        .filter_map(|relative| {
            let path = folder.join(&relative);
            let shown = printable(&relative.display().to_string());
            if path.symlink_metadata().is_ok_and(|meta| meta.is_symlink()) {
                return Some(format!("{shown} (a link)"));
            }
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase);
            if extension.as_deref() == Some("wasm") {
                return Some(shown);
            }
            let Ok(bytes) = fs::read(&path) else {
                return Some(shown);
            };
            let readable =
                is_text(&bytes) || extension.is_some_and(|extension| is_image(&bytes, &extension));
            (!readable).then_some(shown)
        })
        .collect();
    if opaque.is_empty() {
        return Ok(());
    }
    opaque.sort();
    Err(format!(
        "a package ships source only, so it can be reviewed before it runs; {} holds {}. \
         A wasm provider ships its crate (Cargo.toml and src/), which the app builds",
        folder.display(),
        opaque.join(", ")
    ))
}

/// Every file and link in `folder`, relative to it: in a git repository,
/// those it tracks and those it would (all of them, in a fresh clone), so a
/// linked folder's ignored build output does not count; elsewhere, every
/// one. A link is listed, not followed.
fn files(folder: &Path) -> Vec<PathBuf> {
    let listed = git(folder, &[
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ]);
    if let Ok(listed) = listed
        && folder.join(".git").exists()
    {
        return listed
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            // Deleted but not yet committed.
            .filter(|path| {
                folder
                    .join(path)
                    .symlink_metadata()
                    .is_ok_and(|meta| meta.is_file() || meta.is_symlink())
            })
            .collect();
    }
    let mut found = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let Ok(entries) = fs::read_dir(folder.join(&relative)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = relative.join(entry.file_name());
            match entry.file_type() {
                Ok(kind) if kind.is_dir() && entry.file_name() != ".git" => pending.push(path),
                Ok(kind) if kind.is_file() || kind.is_symlink() => found.push(path),
                _ => {},
            }
        }
    }
    found
}

/// Whether a file's `bytes` read as text: valid UTF-8 without NUL bytes.
fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

/// The entries of `dir` that `keep` accepts, by name.
fn names(dir: &Path, keep: impl Fn(&Path) -> bool) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| keep(path))
        .filter_map(|path| Some(path.file_name()?.to_str()?.to_owned()))
        .collect();
    found.sort();
    found
}

/// `text`, from a package, safe to print: control characters, such as a
/// terminal escape that could redraw the approval prompt, and invisible
/// ones, such as a right-to-left override that could disguise a name, are
/// shown escaped instead of sent to the terminal.
pub(crate) fn printable(text: &str) -> String {
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

impl fmt::Display for Contents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = |names: &[String]| {
            names
                .iter()
                .map(|name| printable(name))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut parts = Vec::new();
        if let Some(app) = &self.app {
            parts.push(format!("the app {}, with its shell", printable(&app.name)));
            if !app.keep.is_empty() {
                parts.push(format!("built-in mods it keeps: {}", list(&app.keep)));
            }
        }
        if !self.mods.is_empty() {
            parts.push(format!("mods: {}", list(&self.mods)));
        }
        if !self.providers.is_empty() {
            let providers: Vec<String> = self
                .providers
                .iter()
                .map(|(name, how)| format!("{} ({how})", printable(name)))
                .collect();
            parts.push(format!("providers: {}", providers.join(", ")));
        }
        if !self.components.is_empty() {
            parts.push(format!("components: {}", list(&self.components)));
        }
        if self.style {
            parts.push("a stylesheet".to_owned());
        }
        if let Some(app) = self.app.as_ref().filter(|app| !app.requires.is_empty()) {
            parts.push(format!("requires: {}", list(&app.requires)));
        }
        for (source, contents) in &self.beside {
            parts.push(format!("ships {}, with {contents}", printable(source)));
        }
        f.write_str(&parts.join("; "))
    }
}

impl fmt::Display for Provider {
    /// Each argument of a command quoted on its own, so where one ends shows.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(command) => {
                let words: Vec<String> = command.iter().map(|word| format!("{word:?}")).collect();
                write!(f, "runs [{}] with your access", words.join(" "))
            },
            Self::Source => f.write_str("built with Cargo, with your access"),
            Self::Native(settings) => write!(
                f,
                "replaces the app's built-in provider of this name, with settings {}",
                printable(settings)
            ),
            Self::Static(file) => write!(f, "publishes {}, running nothing", printable(file)),
            Self::Broken(problem) => write!(f, "broken: {}", printable(problem)),
        }
    }
}

/// The built-in git provider's manifest, compiled in so its `writers` are
/// known wherever the command runs from.
const BUILTIN_GIT: &str = include_str!("../../../app/providers/git/provider.toml");

/// Every mod a git provider here may let commit and push: `[settings]
/// writers` in the built-in `providers/git`, the user's, and every installed
/// package's, enabled or not (one can be enabled without asking). Which
/// layer wins depends on what is installed, so all of them count.
pub(crate) fn git_writers(user: &Path) -> Vec<String> {
    let mut texts = vec![BUILTIN_GIT.to_owned()];
    let mut folders = vec![user.to_owned()];
    if let Ok(list) = List::read(user) {
        folders.extend(
            list.package
                .iter()
                .map(|(id, entry)| entry.folder(user, id)),
        );
    }
    texts.extend(
        folders.iter().filter_map(|folder| {
            fs::read_to_string(folder.join("providers/git/provider.toml")).ok()
        }),
    );
    let mut writers: Vec<String> = texts
        .iter()
        .filter_map(|text| text.parse::<toml::Table>().ok())
        .filter_map(|manifest| {
            manifest
                .get("settings")?
                .get("writers")?
                .as_array()
                .cloned()
        })
        .flatten()
        .filter_map(|name| name.as_str().map(str::to_owned))
        .collect();
    writers.sort();
    writers.dedup();
    writers
}

/// Asked before a package's code may run: `Err` refuses it.
pub(crate) type Approve<'a> = &'a dyn Fn(&Contents) -> Result<(), String>;

/// Clones `source` into the user's folder and enables it.
pub(crate) fn install(
    user: &Path,
    source: &str,
    approve: Approve,
) -> Result<(String, Contents), String> {
    let source = Source::parse(source)?;
    let mut list = List::read(user)?;
    if list.package.contains_key(&source.id) {
        return Err(format!(
            "{} is already installed; `moonframe mod update {}` fetches the latest",
            source.id,
            source.name()
        ));
    }
    let packages = user.join(DIR);
    let target = packages.join(&source.id);
    if target.exists() {
        return Err(format!(
            "{} already exists; move it away first",
            target.display()
        ));
    }
    // Cloned beside the others, so it moves into place in one rename.
    let incoming = packages.join(format!(".incoming-{}", std::process::id()));
    let _ = fs::remove_dir_all(&incoming);
    fs::create_dir_all(&packages).map_err(|error| format!("{}: {error}", packages.display()))?;
    let installed = (|| {
        git(&packages, &[
            "clone",
            "--quiet",
            "--",
            &source.url,
            &incoming.display().to_string(),
        ])?;
        if let Some(reference) = &source.reference {
            checkout(&incoming, Some(reference))?;
        }
        let contents = Contents::read(&incoming, source.name())?;
        approve(&contents)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::rename(&incoming, &target).map_err(|error| format!("{}: {error}", target.display()))?;
        Ok(contents)
    })();
    let contents = match installed {
        Ok(contents) => contents,
        Err(problem) => {
            let _ = fs::remove_dir_all(&incoming);
            return Err(problem);
        },
    };
    list.package.insert(source.id.clone(), Entry {
        url: source.url,
        reference: source.reference,
        link: false,
        enabled: true,
    });
    // Unlisted, the folder would only block the next install.
    if let Err(problem) = list.write(user) {
        let _ = fs::remove_dir_all(&target);
        return Err(problem);
    }
    Ok((source.id, contents))
}

/// Uses `folder` in place as a package, so edits show as they are made.
pub(crate) fn link(
    user: &Path,
    folder: &str,
    approve: Approve,
) -> Result<(String, Contents), String> {
    let folder = expand_home(folder);
    let folder = folder
        .canonicalize()
        .map_err(|error| format!("{}: {error}", folder.display()))?;
    let name = folder
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no name", folder.display()))?;
    let url = folder.display().to_string();
    let mut list = List::read(user)?;
    if let Some(id) = list.linked(&folder) {
        return Err(format!("{id} is already installed"));
    }
    // Two folders of one name, such as `a/shared` and `b/shared`, are two
    // packages.
    let mut id = format!("local/{name}");
    let mut n = 1;
    while list.package.contains_key(&id) {
        n += 1;
        id = format!("local/{name}-{n}");
    }
    let contents = Contents::read(&folder, name)?;
    approve(&contents)?;
    list.package.insert(id.clone(), Entry {
        url,
        reference: None,
        link: true,
        enabled: true,
    });
    list.write(user)?;
    Ok((id, contents))
}

/// What `update` did to one package.
#[derive(Debug, PartialEq)]
pub(crate) enum Updated {
    Current(String),
    Moved { from: String, to: String },
    Linked,
}

/// Fetches each named package, or all of them, and checks out the latest
/// of what it follows. A package whose new version `approve` refuses stays
/// where it was.
pub(crate) fn update(
    user: &Path,
    query: Option<&str>,
    approve: Approve,
) -> Result<Vec<(String, Result<Updated, String>)>, String> {
    let list = List::read(user)?;
    let ids = match query {
        Some(query) => vec![list.find(query)?],
        None => list.package.keys().cloned().collect(),
    };
    Ok(ids
        .into_iter()
        .map(|id| {
            let entry = &list.package[&id];
            let folder = entry.folder(user, &id);
            let name = package_name(&id);
            let updated = if entry.link {
                Ok(Updated::Linked)
            } else {
                let links = list.links_inside(&folder);
                update_one(&folder, name, entry.reference.as_deref(), &links, approve)
            };
            (id, updated)
        })
        .collect())
}

fn update_one(
    folder: &Path,
    name: &str,
    reference: Option<&str>,
    links: &[PathBuf],
    approve: Approve,
) -> Result<Updated, String> {
    if !folder.is_dir() {
        return Err(format!(
            "{} is missing; remove the package and install it again",
            folder.display()
        ));
    }
    let before = head(folder)?;
    git(folder, &["fetch", "--quiet", "--tags", "--force", "origin"])?;
    // The default branch may have changed; following it is best effort.
    let _ = git(folder, &["remote", "set-head", "origin", "--auto"]);
    let after = resolve(folder, reference)?;
    if after == before {
        return Ok(Updated::Current(short(&after)));
    }
    // The package's folder is a live layer: an open window would start a
    // new provider the moment it appeared there. So the new version is
    // read in a checkout of its own, outside every layer, and reaches the
    // folder only once approved.
    let incoming = folder.with_file_name(format!(".incoming-{}", std::process::id()));
    let _ = fs::remove_dir_all(&incoming);
    let shown = incoming.display().to_string();
    git(folder, &[
        "worktree", "add", "--quiet", "--detach", &shown, &after,
    ])?;
    let approved = Contents::read(&incoming, name)
        .and_then(|mut contents| {
            contents.add_links(&incoming, links)?;
            Ok(contents)
        })
        .and_then(|contents| approve(&contents));
    let _ = git(folder, &["worktree", "remove", "--force", &shown]);
    let _ = fs::remove_dir_all(&incoming);
    let _ = git(folder, &["worktree", "prune"]);
    approved?;
    git(folder, &["checkout", "--quiet", "--detach", &after])
        .map_err(|problem| format!("cannot check out {}: {problem}", short(&after)))?;
    Ok(Updated::Moved {
        from: short(&before),
        to: short(&after),
    })
}

/// Uninstalls a package; a linked folder is left as it is.
pub(crate) fn remove(user: &Path, query: &str) -> Result<String, String> {
    let mut list = List::read(user)?;
    let id = list.find(query)?;
    let Some(entry) = list.package.remove(&id) else {
        return Err(format!("no package `{id}`"));
    };
    // The folder goes first: listed but missing, `update` names it; unlisted
    // but there, it would block the next install.
    if !entry.link {
        let folder = entry.folder(user, &id);
        match fs::remove_dir_all(&folder) {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                return Err(format!("{}: {error}", folder.display()));
            },
            _ => {},
        }
        // Drops the owner and host folders it leaves empty.
        let packages = user.join(DIR);
        for parent in folder.ancestors().skip(1) {
            if parent == packages
                || !parent.starts_with(&packages)
                || fs::remove_dir(parent).is_err()
            {
                break;
            }
        }
    }
    list.write(user)?;
    Ok(id)
}

/// Turns a package on or off without uninstalling it.
pub(crate) fn set_enabled(user: &Path, query: &str, enabled: bool) -> Result<String, String> {
    let mut list = List::read(user)?;
    let id = list.find(query)?;
    if let Some(entry) = list.package.get_mut(&id) {
        entry.enabled = enabled;
    }
    list.write(user)?;
    Ok(id)
}

/// One installed package, for `mod list`.
pub(crate) struct Installed {
    pub(crate) id: String,
    pub(crate) enabled: bool,
    pub(crate) link: bool,
    pub(crate) reference: Option<String>,
    /// The checked-out commit, abbreviated.
    pub(crate) commit: Option<String>,
    pub(crate) contents: Result<Contents, String>,
}

pub(crate) fn installed(user: &Path) -> Result<Vec<Installed>, String> {
    Ok(List::read(user)?
        .package
        .into_iter()
        .map(|(id, entry)| {
            let folder = entry.folder(user, &id);
            let name = package_name(&id).to_owned();
            Installed {
                commit: head(&folder).ok().map(|commit| short(&commit)),
                contents: if folder.is_dir() {
                    Contents::read(&folder, &name)
                } else {
                    Err(format!("{} is missing", folder.display()))
                },
                id,
                enabled: entry.enabled,
                link: entry.link,
                reference: entry.reference,
            }
        })
        .collect())
}

/// Starts a package in the new folder `folder`: a git repository holding
/// one mod, ready to edit, link, and push.
pub(crate) fn create(folder: &Path) -> Result<String, String> {
    let name = folder
        .file_name()
        .and_then(|name| name.to_str())
        .map(mod_name)
        .ok_or_else(|| format!("{} has no name", folder.display()))?;
    if folder.exists()
        && fs::read_dir(folder)
            .map_err(|error| format!("{}: {error}", folder.display()))?
            .next()
            .is_some()
    {
        return Err(format!(
            "{} already exists and is not empty",
            folder.display()
        ));
    }
    fs::create_dir_all(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let write = |file: &str, text: String| {
        fs::write(folder.join(file), text).map_err(|error| format!("{file}: {error}"))
    };
    write("main.luau", MAIN.replace("{name}", &name))?;
    write(
        "README.md",
        README
            .replace("{name}", &name)
            .replace("{repository}", env!("CARGO_PKG_REPOSITORY")),
    )?;
    git_init(folder)?;
    Ok(name)
}

/// Makes `folder` a new git repository.
pub(crate) fn git_init(folder: &Path) -> Result<(), String> {
    git(folder, &["init", "--quiet"]).map(drop)
}

const MAIN: &str = r#"-- {name}: a mod for moonframe. `region` picks where the shell draws it:
-- left, right, main, bottom, status, or status-right.
return mod {
	region = "right",
	title = "{name}",

	init = function(previous)
		return previous or { count = 0 }
	end,

	update = function(state, msg)
		if msg == "add" then
			state.count += 1
		end
	end,

	view = function(state)
		return ui.panel {
			title = "{name}",
			ui.text(`Clicked {state.count} times`),
			ui.button { "Add", msg = "add" },
		}
	end,
}
"#;

/// A new package's README. `{repository}` is the app's own, from the
/// workspace's `repository` in Cargo.toml, so a fork links to itself.
const README: &str = "# {name}

A mod for [Moonframe]({repository}).

## Install

```sh
moonframe mod install github.com/<owner>/{name}
```

## Develop

```sh
moonframe mod link .   # the window shows your edits as you save
```
";

/// Runs git in `dir`, never asking for a password: a private repository
/// needs credentials git already has.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // How a missing or private repository answers over HTTPS.
        let hint = if stderr.contains("terminal prompts disabled") {
            "\n(no such public repository: check the name, or use an SSH URL for a private one)"
        } else {
            ""
        };
        Err(format!("git {}: {}{hint}", args[0], stderr.trim()))
    }
}

/// Checks out what `reference` names, preferring the remote's branch of
/// that name, else the remote's default branch.
fn checkout(dir: &Path, reference: Option<&str>) -> Result<(), String> {
    let target = resolve(dir, reference)?;
    git(dir, &["checkout", "--quiet", "--detach", &target])
        .map(drop)
        .map_err(|problem| format!("cannot check out {}: {problem}", short(&target)))
}

/// The commit `reference` names, as [`checkout`] picks it.
fn resolve(dir: &Path, reference: Option<&str>) -> Result<String, String> {
    // From a hand-edited packages.toml, git would read it as an option.
    if let Some(reference) = reference.filter(|reference| reference.starts_with('-')) {
        return Err(format!("`{}` is not a ref", printable(reference)));
    }
    let target = match reference {
        None => "origin/HEAD".to_owned(),
        Some(reference) => {
            let branch = format!("origin/{reference}");
            if git(dir, &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{branch}^{{commit}}"),
            ])
            .is_ok()
            {
                branch
            } else {
                reference.to_owned()
            }
        },
    };
    git(dir, &[
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("{target}^{{commit}}"),
    ])
    .map_err(|_| format!("no branch, tag, or commit {target}"))
}

fn head(dir: &Path) -> Result<String, String> {
    git(dir, &["rev-parse", "HEAD"])
}

fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
