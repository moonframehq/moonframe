//! The derived index: one SQLite file in `.commonplace/` holding what the
//! parser found in every note, a full-text index (FTS5) and each link
//! resolved to the note it points at. It is only ever filled from files and
//! can be deleted at any time: a schema it does not know is dropped and
//! rebuilt, never migrated.

mod query;
mod resolve;

pub use query::{
    Backlink, Hit, Listed, MATCH_END, MATCH_START, Mention, NoteInfo, Order, Outgoing, TagCount,
    TaskRow, Unresolved,
};

use {
    crate::{Error, parse::Note, text},
    resolve::Resolver,
    rusqlite::{Connection, OptionalExtension, Transaction, params},
    std::{collections::HashSet, path::Path},
};

/// Bump when the tables change: an index of another version is rebuilt.
const SCHEMA: i64 = 3;

const TABLES: &str = "
CREATE TABLE notes (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    path_key TEXT NOT NULL,
    title TEXT NOT NULL,
    title_key TEXT NOT NULL,
    note_id TEXT,
    kind TEXT,
    owner TEXT NOT NULL,
    mtime INTEGER NOT NULL,
    -- When the note last changed: its `updated` property, else the file's time.
    sort_time INTEGER NOT NULL,
    size INTEGER NOT NULL,
    hash TEXT NOT NULL,
    words INTEGER NOT NULL,
    excerpt TEXT NOT NULL,
    props TEXT NOT NULL
);
CREATE INDEX notes_path_key ON notes(path_key);
CREATE INDEX notes_title_key ON notes(title_key);
CREATE INDEX notes_mtime ON notes(mtime);
CREATE INDEX notes_sort_time ON notes(sort_time);
CREATE TABLE aliases (note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE, alias TEXT NOT NULL, key TEXT NOT NULL);
CREATE INDEX aliases_key ON aliases(key);
CREATE INDEX aliases_note ON aliases(note);
CREATE TABLE links (
    note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    target TEXT NOT NULL,
    target_key TEXT NOT NULL,
    -- The key's last segment, to find `[[folder/name]]` links by name.
    target_name TEXT NOT NULL,
    anchor TEXT,
    display TEXT,
    kind TEXT NOT NULL,
    line INTEGER NOT NULL,
    context TEXT NOT NULL,
    dst INTEGER REFERENCES notes(id) ON DELETE SET NULL
);
CREATE INDEX links_note ON links(note);
CREATE INDEX links_dst ON links(dst);
CREATE INDEX links_target_key ON links(target_key);
CREATE INDEX links_target_name ON links(target_name);
CREATE TABLE tags (note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE, tag TEXT NOT NULL, key TEXT NOT NULL);
CREATE INDEX tags_key ON tags(key);
CREATE INDEX tags_note ON tags(note);
CREATE TABLE props (note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE, key TEXT NOT NULL, value TEXT NOT NULL);
CREATE INDEX props_key ON props(key);
CREATE INDEX props_note ON props(note);
CREATE TABLE headings (note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE, level INTEGER NOT NULL, text TEXT NOT NULL, line INTEGER NOT NULL);
CREATE INDEX headings_note ON headings(note);
CREATE TABLE blocks (note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE, anchor TEXT NOT NULL, text TEXT NOT NULL, line INTEGER NOT NULL);
CREATE INDEX blocks_note ON blocks(note);
CREATE TABLE tasks (note INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE, done INTEGER NOT NULL, text TEXT NOT NULL, line INTEGER NOT NULL);
CREATE INDEX tasks_note ON tasks(note);
CREATE INDEX tasks_done ON tasks(done);
CREATE VIRTUAL TABLE fts USING fts5(title, aliases, body, tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3');
";

/// Past this many notes and names in one batch, links are resolved all
/// at once rather than by name.
const LARGE_BATCH: usize = 256;

/// What the index knows of a file on disk, to tell whether to read it again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub mtime: i64,
    pub size: i64,
    pub hash: String,
}

pub struct Index {
    db: Connection,
}

