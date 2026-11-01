//! What the app remembers between runs, in SQLite: each mod's and each
//! instance's state, and the topics mods publish. The schema grows by
//! numbered migrations ([`MIGRATIONS`]), recorded in SQLite's
//! `user_version`, so an update upgrades the file it finds. A file from a
//! newer app is opened read-only rather than changed under it.
//!
//! Reads happen where they are asked for. Writes go to a thread of their
//! own, which keeps only the latest value of each key and writes them in
//! one transaction, so a burst of changes costs one write.

mod json;

use {
    moonframe_core::Data,
    rusqlite::{Connection, OpenFlags, OptionalExtension as _, params},
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
        sync::{
            Mutex,
            mpsc::{self, Receiver, Sender},
        },
        thread,
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// The schema, one step per migration, in order. Only ever append: a
/// shipped step must never change, since files already ran it.
pub const MIGRATIONS: &[&str] = &[
    // 1: the state of each mod and instance, by key, as JSON.
    "CREATE TABLE state (
        key        TEXT PRIMARY KEY,
        value      TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );",
    // 2: the last value published on each topic, as JSON.
    "CREATE TABLE topics (
        name       TEXT PRIMARY KEY,
        value      TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );",
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot open the state database {path}: {source}")]
    Open {
        path: PathBuf,
        source: rusqlite::Error,
    },
    #[error("cannot create {path}: {source}")]
    Folder {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("migration {step} failed: {source}")]
    Migration {
        step: usize,
        source: rusqlite::Error,
    },
    #[error("cannot start the writer: {0}")]
    Thread(std::io::Error),
}

/// Which table a value lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Table {
    State,
    Topics,
}

impl Table {
    fn name(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Topics => "topics",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::State => "key",
            Self::Topics => "name",
        }
    }
}

enum Write {
    Set(Table, String, String),
    Delete(Table, String),
    /// Answers once everything sent before it is written.
    Flush(Sender<()>),
}

pub struct Store {
    reader: Mutex<Connection>,
    writer: Option<Sender<Write>>,
}

impl Store {
    /// Opens (or creates) the database at `path` and brings its schema up to
    /// date.
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|source| Error::Folder {
                path: folder.to_owned(),
                source,
            })?;
        }
        let open = |flags| {
            Connection::open_with_flags(path, flags).map_err(|source| Error::Open {
                path: path.to_owned(),
                source,
            })
        };
        let mut connection = open(OpenFlags::default())?;
        let newer = prepare(&mut connection)?;
        if newer {
            tracing::warn!(
                "{} was written by a newer app; reading it without saving",
                path.display()
            );
            return Ok(Self {
                reader: Mutex::new(connection),
                writer: None,
            });
        }
        let writer = open(OpenFlags::default())?;
        Self::start(connection, writer)
    }

    /// A database in memory, gone when dropped: for tests.
    pub fn in_memory() -> Result<Self, Error> {
        // Two connections must share one in-memory database.
        let name = format!(
            "file:moonframe-{}?mode=memory&cache=shared",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let open = || {
            Connection::open_with_flags(&name, OpenFlags::default() | OpenFlags::SQLITE_OPEN_URI)
                .map_err(|source| Error::Open {
                    path: PathBuf::from(&name),
                    source,
                })
        };
        let mut reader = open()?;
        prepare(&mut reader)?;
        Self::start(reader, open()?)
    }

    fn start(reader: Connection, writer: Connection) -> Result<Self, Error> {
        let (sender, writes) = mpsc::channel();
        thread::Builder::new()
            .name("state-writer".into())
            .spawn(move || write(writer, &writes))
            .map_err(Error::Thread)?;
        Ok(Self {
            reader: Mutex::new(reader),
            writer: Some(sender),
        })
    }

    /// Whether it saves: false for a file a newer app wrote.
    pub fn writable(&self) -> bool {
        self.writer.is_some()
    }

    /// The saved state at `key`, such as `mod:explorer`.
    pub fn state(&self, key: &str) -> Option<Data> {
        self.read(Table::State, key)
    }

    /// Saves `value` at `key`, after anything saved before it.
    pub fn save_state(&self, key: &str, value: &Data) {
        self.send(Write::Set(
            Table::State,
            key.to_owned(),
            json::to_text(value),
        ));
    }

    pub fn forget_state(&self, key: &str) {
        self.send(Write::Delete(Table::State, key.to_owned()));
    }

    /// Every topic's last value.
    pub fn topics(&self) -> BTreeMap<String, Data> {
        let Ok(connection) = self.reader.lock() else {
            return BTreeMap::new();
        };
        let rows = connection
            .prepare("SELECT name, value FROM topics")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()
            });
        match rows {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|(name, text)| Some((name, json::from_text(&text)?)))
                .collect(),
            Err(error) => {
                tracing::warn!("cannot read topics: {error}");
                BTreeMap::new()
            },
        }
    }

    pub fn save_topic(&self, name: &str, value: &Data) {
        self.send(Write::Set(
            Table::Topics,
            name.to_owned(),
            json::to_text(value),
        ));
    }

    /// Waits until everything saved so far is written.
    pub fn flush(&self) {
        let Some(writer) = &self.writer else {
            return;
        };
        let (done, wait) = mpsc::channel();
        if writer.send(Write::Flush(done)).is_ok() {
            let _ = wait.recv();
        }
    }

    fn read(&self, table: Table, key: &str) -> Option<Data> {
        let connection = self.reader.lock().ok()?;
        let sql = format!(
            "SELECT value FROM {} WHERE {} = ?1",
            table.name(),
            table.key()
        );
        let text = connection
            .query_row(&sql, [key], |row| row.get::<_, String>(0))
            .optional()
            .inspect_err(|error| tracing::warn!("cannot read {key}: {error}"))
            .ok()??;
        json::from_text(&text)
    }

    fn send(&self, write: Write) {
        if let Some(writer) = &self.writer {
            let _ = writer.send(write);
        }
    }
}

