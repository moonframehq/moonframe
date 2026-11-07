//! The folder of documents: one markdown file each, a `.trash` folder, a
//! `.favorites` file, and a journal per run. Nothing is written over a file
//! in place, and a move never replaces another document.
//!
//! Each run journals its edit in a file of its own and holds a lock beside
//! it for as long as it runs: a journal whose lock can be taken belongs to a
//! run that stopped, and is the only kind another run recovers.

use {
    crate::text,
    chrono::Local,
    serde_json::{Value, json},
    sha2::{Digest, Sha256},
    std::{
        collections::BTreeSet,
        fs::{self, File, OpenOptions},
        hash::{BuildHasher, Hasher, RandomState},
        io::{self, Write},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, SystemTime},
    },
};

/// Characters in a document's name, as [`valid`] allows.
const NAME_MAX: usize = 81;

/// Time between the samples' dates, the first newest.
const SAMPLE_AGE: Duration = Duration::from_secs(7 * 3600);

/// Bytes of the open document sent; larger ones open without their text.
pub(crate) const MAX_TEXT: u64 = 256 * 1024;

/// Whether `doc` can name a document: it comes from a mod, and joins a path.
pub(crate) fn valid(doc: &str) -> bool {
    let mut chars = doc.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && doc.len() <= NAME_MAX
        && chars.all(|c| c.is_ascii_alphanumeric() || " _.,'()-".contains(c))
}

/// `name` cut to leave room for `suffix`, with it.
fn suffixed(name: &str, suffix: &str) -> String {
    let room = NAME_MAX - suffix.chars().count();
    let cut: String = name.chars().take(room).collect();
    format!("{}{suffix}", cut.trim_end())
}

fn digest_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn digest_text(text: &str) -> String {
    digest_bytes(text.as_bytes())
}

/// The hash of a file's bytes, or `None` when it can't be read.
pub(crate) fn digest(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|bytes| digest_bytes(&bytes))
}

