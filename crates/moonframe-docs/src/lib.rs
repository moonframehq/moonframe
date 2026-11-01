//! The `docs` native provider: the project's text files, read and saved for
//! the mods that edit them (the markdown editor). Views can't read files, so
//! a mod asks this provider and gets the answer in its own `update`.
//!
//! It publishes `app.docs = { available, root }`, or `{ available = false,
//! reason }` when there is no project. The root is the one the `files`
//! provider lists: `MOONFRAME_FILES`, else the git repository the app runs
//! from.
//!
//! Messages, each answered to the mod that sent it with
//! `{ reply = id, action, ok, path, ... }` (`reply` is the action's name when
//! the mod gave no `id`):
//! - `{ read = "<path>", id }` answers with `text` and `version`, and keeps
//!   watching the file for that mod: when it changes on disk, the mod gets
//!   `{ changed = "<path>", version }` (`missing = true` once it is gone);
//! - `{ write = "<path>", text, version, force, id }` saves `text`, answering
//!   with the new `version`. When the file changed since `version` (or exists
//!   and no `version` is given), it refuses with `conflict = true` unless
//!   `force` is set, so an edit never silently overwrites someone else's;
//! - `{ close = "<path>" }` stops watching it.
//!
//! Paths are relative to the root, without `.`, `..` or empty parts, never
//! inside `.git`, and must stay in the root after following links. Files are
//! UTF-8 text of at most [`MAX_DOCUMENT`] bytes. Saving writes a temporary
//! file beside the original and renames it over, keeping its permissions.
//! Anyone may read; only the mods in provider.toml's `writers` (and the
//! components they draw) may write.

use {
    moonframe_core::{Data, MAX_DOCUMENT},
    moonframe_providers::{Handle, Native},
    std::{
        collections::BTreeMap,
        fs,
        io::Write as _,
        path::{Component, Path, PathBuf},
        time::UNIX_EPOCH,
    },
};

/// Most files watched at once, over every mod.
const MAX_OPEN: usize = 64;

/// Makes the provider, for the app's registry.
pub fn native() -> Box<dyn Native> {
    Box::new(Docs::default())
}

#[derive(Default)]
pub struct Docs {
    /// The project's folder, with links resolved.
    root: Option<PathBuf>,
    /// The mods that may save (`[settings] writers`).
    writers: Vec<String>,
    /// The files mods read, by mod and path, with the version each last saw.
    open: BTreeMap<(String, String), String>,
}

impl Native for Docs {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        self.root = root(host.dir());
        self.writers = writers(host.settings());
        host.publish(match &self.root {
            Some(root) => map([
                ("available", Data::Bool(true)),
                ("root", Data::String(root.to_string_lossy().into_owned())),
            ]),
            None => map([
                ("available", Data::Bool(false)),
                (
                    "reason",
                    Data::from(
                        "not in a git repository; set MOONFRAME_FILES to the folder to edit",
                    ),
                ),
            ]),
        });
        Ok(())
    }

    /// Tells each mod about the files it reads that changed on disk.
    fn tick(&mut self, host: &Handle) {
        let Some(root) = &self.root else {
            return;
        };
        for ((owner, path), seen) in &mut self.open {
            let now = root
                .join(path)
                .metadata()
                .ok()
                .filter(fs::Metadata::is_file)
                .map(|meta| version(&meta));
            let current = now.clone().unwrap_or_default();
            if current == *seen {
                continue;
            }
            *seen = current;
            let mut msg = vec![("changed", Data::String(path.clone()))];
            match now {
                Some(version) => msg.push(("version", Data::String(version))),
                None => msg.push(("missing", Data::Bool(true))),
            }
            host.send(owner, map(msg));
        }
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        let Data::Map(fields) = &message else {
            return;
        };
        let text = |key: &str| match fields.get(key) {
            Some(Data::String(text)) => Some(text.as_str()),
            _ => None,
        };
        let id = fields.get("id");
        if let Some(path) = text("read") {
            let answer = self.read(from, path);
            reply(host, from, id, "read", path, answer);
        } else if let Some(path) = text("write") {
            let force = matches!(fields.get("force"), Some(Data::Bool(true)));
            let answer = self.write(from, path, text("text"), text("version"), force);
            reply(host, from, id, "write", path, answer);
        } else if let Some(path) = text("close") {
            self.open.remove(&(from.to_owned(), path.to_owned()));
        }
    }
}

