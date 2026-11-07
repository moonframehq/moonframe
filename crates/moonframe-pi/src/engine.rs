//! The app's model: workspaces and threads, what each thread's pi is doing,
//! and all `app.pi` is made from. Conversations live in pi's session files;
//! the store only says where they belong. Everything here is quick and
//! holds no pi to wait on, as it runs under [`Core`](crate::core::Core)'s
//! lock; `flows.rs` has what takes time. See `snapshot.rs` for the shape
//! published, and `commands.rs` for the messages taken.

use {
    crate::{
        Config,
        core::Notify,
        live::Live,
        rpc::Rpc,
        sessions::{self, Summary},
        snapshot::Found,
        store::{Meta, State, Store},
        value::{as_list, clip, now_seconds},
    },
    serde_json::{Value, json},
    std::{
        collections::{BTreeMap, HashMap, HashSet},
        path::Path,
        sync::Arc,
    },
};

/// Events kept for the window to pick up.
const KEPT_EVENTS: usize = 12;
const LONGEST_EVENT: usize = 300;

/// What a thread has said: a message of pi's, or a line of our own.
#[derive(Clone, Debug, Default)]
pub(crate) struct Entry {
    pub(crate) id: Option<String>,
    pub(crate) message: Option<Value>,
    pub(crate) note: Option<String>,
    pub(crate) tone: Option<String>,
}

/// The pi in use.
#[derive(Clone, Debug)]
pub(crate) enum PiStatus {
    Looking,
    Ready {
        path: std::path::PathBuf,
        version: String,
    },
    Missing {
        reason: String,
    },
}

/// A checkout's branch, and whether it is a repository.
#[derive(Clone, Debug, Default)]
pub(crate) struct Branch {
    pub(crate) branch: Option<String>,
    pub(crate) git: bool,
}

/// The app's state, guarded by the core's lock.
pub(crate) struct Engine {
    pub(crate) config: Arc<Config>,
    notify: Arc<Notify>,
    pub(crate) store: Store,
    /// Where threads with no project run, links resolved.
    pub(crate) chats: String,
    pub(crate) live: BTreeMap<String, Live>,
    pub(crate) entries: HashMap<String, Vec<Entry>>,
    pub(crate) summaries: HashMap<String, Summary>,
    pub(crate) branches: HashMap<String, Branch>,
    pub(crate) expanded: HashMap<String, HashSet<String>>,
    /// Words of failed sends, by thread, until its composer has them back.
    pub(crate) returned: HashMap<String, Value>,
    pub(crate) shown: HashMap<String, usize>,
    pub(crate) events: Vec<Value>,
    seq: u64,
    pub(crate) pi: PiStatus,
    pub(crate) models: Vec<Value>,
    pub(crate) commands: Vec<Value>,
    pub(crate) stamp: String,
    pub(crate) probe: Option<Arc<Rpc>>,
    pub(crate) prefill: Option<Value>,
    pub(crate) resources_asked: u64,
    pub(crate) resources: Option<(String, Found)>,
    pub(crate) rows_text: String,
    pub(crate) rows_version: u64,
}

impl Engine {
    pub(crate) fn new(config: Arc<Config>, notify: Arc<Notify>) -> Result<Self, String> {
        let store = Store::load(&config.data_dir);
        let chats = config.chats_dir();
        std::fs::create_dir_all(&chats).map_err(|error| error.to_string())?;
        let chats = crate::config::real_path(&chats.to_string_lossy())?;
        Ok(Self {
            config,
            notify,
            store,
            chats,
            live: BTreeMap::new(),
            entries: HashMap::new(),
            summaries: HashMap::new(),
            branches: HashMap::new(),
            expanded: HashMap::new(),
            returned: HashMap::new(),
            shown: HashMap::new(),
            events: Vec::new(),
            seq: 0,
            pi: PiStatus::Looking,
            models: Vec::new(),
            commands: Vec::new(),
            stamp: String::new(),
            probe: None,
            prefill: None,
            resources_asked: 0,
            resources: None,
            rows_text: String::new(),
            rows_version: 0,
        })
    }

    pub(crate) fn data(&self) -> &State {
        &self.store.data
    }

    pub(crate) fn data_mut(&mut self) -> &mut State {
        &mut self.store.data
    }

