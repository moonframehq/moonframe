//! The `diagrams` provider: the diagrams agents show (files the MCP server
//! writes) and the project's own (`.moonframe/diagrams/*.mmd`, `.dot`,
//! `.json`), shared as `app.diagrams`, newest first, each with the
//! `document` `ui.diagram` takes.

use {
    crate::{document, store},
    moonframe_core::Data,
    moonframe_providers::{Handle, Native},
    serde_json::{Value, json},
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// The most files read from one folder.
const MOST_FILES: usize = 200;
/// How many values the documents of one publish may hold together (a
/// publish holds at most 10,000): diagrams past it are listed without
/// their document, newest first keep theirs.
const BUDGET: usize = 8_000;

/// Where a file is from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum From {
    Agent,
    Project,
}

impl From {
    fn name(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Project => "project",
        }
    }
}

#[derive(Default)]
pub struct Diagrams {
    agents: PathBuf,
    project: Option<PathBuf>,
    /// Each file read, by path: when it changed and how big it was, and
    /// what it read as, so an unchanged file is not read again.
    read: HashMap<PathBuf, (SystemTime, u64, Value)>,
    last: Option<Value>,
}

pub fn native() -> Box<dyn Native> {
    Box::<Diagrams>::default()
}

impl Native for Diagrams {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        self.agents = match host.settings().get("folder").and_then(Data::as_str) {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => store::default_dir(),
        };
        self.project = project_folder(moonframe_providers::project());
        self.refresh(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        self.refresh(host);
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        if from.is_empty() {
            // The app telling it the project moved.
            self.project = project_folder(moonframe_providers::project());
        } else if let Some(id) = message.get("close").and_then(Data::as_str) {
            // Only an agent's diagram goes, and only its own file.
            if let Ok(id) = store::safe_id(id) {
                let _ = store::remove(&self.agents, &id);
            }
        }
        self.refresh(host);
    }
}

fn project_folder(project: Option<PathBuf>) -> Option<PathBuf> {
    project.map(|p| p.join(".moonframe").join("diagrams"))
}

impl Diagrams {
    fn refresh(&mut self, host: &Handle) {
        let mut entries = self.scan(&self.agents.clone(), From::Agent);
        if let Some(project) = self.project.clone() {
            entries.extend(self.scan(&project, From::Project));
        }
        entries.sort_by_key(|e| {
            std::cmp::Reverse(e.get("updated").and_then(Value::as_u64).unwrap_or(0))
        });
        let mut spent = 0;
        for entry in &mut entries {
            let weight = entry.get("document").map_or(0, document::weight);
            if spent + weight > BUDGET {
                if let Value::Object(map) = entry {
                    map.remove("document");
                    map.insert("truncated".into(), true.into());
                }
            } else {
                spent += weight;
            }
        }
        let published = json!({
            "available": true,
            "agents": self.agents.to_string_lossy(),
            "project": self.project.as_ref().map(|p| p.to_string_lossy().into_owned()),
            "diagrams": entries,
        });
        if self.last.as_ref() != Some(&published) {
            host.publish_json(&published);
            self.last = Some(published);
        }
    }

    /// The diagrams in `dir`, each read again only when it changed.
    fn scan(&mut self, dir: &Path, from: From) -> Vec<Value> {
        let Ok(listing) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for file in listing.flatten().map(|e| e.path()).take(MOST_FILES) {
            let Some(language) = language(&file) else {
                continue;
            };
            let Ok(meta) = std::fs::metadata(&file) else {
                continue;
            };
            let modified = meta.modified().unwrap_or(UNIX_EPOCH);
            if let Some((when, len, entry)) = self.read.get(&file)
                && *when == modified
                && *len == meta.len()
            {
                out.push(entry.clone());
                continue;
            }
            let entry = entry(&file, language, from, &meta, modified);
            self.read
                .insert(file, (modified, meta.len(), entry.clone()));
            out.push(entry);
        }
        out
    }
}

/// The language a file's name says, `None` for a file that is not a
/// diagram (or a write still under way).
fn language(file: &Path) -> Option<&'static str> {
    let name = file.file_name()?.to_str()?;
    if name.starts_with('.') {
        return None;
    }
    match file.extension()?.to_str()? {
        "json" => Some("json"),
        "mmd" | "mermaid" => Some("mermaid"),
        "dot" | "gv" => Some("dot"),
        _ => None,
    }
}