/// What an action answers: its fields when it worked, else why not and
/// whether it was a conflict.
type Answer = Result<Vec<(&'static str, Data)>, Refusal>;

#[derive(Debug)]
struct Refusal {
    error: String,
    conflict: bool,
}

impl From<&str> for Refusal {
    fn from(error: &str) -> Self {
        error.to_owned().into()
    }
}

impl From<String> for Refusal {
    fn from(error: String) -> Self {
        Self {
            error,
            conflict: false,
        }
    }
}

impl Docs {
    fn read(&mut self, from: &str, path: &str) -> Answer {
        let root = self.root.as_deref().ok_or("no project folder")?;
        let file = existing(root, path)?;
        let meta = file.metadata().map_err(|error| error.to_string())?;
        if meta.len() > MAX_DOCUMENT as u64 {
            return Err(format!("larger than {} KiB", MAX_DOCUMENT / 1024).into());
        }
        let bytes = fs::read(&file).map_err(|error| error.to_string())?;
        let text = String::from_utf8(bytes).map_err(|_| "not a text file".to_owned())?;
        let version = version(&meta);
        let key = (from.to_owned(), path.to_owned());
        if self.open.contains_key(&key) || self.open.len() < MAX_OPEN {
            self.open.insert(key, version.clone());
        }
        Ok(vec![
            ("text", Data::String(text)),
            ("version", Data::String(version)),
        ])
    }

    fn write(
        &mut self,
        from: &str,
        path: &str,
        text: Option<&str>,
        expected: Option<&str>,
        force: bool,
    ) -> Answer {
        // Saving changes the user's files; a mod from elsewhere must not do
        // it unasked. A component's instance is named after the mod that
        // drew it (`editor/markdown-editor#main-2`), and counts as that mod.
        let owner = from.split('/').next().unwrap_or(from);
        if !self.writers.iter().any(|writer| writer == owner) {
            return Err(format!(
                "`{owner}` may not save files: only the mods in providers/docs's `writers` may"
            )
            .into());
        }
        let root = self.root.as_deref().ok_or("no project folder")?;
        let text = text.ok_or("nothing to save: `text` is missing")?;
        if text.len() > MAX_DOCUMENT {
            return Err(format!("larger than {} KiB", MAX_DOCUMENT / 1024).into());
        }
        let file = writable(root, path)?;
        if let Ok(meta) = file.metadata()
            && !force
        {
            let current = version(&meta);
            if expected != Some(current.as_str()) {
                return Err(Refusal {
                    error: match expected {
                        Some(_) => "the file changed on disk since it was opened".to_owned(),
                        None => "the file already exists".to_owned(),
                    },
                    conflict: true,
                });
            }
        }
        save(&file, text).map_err(|error| format!("could not save: {error}"))?;
        let meta = file.metadata().map_err(|error| error.to_string())?;
        let version = version(&meta);
        // Its own save is not news to the mod that made it.
        let key = (from.to_owned(), path.to_owned());
        if self.open.contains_key(&key) || self.open.len() < MAX_OPEN {
            self.open.insert(key, version.clone());
        }
        Ok(vec![
            ("version", Data::String(version)),
            ("message", Data::String(format!("Saved {path}"))),
        ])
    }
}

fn reply(host: &Handle, to: &str, id: Option<&Data>, action: &str, path: &str, answer: Answer) {
    let mut fields = vec![
        ("reply", id.cloned().unwrap_or_else(|| Data::from(action))),
        ("action", Data::from(action)),
        ("path", Data::from(path)),
        ("ok", Data::Bool(answer.is_ok())),
    ];
    match answer {
        Ok(more) => fields.extend(more),
        Err(refusal) => {
            fields.push(("error", Data::String(refusal.error)));
            if refusal.conflict {
                fields.push(("conflict", Data::Bool(true)));
            }
        },
    }
    host.send(to, map(fields));
}

fn map(fields: impl IntoIterator<Item = (&'static str, Data)>) -> Data {
    Data::Map(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

/// What tells two states of a file apart: when it was last written, to the
/// nanosecond, and its length. It stays the same across launches.
fn version(meta: &fs::Metadata) -> String {
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos());
    format!("{modified}:{}", meta.len())
}

/// `path` under `root`, as written: plain names only.
fn joined(root: &Path, path: &str) -> Result<PathBuf, String> {
    let plain = !path.is_empty()
        && path.split('/').all(|part| !part.is_empty())
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(name) if name != ".git"));
    if !plain {
        return Err(format!(
            "`{path}` is not a path in the project: write it relative to the root, without \
             `.`, `..` or `.git`"
        ));
    }
    Ok(root.join(path))
}

/// The file `path` names, which must exist and stay in the root after
/// following links.
fn existing(root: &Path, path: &str) -> Result<PathBuf, String> {
    let file = joined(root, path)?
        .canonicalize()
        .map_err(|_| format!("`{path}` does not exist"))?;
    if !file.starts_with(root) {
        return Err(format!("`{path}` leads outside the project"));
    }
    if !file.is_file() {
        return Err(format!("`{path}` is not a file"));
    }
    Ok(file)
}

/// Where saving `path` writes: the file itself when it exists, else a new
/// one in a folder of the root that does.
fn writable(root: &Path, path: &str) -> Result<PathBuf, String> {
    let file = joined(root, path)?;
    if file.symlink_metadata().is_ok() {
        return existing(root, path);
    }
    let (Some(parent), Some(name)) = (file.parent(), file.file_name()) else {
        return Err(format!("`{path}` is not a file"));
    };
    let parent = parent
        .canonicalize()
        .map_err(|_| format!("the folder of `{path}` does not exist"))?;
    if !parent.starts_with(root) {
        return Err(format!("`{path}` leads outside the project"));
    }
    Ok(parent.join(name))
}

/// Writes `text` to a file beside `file`, then renames it over, so the file
/// is never seen half written. It keeps the original's permissions.
fn save(file: &Path, text: &str) -> std::io::Result<()> {
    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = file.with_file_name(format!(".{name}.moonframe-{}.tmp", std::process::id()));
    let written = (|| {
        let mut out = fs::File::create(&temp)?;
        out.write_all(text.as_bytes())?;
        out.sync_all()?;
        if let Ok(meta) = file.metadata() {
            fs::set_permissions(&temp, meta.permissions())?;
        }
        fs::rename(&temp, file)
    })();
    if written.is_err() {
        fs::remove_file(&temp).ok();
    }
    written
}

/// The project: `MOONFRAME_FILES`, else the git repository holding the app's
/// folder (two above the provider's), found by its `.git`.
fn root(dir: &Path) -> Option<PathBuf> {
    if let Some(folder) = std::env::var_os("MOONFRAME_FILES") {
        return PathBuf::from(folder).canonicalize().ok();
    }
    let start = std::env::var_os("LAYOUTS_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.join("..").join(".."));
    let start = start.canonicalize().ok()?;
    start
        .ancestors()
        .find(|folder| folder.join(".git").exists())
        .map(Path::to_owned)
}

fn writers(settings: &Data) -> Vec<String> {
    let Data::Map(settings) = settings else {
        return Vec::new();
    };
    match settings.get("writers") {
        Some(Data::List(names)) => names
            .iter()
            .filter_map(|name| match name {
                Data::String(name) => Some(name.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
