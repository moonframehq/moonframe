//! Where each mod's state and the topics outlive the app
//! ([`moonframe_core::Persist`]): herdr-core's document store when herdr-core
//! runs here, beside the terminals it keeps, else SQLite
//! (`moonframe_store`).
//!
//! In herdr-core it is one JSON document, the app's own format:
//!
//! ```json
//! { "format": "moonframe.document", "version": 2,
//!   "state": { "mod:shell": { "left": true } },
//!   "topics": { "tabs/main": { ... } } }
//! ```
//!
//! `version` lets a later format read an older one (version 1 kept each
//! mod's state under `mods`), and a document from a newer version is never
//! written over. A key herdr-core has nothing for is read from SQLite, so
//! what SQLite kept comes along: the workspace saves each state it loads.

use {
    moonframe_core::{Data, Persist},
    moonframe_mux::{Backends, Store},
    moonframe_providers::{from_json, to_json},
    serde_json::{Map, Value, json},
    std::{
        collections::BTreeMap,
        sync::{
            Arc, Mutex,
            mpsc::{self, Sender},
        },
        thread,
    },
};

const FORMAT: &str = "moonframe.document";
/// The format this build reads and writes.
const VERSION: u64 = 2;

/// The store the workspace saves to, or `None` to keep nothing. Each
/// profile keeps its own.
pub(crate) fn open(backends: &Backends, profile: Option<&str>) -> Option<Box<dyn Persist>> {
    let sqlite = moonframe_store::Store::open(&crate::paths::state(profile))
        .inspect_err(|error| tracing::warn!("{error}"))
        .ok();
    let name = match profile {
        Some(profile) => format!("{}-{profile}", crate::paths::NAME),
        None => crate::paths::NAME.to_owned(),
    };
    if let Some(store) = backends.store(&name) {
        match CorePersist::open(store, sqlite) {
            Ok(core) => {
                tracing::info!("keeping state in herdr-core");
                return Some(Box::new(core));
            },
            Err(problem) => {
                tracing::warn!("herdr-core keeps no state: {problem}");
                return None;
            },
        }
    }
    match sqlite {
        Some(store) => Some(Box::new(store)),
        None => {
            tracing::warn!("state will not be saved");
            None
        },
    }
}

/// What the document holds, by key.
#[derive(Clone, Debug, Default, PartialEq)]
struct Document {
    state: BTreeMap<String, Data>,
    topics: BTreeMap<String, Data>,
}

impl Document {
    fn to_json(&self) -> Value {
        let map = |values: &BTreeMap<String, Data>| {
            Value::Object(
                values
                    .iter()
                    .map(|(key, data)| (key.clone(), to_json(data)))
                    .collect::<Map<_, _>>(),
            )
        };
        json!({
            "format": FORMAT,
            "version": VERSION,
            "state": map(&self.state),
            "topics": map(&self.topics),
        })
    }

    /// Reads a document; one from a newer version, or not a document, is
    /// refused so it is never written over. A value that won't read is
    /// dropped on its own.
    fn from_json(value: &Value) -> Result<Self, String> {
        if value.get("format").and_then(Value::as_str) != Some(FORMAT) {
            return Err(format!("not a {FORMAT}"));
        }
        let version = value.get("version").and_then(Value::as_u64).unwrap_or(0);
        if version > VERSION {
            return Err(format!(
                "written by a newer version (format {version}; this one reads {VERSION})"
            ));
        }
        let read = |field: &str, prefix: &str| -> BTreeMap<String, Data> {
            let Some(Value::Object(values)) = value.get(field) else {
                return BTreeMap::new();
            };
            values
                .iter()
                .filter_map(|(key, value)| match from_json(value) {
                    Ok(data) => Some((format!("{prefix}{key}"), data)),
                    Err(problem) => {
                        tracing::warn!("saved `{key}` is unusable: {problem}");
                        None
                    },
                })
                .collect()
        };
        let mut state = read("state", "");
        // Version 1 kept each mod's state under its name.
        state.extend(read("mods", "mod:"));
        Ok(Self {
            state,
            topics: read("topics", ""),
        })
    }
}

enum Write {
    Changed,
    /// Answers once everything changed before it is saved.
    Flush(Sender<()>),
}

/// The document in herdr-core. Reads come from memory; each change wakes a
/// thread that saves the newest document, so a slow store never holds up
/// the window and a burst of changes costs one save.
struct CorePersist {
    document: Arc<Mutex<Document>>,
    writes: Sender<Write>,
    /// What SQLite kept, read for a key herdr-core has nothing for.
    sqlite: Option<moonframe_store::Store>,
}

