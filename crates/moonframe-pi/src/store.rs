//! What this app remembers that pi does not: the workspaces, each thread's
//! place and flags, schedules and preferences. Conversations stay in pi's
//! own session files. The file is the one the Node provider wrote: what it
//! holds carries over, and fields not known here stay in it.

use {
    serde::{Deserialize, Serialize},
    serde_json::{Map, Value, json},
    std::{
        collections::BTreeMap,
        fs, io,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    },
};

/// How long a save waits for more changes.
const SAVE_DELAY: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct Workspace {
    pub(crate) path: String,
    pub(crate) name: String,
    #[serde(flatten)]
    pub(crate) rest: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct Worktree {
    pub(crate) path: String,
    pub(crate) branch: String,
}

/// One thread.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct Meta {
    pub(crate) id: String,
    pub(crate) cwd: String,
    pub(crate) workspace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree: Option<Worktree>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) created: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) thinking: Option<String>,
    /// Each finished run's changes, by its prompt's timestamp.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub(crate) turns: Map<String, Value>,
    #[serde(default)]
    pub(crate) pinned: bool,
    #[serde(default)]
    pub(crate) archived: bool,
    #[serde(default)]
    pub(crate) unread: bool,
    #[serde(default)]
    pub(crate) failed: bool,
    #[serde(flatten)]
    pub(crate) rest: Map<String, Value>,
}

/// The file's contents.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct State {
    pub(crate) version: Value,
    pub(crate) workspaces: Vec<Workspace>,
    pub(crate) threads: BTreeMap<String, Meta>,
    pub(crate) schedules: Vec<Map<String, Value>>,
    pub(crate) selected: Option<String>,
    pub(crate) prefs: Map<String, Value>,
    #[serde(flatten)]
    pub(crate) rest: Map<String, Value>,
}

fn blank_prefs() -> Map<String, Value> {
    let prefs =
        json!({ "notifications": true, "sounds": true, "auto_title": true, "group": "time" });
    prefs.as_object().cloned().unwrap_or_default()
}

fn blank() -> State {
    State {
        version: json!(1),
        workspaces: Vec::new(),
        threads: BTreeMap::new(),
        schedules: Vec::new(),
        selected: None,
        prefs: blank_prefs(),
        rest: Map::new(),
    }
}

/// Whatever of `value` reads as a `T`, each on its own: one odd entry must
/// not cost the others, as the next save would write them away.
fn each<T: for<'de> Deserialize<'de>>(
    value: Option<Value>,
    what: &str,
) -> Vec<(Option<String>, T)> {
    let read = |key: Option<String>, item: Value| match serde_json::from_value(item) {
        Ok(item) => Some((key, item)),
        Err(error) => {
            tracing::warn!("pilot: skipping a {what} in state.json: {error}");
            None
        },
    };
    match value {
        Some(Value::Object(fields)) => fields
            .into_iter()
            .filter_map(|(key, item)| read(Some(key), item))
            .collect(),
        Some(Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| read(None, item))
            .collect(),
        _ => Vec::new(),
    }
}

fn parse(text: &str) -> Result<State, serde_json::Error> {
    let mut saved: Map<String, Value> = serde_json::from_str(text)?;
    let mut state = blank();
    if let Some(version) = saved.remove("version") {
        state.version = version;
    }
    let workspaces = each::<Workspace>(saved.remove("workspaces"), "workspace");
    state.workspaces = workspaces.into_iter().map(|(_, item)| item).collect();
    let threads = each::<Meta>(saved.remove("threads"), "thread");
    state.threads = threads
        .into_iter()
        .filter_map(|(key, meta)| Some((key?, meta)))
        .collect();
    let schedules = each::<Map<String, Value>>(saved.remove("schedules"), "schedule");
    state.schedules = schedules.into_iter().map(|(_, task)| task).collect();
    state.selected = saved
        .remove("selected")
        .and_then(|selected| selected.as_str().map(str::to_owned));
    if let Some(Value::Object(prefs)) = saved.remove("prefs") {
        state.prefs.extend(prefs);
    }
    state.rest = saved;
    Ok(state)
}

/// The state, and saving it soon after it changes.
pub(crate) struct Store {
    file: PathBuf,
    pub(crate) data: State,
    due: Option<Instant>,
}

