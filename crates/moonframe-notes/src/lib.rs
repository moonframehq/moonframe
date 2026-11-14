//! Commonplace's notes engine: a vault of markdown files (Obsidian's
//! format, see `examples/commonplace/VAULT.md`) and a derived SQLite index
//! of its notes, links, tags, tasks and text.
//!
//! The files are the truth. The index (`.commonplace/index.sqlite`) is only
//! ever filled from them and can be deleted at any time. Every change goes
//! through a file write, journaled and atomic, then the file is indexed
//! again.
//!
//! ```no_run
//! # fn main() -> Result<(), moonframe_notes::Error> {
//! let mut vault = moonframe_notes::Vault::open("/path/to/vault")?;
//! vault.scan()?;
//! for hit in vault.index().search("spaced repetition", 0, 20)? {
//!     println!("{} {}", hit.path, hit.snippet);
//! }
//! # Ok(())
//! # }
//! ```

// keep-sorted start
mod import;
mod index;
mod parse;
#[cfg(feature = "provider")]
mod provider;
mod rename;
mod scan;
mod text;
mod write;
// keep-sorted end

#[cfg(feature = "provider")]
pub use provider::native;

pub use {
    import::{ImportReport, Progress, SourceNote},
    index::{
        Backlink, Hit, Index, Listed, MATCH_END, MATCH_START, Mention, NoteInfo, Order, Outgoing,
        TagCount, TaskRow, Unresolved,
    },
    parse::{Block, Heading, Link, LinkKind, Note, Owner, Task, parse},
    rename::Renamed,
    scan::{MAX_FILE, Report, is_note},
    text::{digest, key, stem, ulid},
    write::{Saved, merge},
};

use {
    std::{
        fs,
        path::{Path, PathBuf},
    },
    write::Journal,
};

/// The folder in a vault holding the app's journal and trash.
pub const STATE: &str = ".commonplace";

/// Where a vault's index lives: the cache folder (`$XDG_CACHE_HOME`, else
/// `~/.cache`), under `commonplace/<a hash of the vault's path>/`. It is
/// derived data, so it stays out of the vault and whatever syncs it.
pub fn default_index(root: &Path) -> PathBuf {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache")
        });
    let id = digest(&root.display().to_string());
    cache
        .join("commonplace")
        .join(&id[..16])
        .join("index.sqlite")
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("index: {0}")]
    Index(#[from] rusqlite::Error),
    #[error("no note at {0}")]
    NotFound(String),
    #[error("{0} already exists")]
    Exists(String),
    #[error("{0}")]
    Invalid(String),
}

/// A vault: its folder and the index of it.
pub struct Vault {
    root: PathBuf,
    index: Index,
    journal: Journal,
}