fn entry(
    file: &Path,
    language: &str,
    from: From,
    meta: &std::fs::Metadata,
    modified: SystemTime,
) -> Value {
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("diagram")
        .to_owned();
    let seconds = modified
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut out = json!({
        "key": format!("{}/{stem}", from.name()),
        "id": stem,
        "title": stem,
        "from": from.name(),
        "path": file.to_string_lossy(),
        "revision": seconds,
        "updated": seconds,
        "highlight": [],
    });
    if meta.len() > store::MOST_BYTES {
        out["error"] = format!("larger than {} KiB", store::MOST_BYTES / 1024).into();
        return out;
    }
    if language == "json" {
        match store::read(file) {
            Ok(shown) => {
                let checked = if shown.document.get("nodes").is_some()
                    || shown.document.get("messages").is_some()
                {
                    document::check(&shown.document)
                } else {
                    Err("no document".to_owned())
                };
                out["title"] = shown.title.into();
                out["revision"] = shown.revision.into();
                if shown.updated > 0 {
                    out["updated"] = shown.updated.into();
                }
                out["highlight"] = shown.highlight.into();
                if let Some(agent) = shown.agent {
                    out["agent"] = agent.into();
                }
                if let Some(note) = shown.note {
                    out["note"] = note.into();
                }
                if let Some((language, text)) = shown.source {
                    out["language"] = language.into();
                    out["text"] = text.into();
                }
                match checked {
                    Ok(doc) => out["document"] = doc,
                    Err(e) => out["error"] = e.into(),
                }
            },
            Err(e) => out["error"] = e.into(),
        }
        return out;
    }
    match std::fs::read_to_string(file) {
        Ok(text) => {
            match document::from_text(language, &text) {
                Ok(doc) => out["document"] = doc,
                Err(e) => out["error"] = e.into(),
            }
            out["language"] = language.into();
            out["text"] = text.into();
        },
        Err(e) => out["error"] = e.to_string().into(),
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {
        super::*,
        moonframe_providers::Event,
        std::sync::{Arc, Mutex},
    };

    fn handle(seen: &Arc<Mutex<Vec<Value>>>, folder: &Path) -> Handle {
        let seen = Arc::clone(seen);
        let events: moonframe_providers::Events = Arc::new(move |event| {
            if let Event::Published { data, .. } = event {
                seen.lock()
                    .unwrap()
                    .push(moonframe_providers::to_json(&data));
            }
        });
        let settings = Data::map([(
            "folder",
            Data::String(folder.to_string_lossy().into_owned()),
        )]);
        Handle::new("diagrams", Path::new("."), settings, events)
    }

    #[test]
    fn it_shares_agents_diagrams_and_follows_their_changes() {
        let dir = tempfile::tempdir().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let host = handle(&seen, dir.path());
        let mut provider = Diagrams::default();
        provider.start(&host).unwrap();
        let mut server = crate::mcp::Server::new(dir.path(), Some("codex".into()));
        let show = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "diagram_show", "arguments": { "id": "flow", "mermaid": "graph TD; a-->b" } } });
        server.answer(&show.to_string());
        std::fs::write(dir.path().join("broken.mmd"), "flowchart TD\n A ->> B").unwrap();
        provider.tick(&host);
        let last = seen.lock().unwrap().last().cloned().unwrap();
        let diagrams = last["diagrams"].as_array().unwrap();
        let flow = diagrams.iter().find(|d| d["id"] == "flow").unwrap();
        assert_eq!(flow["agent"], "codex");
        assert_eq!(flow["document"]["nodes"].as_array().unwrap().len(), 2);
        let broken = diagrams.iter().find(|d| d["id"] == "broken").unwrap();
        assert!(broken["error"].as_str().unwrap().contains("line 2"));
        // Nothing changed: nothing published again.
        let count = seen.lock().unwrap().len();
        provider.tick(&host);
        assert_eq!(seen.lock().unwrap().len(), count);
        // A mod closes the agent's diagram.
        provider.receive(
            &host,
            "editor",
            Data::map([("close", Data::String("flow".into()))]),
        );
        assert!(!store::path(dir.path(), "flow").exists());
    }
}
