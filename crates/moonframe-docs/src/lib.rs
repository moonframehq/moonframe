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
//! file beside the original and renames it over, keeping its permissions;
//! the version is checked again just before, so a save made elsewhere
//! meanwhile is refused as a conflict rather than lost. At most
//! [`MAX_OPEN`] files are watched; past that the one read longest ago
//! stops being.
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
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// Most files watched at once, over every mod. Past it, the one read or
/// saved longest ago stops being watched: a tab shown again reads its file,
/// and is watched again.
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
    /// The files mods read, by mod and path.
    open: BTreeMap<(String, String), Watch>,
    /// Counts reads and saves, to tell which watch is the oldest.
    uses: u64,
}

/// A file a mod reads: the version it last saw, and when it last read or
/// saved it (in [`Docs::uses`]).
struct Watch {
    version: String,
    used: u64,
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
        for ((owner, path), watch) in &mut self.open {
            let now = root
                .join(path)
                .metadata()
                .ok()
                .filter(fs::Metadata::is_file)
                .map(|meta| version(&meta));
            let current = now.clone().unwrap_or_default();
            if current == watch.version {
                continue;
            }
            watch.version = current;
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

const CHANGED: &str = "the file changed on disk since it was opened";
const EXISTS: &str = "the file already exists";

impl Refusal {
    fn conflict(error: &str) -> Self {
        Self {
            error: error.to_owned(),
            conflict: true,
        }
    }
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
    /// Watches `path` for `from`, at `version`; when full, the watch used
    /// longest ago makes room.
    fn watch(&mut self, from: &str, path: &str, version: &str) {
        self.uses += 1;
        let key = (from.to_owned(), path.to_owned());
        if !self.open.contains_key(&key)
            && self.open.len() >= MAX_OPEN
            && let Some(oldest) = self
                .open
                .iter()
                .min_by_key(|(_, watch)| watch.used)
                .map(|(key, _)| key.clone())
        {
            self.open.remove(&oldest);
        }
        self.open.insert(key, Watch {
            version: version.to_owned(),
            used: self.uses,
        });
    }

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
        self.watch(from, path, &version);
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
        let over = match file.metadata() {
            _ if force => Over::Anything,
            Ok(meta) => {
                let current = version(&meta);
                if expected != Some(current.as_str()) {
                    return Err(match expected {
                        Some(_) => Refusal::conflict(CHANGED),
                        None => Refusal::conflict(EXISTS),
                    });
                }
                Over::Version(current)
            },
            // Gone, or never there: written new, unless it appears meanwhile.
            Err(_) => Over::Nothing,
        };
        save(&file, text, &over).map_err(|error| match error {
            Saved::Conflict(error) => Refusal::conflict(error),
            Saved::Failed(error) => format!("could not save: {error}").into(),
        })?;
        let meta = file.metadata().map_err(|error| error.to_string())?;
        let version = version(&meta);
        // Its own save is not news to the mod that made it.
        self.watch(from, path, &version);
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
            .all(|part| matches!(part, Component::Normal(name) if !is_git(name)));
    if !plain {
        return Err(format!(
            "`{path}` is not a path in the project: write it relative to the root, without \
             `.`, `..` or `.git`"
        ));
    }
    Ok(root.join(path))
}

/// `.git` in any case: macOS and Windows folders ignore it.
fn is_git(name: &std::ffi::OsStr) -> bool {
    name.eq_ignore_ascii_case(".git")
}

/// `resolved`, a path with its links followed, if it is still in the root
/// and out of `.git`: a link can lead anywhere, so what it names is checked
/// again.
fn inside(root: &Path, resolved: &Path, path: &str) -> Result<(), String> {
    let Ok(within) = resolved.strip_prefix(root) else {
        return Err(format!("`{path}` leads outside the project"));
    };
    if within
        .components()
        .any(|part| matches!(part, Component::Normal(name) if is_git(name)))
    {
        return Err(format!("`{path}` leads into `.git`"));
    }
    Ok(())
}

/// The file `path` names, which must exist and stay in the root, out of
/// `.git`, after following links.
fn existing(root: &Path, path: &str) -> Result<PathBuf, String> {
    let file = joined(root, path)?
        .canonicalize()
        .map_err(|_| format!("`{path}` does not exist"))?;
    inside(root, &file, path)?;
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
    let file = parent.join(name);
    inside(root, &file, path)?;
    Ok(file)
}

/// What a save may replace: the file as it was when checked, nothing (a
/// new file), or whatever is there (`force`).
enum Over {
    Version(String),
    Nothing,
    Anything,
}

enum Saved {
    /// The file changed after it was checked; nothing was written.
    Conflict(&'static str),
    Failed(std::io::Error),
}

impl From<std::io::Error> for Saved {
    fn from(error: std::io::Error) -> Self {
        Self::Failed(error)
    }
}

/// Puts `temp` in place of `file` if `file` is still at version `expected`,
/// never over a save made elsewhere meanwhile, which is refused instead.
/// Where the system can swap two files in one step (macOS, Linux), it
/// swaps them, then looks at what came out: anything but `expected` is
/// swapped back, whole. Elsewhere, see [`replace_aside`].
fn replace(temp: &Path, file: &Path, expected: &str) -> Result<(), Saved> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use rustix::{
            fs::{CWD, RenameFlags, renameat_with},
            io::Errno,
        };
        let swap = || renameat_with(CWD, temp, CWD, file, RenameFlags::EXCHANGE);
        match swap() {
            Ok(()) => {
                // `temp` now holds what was in place.
                let was = temp.metadata().ok().map(|meta| version(&meta));
                if was.as_deref() == Some(expected) {
                    fs::remove_file(temp)?;
                    return Ok(());
                }
                swap().map_err(std::io::Error::from)?;
                return Err(Saved::Conflict(CHANGED));
            },
            // Gone since the last look.
            Err(Errno::NOENT) => return Err(Saved::Conflict(CHANGED)),
            // A filesystem that can't swap: as elsewhere.
            Err(Errno::INVAL | Errno::NOTSUP | Errno::NOSYS) => {},
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    replace_aside(temp, file, expected)
}

/// [`replace`] without a swap: the file is moved aside in one step, so a
/// save made elsewhere from then on is a new file in its place; what was
/// moved is looked at, and the new text is linked in only where nothing
/// is (a link never replaces a file). Anything but `expected`, or a file
/// that appeared, is left as it is and refused; the original is put back
/// where it can be. The file is briefly missing meanwhile.
fn replace_aside(temp: &Path, file: &Path, expected: &str) -> Result<(), Saved> {
    let aside = temp.with_extension("old");
    match fs::rename(file, &aside) {
        Ok(()) => {},
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(Saved::Conflict(CHANGED));
        },
        Err(error) => return Err(error.into()),
    }
    // Back where it was, unless a save elsewhere already took its place.
    let put_back = || match fs::hard_link(&aside, file) {
        Ok(()) => fs::remove_file(&aside),
        Err(_) => Ok(()),
    };
    let was = aside.metadata().ok().map(|meta| version(&meta));
    if was.as_deref() != Some(expected) {
        put_back()?;
        return Err(Saved::Conflict(CHANGED));
    }
    match fs::hard_link(temp, file) {
        Ok(()) => {
            fs::remove_file(temp)?;
            fs::remove_file(&aside)?;
            Ok(())
        },
        // A save elsewhere landed meanwhile: it stays.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(&aside)?;
            Err(Saved::Conflict(CHANGED))
        },
        // A folder without links: the original back, then a rename.
        Err(_) => match fs::rename(temp, file) {
            Ok(()) => {
                fs::remove_file(&aside)?;
                Ok(())
            },
            Err(error) => {
                put_back()?;
                Err(error.into())
            },
        },
    }
}

/// Writes `text` to a file beside `file`, then puts it in place, so the file
/// is never seen half written. It keeps the original's permissions. What is
/// there is checked against `over` again just before, after the slow part,
/// so a save made elsewhere meanwhile is refused rather than lost; a new
/// file is linked into place, which fails if one appeared.
fn save(file: &Path, text: &str, over: &Over) -> Result<(), Saved> {
    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // A name no one else picks, made here: never a file or link already
    // there.
    static SAVES: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    let temp = file.with_file_name(format!(
        ".{name}.moonframe-{}-{nanos}-{}.tmp",
        std::process::id(),
        SAVES.fetch_add(1, Ordering::Relaxed)
    ));
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let written = (|| {
        out.write_all(text.as_bytes())?;
        out.sync_all()?;
        let now = file.metadata().ok();
        if let Some(meta) = &now {
            fs::set_permissions(&temp, meta.permissions())?;
        }
        match over {
            Over::Version(expected) => {
                if now.as_ref().map(version).as_deref() != Some(expected.as_str()) {
                    return Err(Saved::Conflict(CHANGED));
                }
                replace(&temp, file, expected)?;
            },
            Over::Nothing => match fs::hard_link(&temp, file) {
                Ok(()) => fs::remove_file(&temp)?,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(Saved::Conflict(EXISTS));
                },
                // A folder without links (some USB drives): only a last look.
                Err(_) if file.symlink_metadata().is_err() => fs::rename(&temp, file)?,
                Err(_) => return Err(Saved::Conflict(EXISTS)),
            },
            Over::Anything => fs::rename(&temp, file)?,
        }
        Ok(())
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
    // As the `files` provider finds it, so a path the explorer gives names
    // the same file here.
    let start = dir.join("..").join("..").canonicalize().ok()?;
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