    /// Asks for a publish, soon.
    pub(crate) fn changed(&self) {
        self.notify.changed();
    }

    pub(crate) fn selected(&self) -> Option<&str> {
        self.data().selected.as_deref()
    }

    pub(crate) fn thread(&self, id: &str) -> Option<&Meta> {
        self.data().threads.get(id)
    }

    pub(crate) fn thread_mut(&mut self, id: &str) -> Option<&mut Meta> {
        self.data_mut().threads.get_mut(id)
    }

    /// Takes in what looking at session files found, adopting sessions pi
    /// wrote in a workspace, the CLI's included.
    pub(crate) fn merge_sessions(&mut self, found: Vec<(String, Vec<Summary>)>) {
        for (cwd, list) in found {
            for session in list {
                let id = session.id.clone();
                let threads = &mut self.store.data.threads;
                let meta = threads.entry(id.clone()).or_insert_with(|| Meta {
                    id: id.clone(),
                    cwd: cwd.clone(),
                    workspace: cwd.clone(),
                    ..Meta::default()
                });
                if meta.file.as_deref() != Some(session.file.as_str()) {
                    meta.file = Some(session.file.clone());
                    self.store.save();
                }
                self.summaries.insert(id, session);
            }
        }
        self.changed();
    }

    /// The folders to look at for sessions.
    pub(crate) fn known_folders(&self) -> Vec<String> {
        let workspaces = self.data().workspaces.iter().map(|item| item.path.clone());
        let threads = self.data().threads.values().map(|meta| meta.cwd.clone());
        let mut seen = HashSet::new();
        workspaces
            .chain(threads)
            .filter(|cwd| seen.insert(cwd.clone()))
            .collect()
    }

    /// What a thread has said, as the timeline reads it.
    pub(crate) fn load_entries(&mut self, id: &str) {
        let file = self.thread(id).and_then(|meta| meta.file.clone());
        let Some(file) = file.filter(|file| Path::new(file).exists()) else {
            self.entries.entry(id.to_owned()).or_default();
            return;
        };
        let all = sessions::parse(Path::new(&file));
        let list = sessions::active_branch(&all)
            .into_iter()
            .filter_map(entry_of)
            .collect();
        self.entries.insert(id.to_owned(), list);
    }

    pub(crate) fn title_of(&self, id: &str) -> String {
        let info = self.summaries.get(id);
        let filled = |text: &&String| !text.is_empty();
        let name = info.and_then(|info| info.name.as_ref()).filter(filled);
        let title = self
            .thread(id)
            .and_then(|meta| meta.title.as_ref())
            .filter(filled);
        let prompt = info.and_then(|info| info.prompt.as_ref()).filter(filled);
        name.or(title)
            .or(prompt)
            .cloned()
            .unwrap_or_else(|| "New thread".to_owned())
    }

    /// Says something to the window, once.
    pub(crate) fn emit(
        &mut self,
        kind: &str,
        thread: Option<&str>,
        title: &str,
        body: &str,
        level: &str,
    ) {
        self.seq += 1;
        let body: String = body.chars().take(LONGEST_EVENT).collect();
        self.events.push(json!({
            "seq": self.seq,
            "kind": kind,
            "thread": thread,
            "title": title,
            "body": body,
            "level": level,
        }));
        if self.events.len() > KEPT_EVENTS {
            self.events.remove(0);
        }
        self.changed();
    }

    pub(crate) fn mark_failed(&mut self, id: &str) {
        let selected = self.selected() == Some(id);
        if let Some(meta) = self.thread_mut(id) {
            meta.failed = !selected;
            self.store.save();
        }
    }

    pub(crate) fn add_workspace(&mut self, real: &str) {
        let known = self.data().workspaces.iter().any(|item| item.path == real);
        if !known {
            self.data_mut().workspaces.push(crate::store::Workspace {
                path: real.to_owned(),
                name: crate::config::base_name(real),
                rest: serde_json::Map::new(),
            });
            self.store.save();
        }
    }

    /// Adds a new thread, with nothing said yet.
    pub(crate) fn insert_thread(&mut self, meta: Meta) {
        self.entries.insert(meta.id.clone(), Vec::new());
        self.data_mut().threads.insert(meta.id.clone(), meta);
        self.store.save();
    }

