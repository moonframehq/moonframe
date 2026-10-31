//! What the app keeps across launches: one JSON document per app, with
//! the state each mod asks to keep (`persist`) under its name.
//!
//! ```json
//! { "format": "moonframe.document", "version": 1,
//!   "mods": { "shell": { "left": true }, "explorer": { "open": ["src"] } } }
//! ```
//!
//! Mods are namespaced, so any mod keeps its own state without the app
//! knowing its shape, and `version` lets a later format migrate an older
//! one; a document from a newer version is never written over.
//!
//! A [`Store`] holds it: herdr-core's document store when herdr-core runs
//! here (beside the terminals it keeps), else a file.

use {
    serde_json::{Map, Value, json},
    std::{
        collections::BTreeMap,
        fs,
        io::{ErrorKind, Write as _},
        path::PathBuf,
    },
};

pub const FORMAT: &str = "moonframe.document";
/// The format this build reads and writes.
pub const VERSION: u64 = 1;

/// The state each mod keeps, by mod name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub mods: BTreeMap<String, Value>,
}

impl Document {
    pub fn to_json(&self) -> Value {
        json!({
            "format": FORMAT,
            "version": VERSION,
            "mods": Value::Object(self.mods.clone().into_iter().collect::<Map<_, _>>()),
        })
    }

    /// Reads a document; one from a newer version, or not a document, is
    /// refused so it is never written over.
    pub fn from_json(value: &Value) -> Result<Self, String> {
        if value.get("format").and_then(Value::as_str) != Some(FORMAT) {
            return Err(format!("not a {FORMAT}"));
        }
        let version = value.get("version").and_then(Value::as_u64).unwrap_or(0);
        if version > VERSION {
            return Err(format!(
                "written by a newer version (format {version}; this one reads {VERSION})"
            ));
        }
        let mods = match value.get("mods") {
            Some(Value::Object(mods)) => mods.clone().into_iter().collect(),
            None | Some(Value::Null) => BTreeMap::new(),
            Some(_) => return Err("`mods` is not an object".into()),
        };
        Ok(Self { mods })
    }
}

/// Where the document lives. Calls may block: use them off the UI thread.
pub trait Store: Send + Sync {
    /// Its short name, for logs: `herdr-core` or `file`.
    fn name(&self) -> &'static str;

    /// The saved document, or `None` when there is none yet.
    fn load(&self) -> Result<Option<Document>, String>;

    fn save(&self, document: &Document) -> Result<(), String>;
}

/// The document in a JSON file, replaced atomically.
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Store for FileStore {
    fn name(&self) -> &'static str {
        "file"
    }

    fn load(&self) -> Result<Option<Document>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("{}: {error}", self.path.display())),
        };
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| format!("{}: {error}", self.path.display()))?;
        Document::from_json(&value)
            .map(Some)
            .map_err(|error| format!("{}: {error}", self.path.display()))
    }

    fn save(&self, document: &Document) -> Result<(), String> {
        let within = |error: std::io::Error| format!("{}: {error}", self.path.display());
        if let Some(folder) = self.path.parent() {
            fs::create_dir_all(folder).map_err(within)?;
        }
        let text =
            serde_json::to_vec_pretty(&document.to_json()).map_err(|error| error.to_string())?;
        // Written beside it, then moved over it, so a crash never leaves
        // half a document.
        let mut temporary = self.path.clone().into_os_string();
        temporary.push(format!(".{}.tmp", std::process::id()));
        let temporary = PathBuf::from(temporary);
        let written = fs::File::create(&temporary)
            .and_then(|mut file| file.write_all(&text).and_then(|()| file.sync_all()))
            .and_then(|()| fs::rename(&temporary, &self.path));
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        written.map_err(within)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_document_round_trips_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path().join("nested/document.json"));
        assert_eq!(store.load().unwrap(), None);
        let document = Document {
            mods: BTreeMap::from([("shell".to_owned(), json!({ "left": false }))]),
        };
        store.save(&document).unwrap();
        assert_eq!(store.load().unwrap(), Some(document));
    }

    #[test]
    fn a_newer_or_foreign_document_is_refused() {
        let newer = json!({ "format": FORMAT, "version": VERSION + 1, "mods": {} });
        assert!(Document::from_json(&newer).unwrap_err().contains("newer"));
        assert!(Document::from_json(&json!({ "spaces": [] })).is_err());
        let older = json!({ "format": FORMAT, "version": 0 });
        assert_eq!(Document::from_json(&older).unwrap(), Document::default());
    }
}