static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Sets the connection up and runs the migrations it lacks. Returns whether
/// the file is from a newer app, which it leaves as it is.
fn prepare(connection: &mut Connection) -> Result<bool, Error> {
    let migration = |step: usize| move |source| Error::Migration { step, source };
    // Writes from the writer and reads here can overlap.
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(migration(0))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(migration(0))?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(migration(0))?;
    let version = usize::try_from(version).unwrap_or(0);
    if version > MIGRATIONS.len() {
        return Ok(true);
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(version) {
        let step = index + 1;
        let transaction = connection.transaction().map_err(migration(step))?;
        transaction.execute_batch(sql).map_err(migration(step))?;
        transaction
            .pragma_update(
                None,
                "user_version",
                i64::try_from(step).unwrap_or(i64::MAX),
            )
            .map_err(migration(step))?;
        transaction.commit().map_err(migration(step))?;
    }
    Ok(false)
}

/// The writer thread: takes every write waiting, keeps the last of each key,
/// and writes them in one transaction.
fn write(mut connection: Connection, writes: &Receiver<Write>) {
    while let Ok(first) = writes.recv() {
        let mut latest: BTreeMap<(Table, String), Option<String>> = BTreeMap::new();
        let mut done = Vec::new();
        for write in std::iter::once(first).chain(writes.try_iter()) {
            match write {
                Write::Set(table, key, value) => {
                    latest.insert((table, key), Some(value));
                },
                Write::Delete(table, key) => {
                    latest.insert((table, key), None);
                },
                Write::Flush(answer) => done.push(answer),
            }
        }
        if let Err(error) = commit(&mut connection, &latest) {
            tracing::warn!("cannot save state: {error}");
        }
        for answer in done {
            let _ = answer.send(());
        }
    }
}

fn commit(
    connection: &mut Connection,
    latest: &BTreeMap<(Table, String), Option<String>>,
) -> rusqlite::Result<()> {
    if latest.is_empty() {
        return Ok(());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        });
    let transaction = connection.transaction()?;
    for ((table, key), value) in latest {
        let (name, column) = (table.name(), table.key());
        match value {
            Some(value) => transaction.execute(
                &format!(
                    "INSERT INTO {name} ({column}, value, updated_at) VALUES (?1, ?2, ?3)
                     ON CONFLICT({column}) DO UPDATE SET value = ?2, updated_at = ?3"
                ),
                params![key, value, now],
            )?,
            None => {
                transaction.execute(&format!("DELETE FROM {name} WHERE {column} = ?1"), [key])?
            },
        };
    }
    transaction.commit()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

impl moonframe_core::Persist for Store {
    fn state(&self, key: &str) -> Option<Data> {
        Store::state(self, key)
    }

    fn save_state(&self, key: &str, value: &Data) {
        Store::save_state(self, key, value);
    }

    fn topics(&self) -> BTreeMap<String, Data> {
        Store::topics(self)
    }

    fn save_topic(&self, name: &str, value: &Data) {
        Store::save_topic(self, name, value);
    }
}