    /// Pushes a message pi just finished.
    pub(crate) fn push_message(&mut self, id: &str, message: Value) {
        if let Some(list) = self.entries.get_mut(id) {
            list.push(Entry {
                message: Some(message),
                ..Entry::default()
            });
        }
        self.changed();
    }

    /// Records a run's changes under its prompt's timestamp.
    pub(crate) fn record_turn(&mut self, id: &str, key: String, turn: Value) {
        let Some(meta) = self.thread_mut(id) else {
            return;
        };
        meta.turns.insert(key, turn);
        if meta.turns.len() > KEPT_TURNS {
            // Keys are millisecond timestamps: the smallest is the oldest.
            let oldest = meta
                .turns
                .keys()
                .min_by_key(|key| key.parse::<u64>().unwrap_or(0))
                .cloned();
            if let Some(oldest) = oldest {
                meta.turns.remove(&oldest);
            }
        }
        self.store.save();
    }

    /// The last answer of a thread, if any.
    pub(crate) fn last_answer(&self, id: &str) -> Option<&Value> {
        let list = self.entries.get(id)?;
        let messages = list.iter().rev().filter_map(|entry| entry.message.as_ref());
        messages
            .into_iter()
            .find(|message| message["role"] == "assistant")
    }

    /// The content of a thread's first prompt.
    pub(crate) fn first_prompt(&self, id: &str) -> Option<&Value> {
        let list = self.entries.get(id)?;
        let messages = list.iter().filter_map(|entry| entry.message.as_ref());
        messages
            .into_iter()
            .find(|message| message["role"] == "user")
            .map(|message| &message["content"])
    }
}

const KEPT_TURNS: usize = 60;

fn entry_of(entry: &Value) -> Option<Entry> {
    let id = entry["id"].as_str().map(str::to_owned);
    match entry["type"].as_str()? {
        "message" if entry["message"]["role"] != "system" => Some(Entry {
            id,
            message: Some(entry["message"].clone()),
            ..Entry::default()
        }),
        "compaction" => {
            let tokens = entry["tokensBefore"].as_f64().unwrap_or(0.0);
            let thousands = (tokens / 1000.0).round();
            Some(Entry {
                id,
                note: Some(format!("Compacted {thousands}k tokens of context")),
                ..Entry::default()
            })
        },
        "branch_summary" => Some(Entry {
            id,
            note: Some("Came back from another branch".to_owned()),
            ..Entry::default()
        }),
        _ => None,
    }
}

/// The first line of `text`, short enough to be a title.
pub(crate) fn first_words(text: &str) -> Option<String> {
    let line = text.trim().split('\n').next().unwrap_or_default();
    Some(clip(line, 60)).filter(|line| !line.is_empty())
}

/// A new thread's fields.
pub(crate) fn new_meta(
    id: String,
    cwd: String,
    workspace: String,
    title: Option<String>,
    model: Option<Value>,
    thinking: Option<String>,
) -> Meta {
    Meta {
        id,
        cwd,
        workspace,
        title,
        created: Some(now_seconds()),
        model,
        thinking,
        ..Meta::default()
    }
}

/// The model picked in a message, as `{ provider, id }`, if it has one.
pub(crate) fn picked_model(value: &Value) -> Option<Value> {
    value["provider"]
        .is_string()
        .then(|| json!({ "provider": value["provider"], "id": value["id"] }))
}

/// A list of paths in a message; an empty one may arrive as `{}`.
pub(crate) fn paths_of(value: &Value) -> Vec<String> {
    let names = as_list(value).iter().filter_map(Value::as_str);
    names.map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_the_first_line() {
        assert_eq!(first_words("  fix it\nplease").as_deref(), Some("fix it"));
        assert_eq!(first_words(""), None);
        assert_eq!(
            first_words(&"x".repeat(80)).map(|t| t.chars().count()),
            Some(60)
        );
    }

    #[test]
    fn notes_come_from_compactions_and_branches() {
        let entry = entry_of(&json!({ "type": "compaction", "id": "c", "tokensBefore": 12_400 }));
        let note = entry.and_then(|entry| entry.note);
        assert_eq!(note.as_deref(), Some("Compacted 12k tokens of context"));
        assert!(entry_of(&json!({ "type": "session_info" })).is_none());
        let system = json!({ "type": "message", "message": { "role": "system" } });
        assert!(entry_of(&system).is_none());
    }
}