impl Index {
    /// Opens the index at `file`, creating it, or rebuilding it empty when it
    /// was made by another schema or cannot be read.
    pub(crate) fn open(file: &Path) -> Result<Self, Error> {
        match Self::try_open(file) {
            Ok(index) => Ok(index),
            Err(_) => {
                // A derived file: start it over rather than fail the vault.
                for suffix in ["", "-wal", "-shm"] {
                    let mut name = file.as_os_str().to_owned();
                    name.push(suffix);
                    let _ = std::fs::remove_file(name);
                }
                Self::try_open(file)
            },
        }
    }

    /// An index in memory, for tests and one-off commands.
    pub(crate) fn in_memory() -> Result<Self, Error> {
        let db = Connection::open_in_memory()?;
        Self::prepare(db)
    }

    fn try_open(file: &Path) -> Result<Self, Error> {
        let db = Connection::open(file)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        Self::prepare(db)
    }

    fn prepare(db: Connection) -> Result<Self, Error> {
        db.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version != SCHEMA {
            let tables: Vec<String> = db
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'fts_%'")?
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            for table in tables {
                db.execute_batch(&format!("DROP TABLE IF EXISTS \"{table}\";"))?;
            }
            db.execute_batch(TABLES)?;
            db.pragma_update(None, "user_version", SCHEMA)?;
        }
        Ok(Self { db })
    }