impl CorePersist {
    fn open(store: Arc<dyn Store>, sqlite: Option<moonframe_store::Store>) -> Result<Self, String> {
        let document = match store.load()? {
            Some(value) => Document::from_json(&value)?,
            // First run with herdr-core: the topics SQLite kept come along.
            None => Document {
                state: BTreeMap::new(),
                topics: sqlite
                    .as_ref()
                    .map(moonframe_store::Store::topics)
                    .unwrap_or_default(),
            },
        };
        let document = Arc::new(Mutex::new(document));
        let (writes, pending) = mpsc::channel::<Write>();
        let saved = document.clone();
        thread::Builder::new()
            .name("state-writer".into())
            .spawn(move || {
                while let Ok(first) = pending.recv() {
                    let mut done = Vec::new();
                    for write in std::iter::once(first).chain(pending.try_iter()) {
                        if let Write::Flush(answer) = write {
                            done.push(answer);
                        }
                    }
                    let value = match saved.lock() {
                        Ok(document) => document.to_json(),
                        Err(_) => break,
                    };
                    if let Err(problem) = store.save(&value) {
                        tracing::warn!(store = store.name(), "cannot save state: {problem}");
                    }
                    for answer in done {
                        let _ = answer.send(());
                    }
                }
            })
            .map_err(|error| format!("cannot start the writer: {error}"))?;
        Ok(Self {
            document,
            writes,
            sqlite,
        })
    }

    fn change(&self, change: impl FnOnce(&mut Document)) {
        if let Ok(mut document) = self.document.lock() {
            change(&mut document);
            let _ = self.writes.send(Write::Changed);
        }
    }
}

impl Persist for CorePersist {
    fn state(&self, key: &str) -> Option<Data> {
        let kept = self.document.lock().ok()?.state.get(key).cloned();
        kept.or_else(|| self.sqlite.as_ref()?.state(key))
    }

    fn save_state(&self, key: &str, value: &Data) {
        self.change(|document| {
            document.state.insert(key.to_owned(), value.clone());
        });
    }

    fn topics(&self) -> BTreeMap<String, Data> {
        self.document
            .lock()
            .map(|document| document.topics.clone())
            .unwrap_or_default()
    }

    fn save_topic(&self, name: &str, value: &Data) {
        self.change(|document| {
            document.topics.insert(name.to_owned(), value.clone());
        });
    }

    fn flush(&self) {
        let (done, wait) = mpsc::channel();
        if self.writes.send(Write::Flush(done)).is_ok() {
            let _ = wait.recv();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A store in memory.
    #[derive(Default)]
    struct Memory(Mutex<Option<Value>>);

    impl Store for Memory {
        fn name(&self) -> &'static str {
            "memory"
        }

        fn load(&self) -> Result<Option<Value>, String> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn save(&self, document: &Value) -> Result<(), String> {
            *self.0.lock().unwrap() = Some(document.clone());
            Ok(())
        }
    }

    #[test]
    fn state_and_topics_round_trip_through_the_store() {
        let store = Arc::new(Memory::default());
        let first = CorePersist::open(store.clone(), None).unwrap();
        first.save_state("mod:shell", &Data::Bool(false));
        first.save_topic("tabs/main", &Data::Number(2.0));
        first.flush();

        let second = CorePersist::open(store, None).unwrap();
        assert_eq!(second.state("mod:shell"), Some(Data::Bool(false)));
        assert_eq!(second.topics()["tabs/main"], Data::Number(2.0));
    }

    #[test]
    fn version_one_reads_and_newer_is_refused() {
        let old = json!({ "format": FORMAT, "version": 1, "mods": { "shell": true } });
        let document = Document::from_json(&old).unwrap();
        assert_eq!(document.state["mod:shell"], Data::Bool(true));
        let newer = json!({ "format": FORMAT, "version": VERSION + 1 });
        assert!(Document::from_json(&newer).unwrap_err().contains("newer"));
        assert!(Document::from_json(&json!({ "spaces": [] })).is_err());
    }

    #[test]
    fn a_first_run_starts_from_what_sqlite_kept() {
        let sqlite = moonframe_store::Store::in_memory().unwrap();
        sqlite.save_state("mod:shell", &Data::Bool(true));
        sqlite.save_topic("tabs/main", &Data::Number(1.0));
        sqlite.flush();
        let core = CorePersist::open(Arc::new(Memory::default()), Some(sqlite)).unwrap();
        assert_eq!(core.state("mod:shell"), Some(Data::Bool(true)));
        assert_eq!(core.topics()["tabs/main"], Data::Number(1.0));
    }
}