/// A file as text: bad bytes replaced, line ends as `\n`, empty if unreadable.
pub(crate) fn read(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    String::from_utf8_lossy(&bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

/// Removes `path`, which may have gone already.
fn gone(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// A name no other run or call of this one has.
pub(crate) fn unique() -> String {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    hasher.write_u64(COUNT.fetch_add(1, Ordering::Relaxed));
    let high = hasher.finish();
    hasher.write_u64(high);
    format!("{high:016x}{:016x}", hasher.finish())
}

/// A complete, synced temporary file in `folder`, removed unless taken.
struct Temporary(PathBuf);

impl Temporary {
    fn write(folder: &Path, body: &str) -> io::Result<Self> {
        let temporary = Self(folder.join(format!(".tmp-{}", unique())));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary.0)?;
        file.write_all(body.as_bytes())?;
        file.sync_all()?;
        Ok(temporary)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Replacing with a complete, synced file leaves the previous text or the
/// new one if the app is killed, never a truncated document.
pub(crate) fn atomic_write(path: &Path, body: &str) -> io::Result<()> {
    let folder = path.parent().unwrap_or(Path::new("."));
    let temporary = Temporary::write(folder, body)?;
    fs::rename(&temporary.0, path)
}

/// Writes `body` as a new document named `name` in `folder`, or "name (2)",
/// "name (3)"…: the complete file is linked into place, which fails rather
/// than replaces when another run took the name first.
fn write_new(folder: &Path, name: &str, body: &str) -> io::Result<String> {
    let temporary = Temporary::write(folder, body)?;
    let mut candidate = name.to_owned();
    for n in 2.. {
        match fs::hard_link(&temporary.0, folder.join(format!("{candidate}.md"))) {
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                candidate = suffixed(name, &format!(" ({n})"));
            },
            linked => return linked.map(|()| candidate),
        }
    }
    unreachable!("a name is always free in the end")
}

/// The folder's paths, and this run's journal in it.
pub(crate) struct Folder {
    pub(crate) root: PathBuf,
    trash: PathBuf,
    favorites: PathBuf,
    /// One shared journal, from before runs had their own.
    legacy: PathBuf,
    journals: PathBuf,
    pub(crate) journal: PathBuf,
    lock: PathBuf,
}

impl Folder {
    pub(crate) fn new(root: PathBuf, session: &str) -> Self {
        let journals = root.join(".journals");
        Self {
            trash: root.join(".trash"),
            favorites: root.join(".favorites"),
            legacy: root.join(".pending"),
            journal: journals.join(format!("{session}.json")),
            lock: journals.join(format!("{session}.lock")),
            journals,
            root,
        }
    }

    /// Makes the folder; the first time, with `samples` in it, newest first.
    pub(crate) fn ensure(&self, samples: &[(String, String)]) -> io::Result<()> {
        let first = !self.root.is_dir();
        fs::create_dir_all(&self.trash)?;
        if first {
            let now = SystemTime::now();
            let mut age = Duration::ZERO;
            for (name, body) in samples {
                let path = self.path_of(name, false);
                fs::write(&path, body)?;
                File::options()
                    .write(true)
                    .open(&path)?
                    .set_modified(now - age)?;
                age += SAMPLE_AGE;
            }
        }
        Ok(())
    }

    pub(crate) fn path_of(&self, doc: &str, trashed: bool) -> PathBuf {
        let folder = if trashed {
            &self.trash
        } else {
            &self.root
        };
        folder.join(format!("{doc}.md"))
    }

    /// The document's file and whether it is in the trash.
    pub(crate) fn find(&self, doc: &str) -> Option<(PathBuf, bool)> {
        if !valid(doc) {
            return None;
        }
        [false, true]
            .into_iter()
            .map(|trashed| (self.path_of(doc, trashed), trashed))
            .find(|(path, _)| path.is_file())
    }

    pub(crate) fn favorites(&self) -> BTreeSet<String> {
        fs::read_to_string(&self.favorites)
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn save_favorites(&self, names: &BTreeSet<String>) -> io::Result<()> {
        let lines: String = names.iter().map(|name| format!("{name}\n")).collect();
        fs::write(&self.favorites, lines)
    }

    /// `doc` itself if no file has that name in the folder it moves to, else
    /// "doc (2)", "doc (3)"…, so a move never replaces another file.
    pub(crate) fn free(&self, doc: &str, trashed: bool) -> String {
        let mut name = doc.to_owned();
        let mut n = 2;
        while self.path_of(&name, trashed).exists() {
            name = suffixed(doc, &format!(" ({n})"));
            n += 1;
        }
        name
    }

    /// A name for a new document, today's, free in both folders.
    pub(crate) fn fresh_name(&self) -> String {
        let base = Local::now().format("Untitled %Y-%m-%d").to_string();
        let mut name = base.clone();
        let mut n = 2;
        while self.path_of(&name, false).exists() || self.path_of(&name, true).exists() {
            name = format!("{base} ({n})");
            n += 1;
        }
        name
    }

    /// Every document, newest first; `pending` is text not written yet.
    pub(crate) fn listing(&self, pending: Option<&(PathBuf, String)>) -> Vec<Value> {
        let stars = self.favorites();
        let mut found = Vec::new();
        for (trashed, folder) in [(false, &self.root), (true, &self.trash)] {
            let entries = fs::read_dir(folder).into_iter().flatten().flatten();
            for entry in entries {
                let file = entry.file_name();
                let Some(doc) = file.to_str().and_then(|name| name.strip_suffix(".md")) else {
                    continue;
                };
                let path = entry.path();
                let Ok(stamp) = fs::metadata(&path).and_then(|file| file.modified()) else {
                    continue;
                };
                if !valid(doc) {
                    continue;
                }
                let body = match pending {
                    Some((file, body)) if *file == path => body.clone(),
                    _ => read(&path),
                };
                let item = json!({
                    "id": doc,
                    "title": text::title(&body, doc),
                    "excerpt": text::excerpt(&body),
                    "words": text::words(&body),
                    "modified": text::when(stamp),
                    "favorite": stars.contains(doc),
                    "trashed": trashed,
                });
                found.push((stamp, doc.to_owned(), item));
            }
        }
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        found.into_iter().map(|(_, _, item)| item).collect()
    }

    /// Whether a journal's path is a document of this library.
    fn inside(&self, path: &Path) -> bool {
        let parent = path.canonicalize().ok();
        let parent = parent.as_deref().and_then(Path::parent);
        path.extension().is_some_and(|extension| extension == "md")
            && [&self.root, &self.trash]
                .into_iter()
                .any(|folder| parent.is_some() && folder.canonicalize().ok().as_deref() == parent)
    }

    /// Writes a journaled edit to its document if that still exists: a
    /// deleted document is deliberately not recreated.
    fn replay(&self, journal: &Path) -> io::Result<()> {
        let saved: Option<Value> = fs::read(journal)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let Some(saved) = saved else {
            return Ok(());
        };
        let Some(body) = saved.get("text").and_then(Value::as_str) else {
            return Ok(());
        };
        let path = match (saved.get("path").and_then(Value::as_str), saved.get("id")) {
            (Some(path), _) => Some(PathBuf::from(path)),
            // A journal from before paths were kept.
            (None, Some(id)) => id
                .as_str()
                .and_then(|id| self.find(id))
                .map(|(path, _)| path),
            (None, None) => None,
        };
        let Some(path) = path.filter(|path| self.inside(path) && path.is_file()) else {
            return Ok(());
        };
        // The edit was made on the file as `base` hashes it. If the file has
        // changed since (another run saved it, or another editor), the newer
        // text stays and the recovered one goes beside it.
        let now = digest(&path);
        let base = saved.get("base").and_then(Value::as_str);
        if now.as_deref() == base || now.as_deref() == Some(digest_text(body).as_str()) {
            return atomic_write(&path, body);
        }
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        let folder = path.parent().unwrap_or(&self.root);
        write_new(folder, &suffixed(&name, " (recovered)"), body).map(drop)
    }

    /// Takes this run's lock, then replays the journals of runs that stopped
    /// before writing their last edit, leaving those of live runs alone. The
    /// lock is taken under a name no scan looks at and renamed into place,
    /// so every `*.lock` another run sees is already held.
    pub(crate) fn recover(&self) -> io::Result<File> {
        fs::create_dir_all(&self.journals)?;
        let hidden = self.lock.with_extension("locking");
        let lock = File::create(&hidden)?;
        lock.lock()?;
        fs::rename(&hidden, &self.lock)?;
        if self.legacy.is_file() {
            self.replay(&self.legacy)?;
            gone(&self.legacy)?;
        }
        for entry in fs::read_dir(&self.journals)?.flatten() {
            let other = entry.path();
            if other == self.lock || other.extension().is_none_or(|e| e != "lock") {
                continue;
            }
            // Opened as it is: never create one that has gone.
            let Ok(held) = OpenOptions::new().write(true).open(&other) else {
                continue;
            };
            // Taken only when its run has stopped.
            if held.try_lock().is_err() {
                continue;
            }
            let journal = other.with_extension("json");
            if journal.is_file() {
                self.replay(&journal)?;
            }
            gone(&journal)?;
            gone(&other)?;
        }
        Ok(lock)
    }

    /// Lets go of this run's lock file, at a clean stop.
    pub(crate) fn release(&self) {
        let _ = fs::remove_file(&self.lock);
    }

    /// Records an edit, before it is published, for recovery.
    pub(crate) fn journal(&self, path: &Path, body: &str, base: Option<&str>) -> io::Result<()> {
        let entry = json!({ "path": path.to_string_lossy(), "text": body, "base": base });
        atomic_write(&self.journal, &entry.to_string())
    }

    /// Forgets the journaled edit, once its document has been replaced.
    pub(crate) fn clear_journal(&self) -> io::Result<()> {
        gone(&self.journal)
    }
}
