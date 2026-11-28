//! A shown diagram as a file: what the MCP server writes and the provider
//! reads, one JSON file per diagram in a folder both know. Files make the
//! two independent: an agent's server can start before the app, outlive
//! it, or come from an agent the app did not start.

use {
    serde_json::{Value, json},
    std::{
        io,
        path::{Path, PathBuf},
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// The largest diagram file read: a few thousand nodes of JSON.
pub const MOST_BYTES: u64 = 1 << 20;

/// A diagram an agent shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub id: String,
    pub title: String,
    /// Who shows it, when the server was told (`--agent`).
    pub agent: Option<String>,
    /// The app's agent run that showed it, when the app started the agent.
    pub run: Option<String>,
    /// Counts each change, so a view can tell a new version.
    pub revision: u64,
    /// When it last changed, Unix seconds.
    pub updated: u64,
    /// The document `ui.diagram` takes.
    pub document: Value,
    /// Node and edge ids to draw in the accent.
    pub highlight: Vec<String>,
    /// What the agent says about the highlight.
    pub note: Option<String>,
    /// The text it came from (`mermaid`, `dot`), to copy or send back.
    pub source: Option<(String, String)>,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Shown {
    pub fn to_json(&self) -> Value {
        let mut out = json!({
            "id": self.id,
            "title": self.title,
            "revision": self.revision,
            "updated": self.updated,
            "document": self.document,
            "highlight": self.highlight,
        });
        if let Some(agent) = &self.agent {
            out["agent"] = agent.clone().into();
        }
        if let Some(run) = &self.run {
            out["run"] = run.clone().into();
        }
        if let Some(note) = &self.note {
            out["note"] = note.clone().into();
        }
        if let Some((language, text)) = &self.source {
            out["source"] = json!({ "language": language, "text": text });
        }
        out
    }

    pub fn from_json(value: &Value) -> Result<Self, String> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let id = text("id").ok_or("no `id`")?;
        Ok(Self {
            title: text("title").unwrap_or_else(|| id.clone()),
            id,
            agent: text("agent"),
            run: text("run"),
            revision: value.get("revision").and_then(Value::as_u64).unwrap_or(1),
            updated: value.get("updated").and_then(Value::as_u64).unwrap_or(0),
            document: value.get("document").cloned().unwrap_or(Value::Null),
            highlight: value
                .get("highlight")
                .and_then(Value::as_array)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            note: text("note"),
            source: value.get("source").and_then(|s| {
                Some((
                    s.get("language")?.as_str()?.to_owned(),
                    s.get("text")?.as_str()?.to_owned(),
                ))
            }),
        })
    }
}

/// `id` if it may name a file: 1 to 64 of letters, digits, `-` and `_`.
pub fn safe_id(id: &str) -> Result<String, String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(id.to_owned())
    } else {
        Err(format!(
            "a diagram id is 1 to 64 letters, digits, `-` or `_`, not `{id}`"
        ))
    }
}

pub fn path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// Writes `shown` whole, so a reader never sees half a file.
pub fn write(dir: &Path, shown: &Shown) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let target = path(dir, &shown.id);
    let partial = dir.join(format!(".{}.{}.partial", shown.id, std::process::id()));
    std::fs::write(
        &partial,
        serde_json::to_vec_pretty(&shown.to_json()).map_err(io::Error::other)?,
    )?;
    std::fs::rename(&partial, &target)
}

pub fn remove(dir: &Path, id: &str) -> io::Result<()> {
    match std::fs::remove_file(path(dir, id)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

pub fn read(file: &Path) -> Result<Shown, String> {
    let meta = std::fs::metadata(file).map_err(|e| e.to_string())?;
    if meta.len() > MOST_BYTES {
        return Err(format!("larger than {} KiB", MOST_BYTES / 1024));
    }
    let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Shown::from_json(&value)
}

/// The folder the `diagrams` provider reads, once it started: its
/// `folder` setting, else [`default_dir`].
static AGENTS_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Says which folder the provider reads, so the server offered to agents
/// writes there.
pub fn set_agents_dir(dir: &Path) {
    if let Ok(mut known) = AGENTS_DIR.lock() {
        *known = Some(dir.to_owned());
    }
}

/// Where agents' diagrams go: the folder the provider reads, else
/// [`default_dir`].
pub fn agents_dir() -> PathBuf {
    AGENTS_DIR
        .lock()
        .ok()
        .and_then(|known| known.clone())
        .unwrap_or_else(default_dir)
}

/// Where agents' diagrams go by default: `$MOONFRAME_DIAGRAMS`, else the
/// cache folder's `moonframe/diagrams`.
pub fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MOONFRAME_DIAGRAMS").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    cache.join("moonframe").join("diagrams")
}
