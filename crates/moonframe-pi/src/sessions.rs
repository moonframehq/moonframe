//! Reading pi's session files: JSONL entries in a tree by parentId, the last
//! entry being where the conversation stands. These files are the truth;
//! nothing here writes them.

use {
    crate::{
        Config,
        value::{clip, lock, squeezed, text_of},
    },
    chrono::DateTime,
    serde_json::{Value, json},
    std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        sync::Mutex,
        time::SystemTime,
    },
};

/// What a list needs to know of one session.
#[derive(Clone, Debug)]
pub(crate) struct Summary {
    pub(crate) id: String,
    pub(crate) file: String,
    pub(crate) updated: f64,
    pub(crate) name: Option<String>,
    pub(crate) prompt: Option<String>,
    pub(crate) preview: String,
    pub(crate) messages: u64,
    pub(crate) model: Option<Value>,
    pub(crate) thinking: Option<String>,
}

/// A file's entries, those that have finished being written.
pub(crate) fn parse(file: &Path) -> Vec<Value> {
    let Ok(bytes) = fs::read(file) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    text.split('\n')
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// The entries from the root to where the session stands, oldest first.
pub(crate) fn active_branch(entries: &[Value]) -> Vec<&Value> {
    let mut by_id: HashMap<&str, &Value> = HashMap::new();
    let mut leaf = None;
    for entry in entries {
        if let Some(id) = entry["id"].as_str() {
            by_id.insert(id, entry);
            leaf = Some(entry);
        }
    }
    let mut path = Vec::new();
    let mut at = leaf;
    while let Some(entry) = at {
        path.push(entry);
        if path.len() > entries.len() {
            break;
        }
        at = entry["parentId"]
            .as_str()
            .and_then(|id| by_id.get(id).copied());
    }
    path.reverse();
    path
}

/// One line to read in a list, without its markdown marks.
fn first_line(text: &str, length: usize) -> String {
    let line = text.trim().split('\n').next().unwrap_or_default();
    let plain: String = line.chars().filter(|c| !"*`_#".contains(*c)).collect();
    clip(&squeezed(&plain), length)
}

fn seconds(stamp: &Value) -> Option<f64> {
    let time = DateTime::parse_from_rfc3339(stamp.as_str()?).ok()?;
    #[allow(clippy::cast_precision_loss, reason = "milliseconds of a date fit")]
    let millis = time.timestamp_millis() as f64;
    Some(millis / 1000.0)
}

fn summarise(file: &Path, entries: &[Value]) -> Option<Summary> {
    let header = entries.iter().find(|entry| entry["type"] == "session")?;
    let mut summary = Summary {
        id: header["id"].as_str()?.to_owned(),
        file: file.to_string_lossy().into_owned(),
        updated: seconds(&header["timestamp"]).unwrap_or(0.0),
        name: None,
        prompt: None,
        preview: String::new(),
        messages: 0,
        model: None,
        thinking: None,
    };
    for entry in active_branch(entries) {
        if let Some(updated) = seconds(&entry["timestamp"]) {
            summary.updated = updated;
        }
        match entry["type"].as_str() {
            Some("session_info") => summary.name = entry["name"].as_str().map(str::to_owned),
            Some("model_change") => {
                summary.model =
                    Some(json!({ "provider": entry["provider"], "id": entry["modelId"] }));
            },
            Some("thinking_level_change") => {
                summary.thinking = entry["thinkingLevel"].as_str().map(str::to_owned);
            },
            Some("message") => read_message(&mut summary, &entry["message"]),
            _ => {},
        }
    }
    Some(summary)
}

fn read_message(summary: &mut Summary, message: &Value) {
    match message["role"].as_str() {
        Some("user") => {
            summary.messages += 1;
            if summary.prompt.is_none() {
                summary.prompt = Some(first_line(&text_of(&message["content"]), 80));
            }
        },
        Some("assistant") => {
            let text = text_of(&message["content"]);
            if !text.trim().is_empty() {
                summary.preview = first_line(&text, 90);
            }
            if let (Some("error"), Some(error)) = (
                message["stopReason"].as_str(),
                message["errorMessage"].as_str(),
            ) && !error.is_empty()
            {
                summary.preview = first_line(error, 90);
            }
            if message["provider"].is_string() && message["model"].is_string() {
                summary.model =
                    Some(json!({ "provider": message["provider"], "id": message["model"] }));
            }
        },
        _ => {},
    }
}

#[derive(Default)]
struct Cached {
    stamp: Option<(SystemTime, u64)>,
    summary: Option<Summary>,
}

/// Summaries of session files, read again only when a file changed.
#[derive(Default)]
pub(crate) struct Sessions {
    cache: Mutex<HashMap<PathBuf, Cached>>,
}

impl Sessions {
    /// A file's summary, or `None` when it is gone or no session.
    pub(crate) fn summary(&self, file: &Path) -> Option<Summary> {
        let Ok(meta) = fs::metadata(file) else {
            lock(&self.cache).remove(file);
            return None;
        };
        let stamp = meta.modified().ok().map(|time| (time, meta.len()));
        if let Some(cached) = lock(&self.cache).get(file)
            && stamp.is_some()
            && cached.stamp == stamp
        {
            return cached.summary.clone();
        }
        let summary = summarise(file, &parse(file));
        let cached = Cached {
            stamp,
            summary: summary.clone(),
        };
        lock(&self.cache).insert(file.to_owned(), cached);
        summary
    }

    /// The sessions pi keeps for `cwd`.
    pub(crate) fn sessions_in(&self, config: &Config, cwd: &str) -> Vec<Summary> {
        let Ok(names) = fs::read_dir(config.session_dir(cwd)) else {
            return Vec::new();
        };
        names
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ending| ending == "jsonl"))
            .filter_map(|path| self.summary(&path))
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn entries(lines: &[Value]) -> Vec<Value> {
        lines.to_vec()
    }

    #[test]
    fn the_active_branch_follows_parents_from_the_last_entry() {
        let list = entries(&[
            json!({ "type": "session", "id": "s" }),
            json!({ "id": "a", "parentId": null }),
            json!({ "id": "b", "parentId": "a" }),
            json!({ "id": "c", "parentId": "a" }),
        ]);
        let ids: Vec<&str> = active_branch(&list)
            .iter()
            .map(|entry| entry["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["a", "c"]);
    }

    #[test]
    fn a_cycle_ends() {
        let list = entries(&[
            json!({ "id": "a", "parentId": "b" }),
            json!({ "id": "b", "parentId": "a" }),
        ]);
        assert!(active_branch(&list).len() <= 3);
    }

    #[test]
    fn a_half_written_line_is_left_for_later() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.jsonl");
        fs::write(&file, "{\"id\":\"a\"}\n{\"id\":").unwrap();
        assert_eq!(parse(&file).len(), 1);
    }

    #[test]
    fn a_summary_reads_the_branch_and_follows_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.jsonl");
        let header = json!({ "type": "session", "id": "s1", "timestamp": "2026-01-01T00:00:00.000Z", "cwd": "/c" });
        let user = json!({ "type": "message", "id": "m1", "parentId": null, "timestamp": "2026-01-01T00:00:01.500Z",
            "message": { "role": "user", "content": "**Fix** the\nprice" } });
        let answer = json!({ "type": "message", "id": "m2", "parentId": "m1",
            "message": { "role": "assistant", "provider": "p", "model": "m",
                "content": [{ "type": "text", "text": "Done `now`" }] } });
        fs::write(&file, format!("{header}\n{user}\n{answer}\n")).unwrap();
        let sessions = Sessions::default();
        let summary = sessions.summary(&file).unwrap();
        assert_eq!(summary.id, "s1");
        assert_eq!(summary.prompt.as_deref(), Some("Fix the"));
        assert_eq!(summary.preview, "Done now");
        assert_eq!(summary.messages, 1);
        assert_eq!(summary.updated, 1_767_225_601.5);
        assert_eq!(summary.model.unwrap()["id"], "m");

        let named =
            json!({ "type": "session_info", "id": "m3", "parentId": "m2", "name": "Named" });
        fs::write(&file, format!("{header}\n{user}\n{answer}\n{named}\n")).unwrap();
        assert_eq!(
            sessions.summary(&file).unwrap().name.as_deref(),
            Some("Named")
        );
        fs::remove_file(&file).unwrap();
        assert!(sessions.summary(&file).is_none());
    }
}
