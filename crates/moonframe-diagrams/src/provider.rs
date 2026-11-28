//! The `diagrams` provider: the diagrams agents show (files the MCP server
//! writes) and the project's own (`.moonframe/diagrams/*.mmd`, `.dot`,
//! `.json`), shared as `app.diagrams`, newest first, each with the
//! `document` `ui.diagram` takes.

mod folder;

use {
    crate::{document, store},
    folder::Folder,
    moonframe_core::Data,
    moonframe_providers::{Handle, MAX_ENTRIES, Native},
    serde_json::{Value, json},
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// The most files read from one folder.
const MOST_FILES: usize = 200;
/// How many values one publish may hold, every field counted (a publish
/// holds at most [`MAX_ENTRIES`]): diagrams past it are listed without
/// their document, newest first keep theirs, and any past it even so are
/// left out.
const BUDGET: usize = MAX_ENTRIES - 16;

/// Where a file is from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum From {
    // keep-sorted start
    Agent,
    Project,
    // keep-sorted end
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
    /// The project, whose `.moonframe/diagrams` is checked again on each
    /// scan (it may come later, or be swapped for a link).
    project: Option<PathBuf>,
    /// Each file of the last scan, by path: when it changed and how big it
    /// was, and what it read as, so an unchanged file is not read again.
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
        store::set_agents_dir(&self.agents);
        self.project = moonframe_providers::project();
        self.refresh(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        self.refresh(host);
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        if from.is_empty() {
            // The app telling it the project moved.
            self.project = moonframe_providers::project();
        } else if let Some(key) = message.get("close").and_then(Data::as_str) {
            // Only an agent's diagram goes, and only the very file listed.
            if let Some(id) = closable(key) {
                let _ = store::remove(&self.agents, &id);
            }
        }
        self.refresh(host);
    }
}

impl Diagrams {
    fn refresh(&mut self, host: &Handle) {
        // Files gone since (or of a project left) are forgotten.
        let mut before = std::mem::take(&mut self.read);
        let mut entries = match Folder::open(&self.agents) {
            Ok(agents) => self.scan(&agents, From::Agent, &mut before),
            Err(_) => Vec::new(),
        };
        let folder = self.project.as_deref().and_then(Folder::project);
        if let Some(project) = &folder {
            entries.extend(self.scan(project, From::Project, &mut before));
        }
        entries.sort_by_key(|e| {
            std::cmp::Reverse(e.get("updated").and_then(Value::as_u64).unwrap_or(0))
        });
        let mut published = json!({
            "available": true,
            "agents": self.agents.to_string_lossy(),
            "project": folder.as_ref().map(|f| f.path().to_string_lossy().into_owned()),
            "diagrams": [],
        });
        published["diagrams"] = Value::Array(within_budget(entries, document::weight(&published)));
        if self.last.as_ref() != Some(&published) {
            host.publish_json(&published);
            self.last = Some(published);
        }
    }

    /// The diagrams in `folder`, each read again only when it changed.
    fn scan(
        &mut self,
        folder: &Folder,
        from: From,
        before: &mut HashMap<PathBuf, (SystemTime, u64, Value)>,
    ) -> Vec<Value> {
        let mut out = Vec::new();
        for name in folder.names(MOST_FILES) {
            let file = folder.path().join(&name);
            let Some(language) = language(&file) else {
                continue;
            };
            // Only a file of the folder's own, opened through it: a link
            // could lead anywhere (a key in the home folder) and its text
            // would be shared.
            let Ok((handle, meta)) = folder.open_file(&name) else {
                continue;
            };
            let modified = meta.modified().unwrap_or(UNIX_EPOCH);
            if let Some(known) = before.remove(&file)
                && known.0 == modified
                && known.1 == meta.len()
            {
                out.push(known.2.clone());
                self.read.insert(file, known);
                continue;
            }
            let entry = entry(handle, &file, language, from, &meta, modified);
            self.read
                .insert(file, (modified, meta.len(), entry.clone()));
            out.push(entry);
        }
        out
    }
}