impl Vault {
    /// Opens the vault at `root`: creates `.commonplace/` (journal, trash)
    /// in it and its index in the cache folder ([`default_index`]), and
    /// finishes any write a previous run left in the journal. Call
    /// [`Vault::scan`] to bring the index up to date.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, Error> {
        let root = root.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(Error::Invalid(format!(
                "no vault folder at {}",
                root.display()
            )));
        }
        let root = root.canonicalize().unwrap_or(root);
        let index_file = default_index(&root);
        Self::open_with_index(root, &index_file)
    }

    /// Opens the vault at `root` with its index at `index_file`.
    pub fn open_with_index(root: impl AsRef<Path>, index_file: &Path) -> Result<Self, Error> {
        let root = root.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(Error::Invalid(format!(
                "no vault folder at {}",
                root.display()
            )));
        }
        // The real path: file watchers report paths through no symlinks.
        let root = root.canonicalize().unwrap_or(root);
        let state = root.join(STATE);
        fs::create_dir_all(&state)?;
        let ignore = state.join(".gitignore");
        if !ignore.exists() {
            fs::write(&ignore, "*\n")?;
        }
        if let Some(folder) = index_file.parent() {
            fs::create_dir_all(folder)?;
            // Say which vault this index is of, for whoever looks in the cache.
            let _ = fs::write(folder.join("vault.txt"), root.display().to_string());
        }
        let index = Index::open(index_file)?;
        let journal = Journal::new(state.join("journal"));
        let vault = Self {
            root,
            index,
            journal,
        };
        vault.recover()?;
        Ok(vault)
    }

    /// A vault whose index lives in memory only: for tests and one-off reads.
    pub fn open_in_memory(root: impl AsRef<Path>) -> Result<Self, Error> {
        let root = root.as_ref().to_path_buf();
        let journal = Journal::new(root.join(STATE).join("journal"));
        Ok(Self {
            root,
            index: Index::in_memory()?,
            journal,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index(&self) -> &Index {
        &self.index
    }

    /// Brings the index in step with every file in the vault.
    pub fn scan(&mut self) -> Result<Report, Error> {
        scan::scan(&self.root, &mut self.index)
    }

    /// Reads one file again, as after a watcher saw it change (or vanish).
    pub fn refresh(&mut self, path: &str) -> Result<(), Error> {
        scan::refresh(&self.root, &mut self.index, path)
    }

    /// The text of the note at `path`.
    pub fn read(&self, path: &str) -> Result<String, Error> {
        let file = self.file(path)?;
        match fs::read_to_string(&file) {
            Ok(text) => Ok(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(Error::NotFound(path.to_string()))
            },
            Err(error) => Err(error.into()),
        }
    }

    /// Writes `text` to the note at `path`. `base` is the text the edit
    /// started from: when the file changed since, the two are merged or,
    /// where they overlap, both kept (see [`Saved`]). `stamp` names a
    /// conflict copy, such as `2026-11-10 1412`.
    pub fn write(
        &mut self,
        path: &str,
        text: &str,
        base: Option<&str>,
        stamp: &str,
    ) -> Result<Saved, Error> {
        self.file(path)?;
        let saved = write::save(&self.root, &self.journal, path, text, base, stamp)?;
        scan::refresh(&self.root, &mut self.index, path)?;
        if let Saved::Conflict { copy, .. } = &saved {
            scan::refresh(&self.root, &mut self.index, copy)?;
        }
        Ok(saved)
    }

    /// Creates a note at `path` holding `text`, with a new `id` in its
    /// frontmatter. Refused when the file exists.
    pub fn create(&mut self, path: &str, text: &str) -> Result<String, Error> {
        let file = self.file(path)?;
        if file.exists() {
            return Err(Error::Exists(path.to_string()));
        }
        let text = parse::with_id(text, &ulid());
        write::write_journaled(&self.root, &self.journal, path, &text)?;
        scan::refresh(&self.root, &mut self.index, path)?;
        Ok(text)
    }

    /// Moves a note out of the vault's notes: to the system's trash is the
    /// app's business; here it goes to `.commonplace/trash/`, keeping its path.
    pub fn trash(&mut self, path: &str) -> Result<(), Error> {
        let file = self.file(path)?;
        if !file.is_file() {
            return Err(Error::NotFound(path.to_string()));
        }
        let place = self.root.join(STATE).join("trash").join(path);
        if let Some(folder) = place.parent() {
            fs::create_dir_all(folder)?;
        }
        let place = if place.exists() {
            place.with_extension(format!("{}.md", &ulid()[16..]))
        } else {
            place
        };
        fs::rename(&file, &place)?;
        scan::refresh(&self.root, &mut self.index, path)
    }

    /// Imports every note in Notes.app into the vault folder `into`
    /// (`notes`), with the bodies and attachments of Notes.app's Markdown
    /// export in `export` when given, then indexes what came in. `progress` hears each phase's progress
    /// once Notes.app has been read. Importing again updates notes changed
    /// in Notes.app and never overwrites one edited in the vault.
    pub fn import_apple_notes(
        &mut self,
        into: &str,
        export: Option<&Path>,
        dry_run: bool,
        progress: impl FnMut(Progress),
    ) -> Result<ImportReport, Error> {
        let report = import::apple_notes(&self.root, into, export, dry_run, progress)?;
        if !dry_run {
            self.scan()?;
        }
        Ok(report)
    }

    /// Imports a folder of Markdown files another app exported (Notes.app's
    /// Markdown export, with its `Attachments` folder) as they are, into
    /// `into/<folder>`, without the dates and folders only the app knows.
    pub fn import_markdown_export(
        &mut self,
        folder: &Path,
        source: &str,
        into: &str,
        dry_run: bool,
    ) -> Result<ImportReport, Error> {
        let (notes, joined) = import::export::join(Vec::new(), folder, source, "")?;
        let mut report = import::write(&self.root, source, into, &notes, dry_run, |_, _| {})?;
        report.joined = Some(joined);
        if !dry_run {
            self.scan()?;
        }
        Ok(report)
    }

    /// Writes notes another app gave (see [`SourceNote`]) into the vault
    /// folder `into`, as [`Vault::import_apple_notes`] does for Notes.app.
    pub fn import_notes(
        &mut self,
        source: &str,
        into: &str,
        notes: &[SourceNote],
        dry_run: bool,
    ) -> Result<ImportReport, Error> {
        let report = import::write(&self.root, source, into, notes, dry_run, |_, _| {})?;
        if !dry_run {
            self.scan()?;
        }
        Ok(report)
    }

    /// `path` checked as a note's place in this vault (no `..`, no hidden
    /// folders, `.md`), as a file path.
    fn file(&self, path: &str) -> Result<PathBuf, Error> {
        let valid =
            is_note(path) && !path.split('/').any(|part| part == "..") && !path.starts_with('/');
        if !valid {
            return Err(Error::Invalid(format!(
                "{path} is not a note's place in the vault"
            )));
        }
        Ok(self.root.join(path))
    }

    /// Finishes writes left in the journal by a run that stopped part way.
    fn recover(&self) -> Result<(), Error> {
        for pending in self.journal.pending() {
            if self.file(&pending.path).is_ok() {
                write::atomic_write(&self.root.join(&pending.path), pending.text.as_bytes())?;
            }
            self.journal.clear(&pending.path);
        }
        Ok(())
    }
}