impl Store {
    pub(crate) fn load(dir: &Path) -> Self {
        let file = dir.join("state.json");
        let data = match fs::read_to_string(&file) {
            Ok(text) => parse(&text).unwrap_or_else(|error| {
                tracing::error!("pilot: cannot read {}: {error}", file.display());
                blank()
            }),
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound {
                    tracing::error!("pilot: cannot read {}: {error}", file.display());
                }
                blank()
            },
        };
        Self {
            file,
            data,
            due: None,
        }
    }

    /// Asks for a save shortly.
    pub(crate) fn save(&mut self) {
        self.due.get_or_insert_with(|| Instant::now() + SAVE_DELAY);
    }

    /// Saves if it has waited long enough.
    pub(crate) fn flush_if_due(&mut self) {
        if self.due.is_some_and(|due| Instant::now() >= due) {
            self.flush();
        }
    }

    /// Saves now, if anything is waiting.
    pub(crate) fn flush_pending(&mut self) {
        if self.due.is_some() {
            self.flush();
        }
    }

    fn flush(&mut self) {
        self.due = None;
        if let Err(error) = self.write() {
            tracing::error!("pilot: cannot save {}: {error}", self.file.display());
        }
    }

    fn write(&self) -> io::Result<()> {
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut text = Vec::new();
        let format = serde_json::ser::PrettyFormatter::with_indent(b"\t");
        let mut writer = serde_json::Serializer::with_formatter(&mut text, format);
        self.data.serialize(&mut writer).map_err(io::Error::other)?;
        let scratch = self
            .file
            .with_extension(format!("json.{}", std::process::id()));
        fs::write(&scratch, text)?;
        fs::rename(&scratch, &self.file)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// As the Node provider left it.
    const NODE_STATE: &str = r#"{
	"version": 1,
	"workspaces": [{ "path": "/code/cart", "name": "cart" }],
	"threads": {
		"a": {
			"id": "a",
			"cwd": "/code/cart",
			"workspace": "/code/cart",
			"worktree": null,
			"title": "Fix it",
			"created": 1762550000.5,
			"model": null,
			"thinking": null,
			"file": "/agent/sessions/x/a.jsonl",
			"pinned": true,
			"turns": { "1762550000123": { "files": [], "added": 1, "deleted": 2, "before": "b", "after": "c" } }
		},
		"b": { "id": "b", "cwd": "/code/cart", "workspace": "/code/cart", "unread": false, "worktree": { "path": "/w", "branch": "pilot/x" } }
	},
	"schedules": [{ "id": "s", "name": "Nightly", "next": 1.5, "custom": "kept" }],
	"selected": "a",
	"prefs": { "group": "project", "last_model": { "provider": "mock", "id": "m" } },
	"later": "a field from a newer version"
}"#;

    #[test]
    fn what_the_node_provider_saved_carries_over() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("state.json"), NODE_STATE).unwrap();
        let mut store = Store::load(dir.path());
        let a = &store.data.threads["a"];
        assert_eq!(a.title.as_deref(), Some("Fix it"));
        assert!(a.pinned);
        assert_eq!(a.turns["1762550000123"]["added"], 1);
        let worktree = store.data.threads["b"].worktree.clone().unwrap();
        assert_eq!(worktree.branch, "pilot/x");
        assert_eq!(store.data.prefs["group"], "project");
        assert_eq!(store.data.prefs["sounds"], true, "new defaults fill in");
        assert_eq!(store.data.selected.as_deref(), Some("a"));

        store.save();
        store.flush_pending();
        let again: Value =
            serde_json::from_str(&fs::read_to_string(dir.path().join("state.json")).unwrap())
                .unwrap();
        assert_eq!(again["later"], "a field from a newer version");
        assert_eq!(again["schedules"][0]["custom"], "kept");
        assert_eq!(again["threads"]["a"]["file"], "/agent/sessions/x/a.jsonl");
        assert_eq!(again["threads"]["a"]["created"], 1_762_550_000.5);
        assert_eq!(
            again["threads"]["a"]["turns"]["1762550000123"]["before"],
            "b"
        );
    }

    #[test]
    fn one_odd_thread_costs_only_itself() {
        let dir = tempfile::tempdir().unwrap();
        let text =
            r#"{ "threads": { "a": { "id": "a", "cwd": "/c", "workspace": "/c" }, "b": 7 } }"#;
        fs::write(dir.path().join("state.json"), text).unwrap();
        let store = Store::load(dir.path());
        assert_eq!(store.data.threads.len(), 1);
    }

    #[test]
    fn a_save_waits_a_moment_and_leaves_no_scratch_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path());
        store.save();
        store.flush_if_due();
        assert!(!dir.path().join("state.json").exists());
        std::thread::sleep(SAVE_DELAY * 2);
        store.flush_if_due();
        assert!(dir.path().join("state.json").exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