/// `entries`, newest first, held to [`BUDGET`] with what is published
/// around them (`spent`): past it a diagram goes without its document,
/// and one that does not fit even so is left out.
fn within_budget(entries: Vec<Value>, mut spent: usize) -> Vec<Value> {
    let mut kept = Vec::with_capacity(entries.len());
    for mut entry in entries {
        let full = document::weight(&entry);
        if spent + full <= BUDGET {
            spent += full;
            kept.push(entry);
            continue;
        }
        if let Value::Object(map) = &mut entry {
            map.remove("document");
            map.insert("truncated".into(), true.into());
        }
        let lean = document::weight(&entry);
        if spent + lean <= BUDGET {
            spent += lean;
            kept.push(entry);
        }
    }
    kept
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
    handle: std::fs::File,
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
    // The whole name: `a.mmd` and `a.dot` are two diagrams.
    let name = file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("diagram");
    let seconds = modified
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut out = json!({
        "key": format!("{}/{name}", from.name()),
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
        let shown = read_whole(handle).and_then(|bytes| {
            let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            store::Shown::from_json(&value)
        });
        match shown {
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
    match read_whole(handle)
        .and_then(|b| String::from_utf8(b).map_err(|_| "not UTF-8 text".to_owned()))
    {
        Ok(text) => {
            match document::from_text(language, &text) {
                Ok(doc) => out["document"] = doc,
                Err(e) => out["error"] = e.into(),
            }
            out["language"] = language.into();
            out["text"] = text.into();
        },
        Err(e) => out["error"] = e.into(),
    }
    out
}

/// The id of the agent's diagram `key` names, if closing may remove it:
/// only the JSON files the MCP server writes go (`agent/<id>.json`), not
/// a `.mmd` or `.dot` put in the agents' folder, nor a project's file.
fn closable(key: &str) -> Option<String> {
    let id = key.strip_prefix("agent/")?.strip_suffix(".json")?;
    store::safe_id(id).ok()
}

/// What the opened file holds, up to [`store::MOST_BYTES`].
fn read_whole(handle: std::fs::File) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    handle
        .take(store::MOST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > store::MOST_BYTES {
        return Err(format!("larger than {} KiB", store::MOST_BYTES / 1024));
    }
    Ok(bytes)
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

    #[cfg(unix)]
    #[test]
    fn links_are_not_followed() {
        let project = tempfile::tempdir().unwrap();
        let secret = tempfile::tempdir().unwrap();
        std::fs::write(
            secret.path().join("key"),
            "flowchart TD\n secret --> leaked",
        )
        .unwrap();
        let dir = project.path().join(".moonframe").join("diagrams");
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(secret.path().join("key"), dir.join("key.mmd")).unwrap();
        std::fs::write(dir.join("own.mmd"), "graph TD; a-->b").unwrap();
        let mut provider = Diagrams::default();
        let folder = Folder::project(project.path()).unwrap();
        let found = provider.scan(&folder, From::Project, &mut HashMap::new());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0]["id"], "own");
        // The folder swapped for a link once it is open is not read
        // through: the scan stays in the folder it opened.
        std::fs::rename(&dir, project.path().join(".moonframe").join("moved")).unwrap();
        std::fs::write(secret.path().join("key.mmd"), "graph TD; secret-->leaked").unwrap();
        std::os::unix::fs::symlink(secret.path(), &dir).unwrap();
        let found = provider.scan(&folder, From::Project, &mut HashMap::new());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0]["id"], "own");
        assert!(Folder::project(project.path()).is_none());
        // A diagrams folder that is a link out of the project is not read.
        let linked = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(linked.path().join(".moonframe")).unwrap();
        std::os::unix::fs::symlink(
            secret.path(),
            linked.path().join(".moonframe").join("diagrams"),
        )
        .unwrap();
        assert!(Folder::project(linked.path()).is_none());

        // A folder that comes later as a link is refused when it comes,
        // as each scan checks it again.
        let later = tempfile::tempdir().unwrap();
        assert!(Folder::project(later.path()).is_none());
        std::fs::create_dir_all(later.path().join(".moonframe")).unwrap();
        std::os::unix::fs::symlink(
            secret.path(),
            later.path().join(".moonframe").join("diagrams"),
        )
        .unwrap();
        assert!(Folder::project(later.path()).is_none());
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
            Data::map([("close", Data::String("agent/flow.json".into()))]),
        );
        assert!(!store::path(dir.path(), "flow").exists());
    }

    #[test]
    fn closing_removes_only_the_file_listed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("arch.dot"), "digraph { a -> b }").unwrap();
        std::fs::write(dir.path().join("arch.json"), "{}").unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let host = handle(&seen, dir.path());
        let mut provider = Diagrams::default();
        provider.start(&host).unwrap();
        let close = |key: &str| Data::map([("close", Data::String(key.into()))]);
        // A text diagram is not closed, nor the JSON file of its name.
        provider.receive(&host, "editor", close("agent/arch.dot"));
        provider.receive(&host, "editor", close("arch"));
        provider.receive(&host, "editor", close("project/arch.json"));
        assert!(dir.path().join("arch.dot").exists());
        assert!(dir.path().join("arch.json").exists());
        provider.receive(&host, "editor", close("agent/arch.json"));
        assert!(!dir.path().join("arch.json").exists());
        assert_eq!(closable("agent/../x.json"), None);
    }

    #[test]
    fn same_named_files_keep_their_own_keys_and_gone_files_are_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("arch.mmd"), "graph TD; a-->b").unwrap();
        std::fs::write(dir.path().join("arch.dot"), "digraph { a -> b }").unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let host = handle(&seen, dir.path());
        let mut provider = Diagrams::default();
        provider.start(&host).unwrap();
        let last = seen.lock().unwrap().last().cloned().unwrap();
        let mut keys: Vec<&str> = last["diagrams"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["key"].as_str().unwrap())
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["agent/arch.dot", "agent/arch.mmd"]);
        assert_eq!(provider.read.len(), 2);
        std::fs::remove_file(dir.path().join("arch.dot")).unwrap();
        provider.tick(&host);
        assert_eq!(provider.read.len(), 1, "the gone file is not kept");
    }

    #[test]
    fn a_too_deep_diagram_is_an_error_and_the_others_still_go() {
        let dir = tempfile::tempdir().unwrap();
        let mut deep = json!("x");
        for _ in 0..20 {
            deep = json!([deep]);
        }
        let file = json!({ "id": "deep", "document": { "nodes": [{ "id": "a", "data": deep }] } });
        std::fs::write(dir.path().join("deep.json"), file.to_string()).unwrap();
        std::fs::write(dir.path().join("fine.mmd"), "graph TD; a-->b").unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let host = handle(&seen, dir.path());
        let mut provider = Diagrams::default();
        provider.start(&host).unwrap();
        let last = seen.lock().unwrap().last().cloned().expect("it published");
        assert!(moonframe_providers::from_json(&last).is_ok());
        let diagrams = last["diagrams"].as_array().unwrap();
        let deep = diagrams.iter().find(|d| d["id"] == "deep").unwrap();
        assert!(deep["error"].as_str().unwrap().contains("deeper"), "{deep}");
        assert!(
            diagrams
                .iter()
                .any(|d| d["id"] == "fine" && d.get("document").is_some())
        );
    }

    #[test]
    fn many_diagrams_stay_within_what_one_publish_holds() {
        let dir = tempfile::tempdir().unwrap();
        let nodes: String = (0..400)
            .map(|i| format!(" n{i} --> n{}\n", i + 1))
            .collect();
        for i in 0..40 {
            std::fs::write(
                dir.path().join(format!("d{i}.mmd")),
                format!("flowchart TD\n{nodes}"),
            )
            .unwrap();
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let host = handle(&seen, dir.path());
        let mut provider = Diagrams::default();
        provider.start(&host).unwrap();
        let last = seen.lock().unwrap().last().cloned().expect("it published");
        assert!(document::weight(&last) <= MAX_ENTRIES);
        let diagrams = last["diagrams"].as_array().unwrap();
        assert_eq!(diagrams.len(), 40, "all listed");
        assert!(diagrams.iter().any(|d| d.get("document").is_some()));
        assert!(diagrams.iter().any(|d| d["truncated"] == true));
        // Entries that do not fit even without their document are left out.
        let lean = json!({ "id": "x", "document": { "nodes": [] } });
        let kept = within_budget(vec![lean.clone(), lean], BUDGET - 3);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0]["truncated"], true);
    }
}