    /// Every indexed path with its stamp.
    pub(crate) fn stamps(&self) -> Result<Vec<(String, Stamp)>, Error> {
        let mut statement = self
            .db
            .prepare("SELECT path, mtime, size, hash FROM notes")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, Stamp {
                mtime: row.get(1)?,
                size: row.get(2)?,
                hash: row.get(3)?,
            }))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Applies a batch: notes read again, files gone, and stamps of files
    /// whose time changed but not their text. Links are resolved again where
    /// the batch could change what they point at.
    pub(crate) fn apply(
        &mut self,
        changed: &[(Note, Stamp)],
        removed: &[String],
        touched: &[(String, Stamp)],
    ) -> Result<(), Error> {
        if changed.is_empty() && removed.is_empty() && touched.is_empty() {
            return Ok(());
        }
        let tx = self.db.transaction()?;
        // Names whose resolution may change: those of notes leaving and coming.
        let mut keys: HashSet<String> = HashSet::new();
        for path in removed {
            names_of(&tx, path, &mut keys)?;
            delete(&tx, path)?;
        }
        let mut fresh = Vec::with_capacity(changed.len());
        for (note, stamp) in changed {
            names_of(&tx, &note.path, &mut keys)?;
            delete(&tx, &note.path)?;
            keys.insert(text::key(&note.path));
            keys.insert(text::key(&note.title));
            keys.extend(note.aliases.iter().map(|alias| text::key(alias)));
            fresh.push(insert(&tx, note, stamp)?);
        }
        for (path, stamp) in touched {
            tx.execute(
                "UPDATE notes SET mtime = ?2, size = ?3 WHERE path = ?1",
                params![path, stamp.mtime, stamp.size],
            )?;
        }
        let resolver = Resolver::load(&tx)?;
        if fresh.len() + keys.len() > LARGE_BATCH {
            // One pass over every link beats a query per name.
            resolver.resolve_all(&tx)?;
        } else {
            resolver.resolve_notes(&tx, &fresh)?;
            resolver.resolve_keys(&tx, &keys)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The note at `path`, by its row.
    pub(crate) fn row_of(&self, path: &str) -> Result<Option<i64>, Error> {
        Ok(self
            .db
            .query_row("SELECT id FROM notes WHERE path = ?1", [path], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub(crate) fn db(&self) -> &Connection {
        &self.db
    }
}

/// The keys `path`'s indexed note answers to now (before it changes).
fn names_of(tx: &Transaction, path: &str, keys: &mut HashSet<String>) -> Result<(), Error> {
    let found: Option<(i64, String, String)> = tx
        .query_row(
            "SELECT id, path_key, title_key FROM notes WHERE path = ?1",
            [path],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((id, path_key, title_key)) = found else {
        return Ok(());
    };
    keys.insert(path_key);
    keys.insert(title_key);
    let mut statement = tx.prepare_cached("SELECT key FROM aliases WHERE note = ?1")?;
    for alias in statement.query_map([id], |row| row.get::<_, String>(0))? {
        keys.insert(alias?);
    }
    Ok(())
}

fn delete(tx: &Transaction, path: &str) -> Result<(), Error> {
    let id: Option<i64> = tx
        .query_row("SELECT id FROM notes WHERE path = ?1", [path], |row| {
            row.get(0)
        })
        .optional()?;
    if let Some(id) = id {
        tx.execute("DELETE FROM fts WHERE rowid = ?1", [id])?;
        tx.execute("DELETE FROM notes WHERE id = ?1", [id])?;
    }
    Ok(())
}

/// When `note` last changed, in milliseconds: its `updated` (or `modified`)
/// property when it has one (an import keeps the source's dates there),
/// else `mtime`, the file's.
fn sort_time(note: &Note, mtime: i64) -> i64 {
    ["updated", "modified"]
        .iter()
        .filter_map(|key| note.props.get(*key).and_then(serde_json::Value::as_str))
        .find_map(text::parse_time)
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(mtime)
}

fn insert(tx: &Transaction, note: &Note, stamp: &Stamp) -> Result<i64, Error> {
    let props = serde_json::to_string(&note.props).unwrap_or_else(|_| "{}".to_string());
    tx.prepare_cached(
        "INSERT INTO notes (path, path_key, title, title_key, note_id, kind, owner, mtime, sort_time, size, hash, words, excerpt, props)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    )?
    .execute(params![
        note.path,
        text::key(&note.path),
        note.title,
        text::key(&note.title),
        note.id,
        note.kind,
        note.owner.as_str(),
        stamp.mtime,
        sort_time(note, stamp.mtime),
        stamp.size,
        stamp.hash,
        note.words,
        note.excerpt,
        props,
    ])?;
    let id = tx.last_insert_rowid();
    {
        let mut statement =
            tx.prepare_cached("INSERT INTO aliases (note, alias, key) VALUES (?1, ?2, ?3)")?;
        for alias in &note.aliases {
            statement.execute(params![id, alias, text::key(alias)])?;
        }
    }
    {
        let mut statement = tx.prepare_cached(
            "INSERT INTO links (note, target, target_key, target_name, anchor, display, kind, line, context) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for link in &note.links {
            let key = text::key(&link.target);
            let name = key.rsplit('/').next().unwrap_or(&key).to_string();
            statement.execute(params![
                id,
                link.target,
                key,
                name,
                link.anchor,
                link.display,
                link.kind.as_str(),
                link.line,
                link.context,
            ])?;
        }
    }
    {
        let mut statement =
            tx.prepare_cached("INSERT INTO tags (note, tag, key) VALUES (?1, ?2, ?3)")?;
        for tag in &note.tags {
            statement.execute(params![id, tag, tag.to_lowercase()])?;
        }
    }
    {
        let mut statement =
            tx.prepare_cached("INSERT INTO props (note, key, value) VALUES (?1, ?2, ?3)")?;
        for (key, value) in &note.props {
            statement.execute(params![id, key.to_lowercase(), value.to_string()])?;
        }
    }
    {
        let mut statement = tx.prepare_cached(
            "INSERT INTO headings (note, level, text, line) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for heading in &note.headings {
            statement.execute(params![id, heading.level, heading.text, heading.line])?;
        }
    }
    {
        let mut statement = tx.prepare_cached(
            "INSERT INTO blocks (note, anchor, text, line) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for block in &note.blocks {
            statement.execute(params![id, block.anchor, block.text, block.line])?;
        }
    }
    {
        let mut statement = tx
            .prepare_cached("INSERT INTO tasks (note, done, text, line) VALUES (?1, ?2, ?3, ?4)")?;
        for task in &note.tasks {
            statement.execute(params![id, task.done, task.text, task.line])?;
        }
    }
    tx.prepare_cached("INSERT INTO fts (rowid, title, aliases, body) VALUES (?1, ?2, ?3, ?4)")?
        .execute(params![id, note.title, note.aliases.join(" "), note.body])?;
    Ok(id)
}
