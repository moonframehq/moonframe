//! Writes and `gh`, which may wait (on the network, on a key to sign with),
//! each on a thread; what they found is picked up at the next tick.

use {
    crate::{git::run, last_name, tilde},
    chrono::NaiveDateTime,
    serde_json::{Value, json},
    std::{
        path::Path,
        sync::{Arc, Mutex, PoisonError},
        thread,
    },
};

/// A job that finished.
pub(crate) enum Done {
    /// The write `name` (for the mod's `id`) came to this.
    Write {
        name: String,
        id: Option<Value>,
        result: Value,
    },
    /// What `gh` said of the open project.
    Forge(Value),
}

/// Finished jobs, waiting for the next tick.
#[derive(Clone, Default)]
pub(crate) struct Finished(Arc<Mutex<Vec<Done>>>);

impl Finished {
    pub(crate) fn take(&self) -> Vec<Done> {
        std::mem::take(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Runs `work` on a thread of its own.
    pub(crate) fn start(&self, work: impl FnOnce() -> Done + Send + 'static) {
        let finished = self.clone();
        thread::spawn(move || {
            let done = work();
            let mut list = finished.0.lock().unwrap_or_else(PoisonError::into_inner);
            list.push(done);
        });
    }
}

/// Stages everything in `dir` and commits it.
pub(crate) fn commit(dir: &str, message: &str) -> Value {
    let (ok, text) = run(dir, "git", &["add", "-A"]);
    if !ok {
        return failed(text, "git add failed");
    }
    let (ok, text) = run(dir, "git", &["commit", "-m", message]);
    if !ok {
        return failed(text, "git commit failed");
    }
    let first = text.lines().next().unwrap_or("Committed");
    json!({ "ok": true, "message": first })
}

fn failed(text: String, otherwise: &str) -> Value {
    let error = if text.is_empty() {
        otherwise.to_owned()
    } else {
        text
    };
    json!({ "ok": false, "error": error })
}

/// Pulls (fast-forward) or pushes `dir`, saying git's last word.
pub(crate) fn sync(dir: &str, what: &str) -> Value {
    let args: &[&str] = if what == "pull" {
        &["pull", "--ff-only"]
    } else {
        &["push", "-u", "origin", "HEAD"]
    };
    let (ok, text) = run(dir, "git", args);
    let last = text.lines().rfind(|line| !line.trim().is_empty());
    if !ok {
        return json!({ "ok": false, "error": last.map_or(format!("git {what} failed"), str::to_owned) });
    }
    let done = if what == "pull" {
        "Pulled"
    } else {
        "Pushed"
    };
    json!({ "ok": true, "message": last.unwrap_or(done) })
}

/// A new work tree of `repo` on a new branch `name`.
pub(crate) fn add_worktree(repo: &str, name: &str, home: &str) -> Value {
    let folder = last_name(repo).unwrap_or_default();
    let place = format!("{home}/.moonframe/worktrees/{folder}/{name}");
    if Path::new(&place).exists() {
        return json!({ "ok": false, "error": format!("{place} already exists") });
    }
    let (ok, text) = run(repo, "git", &["worktree", "add", "-b", name, &place]);
    if !ok {
        return failed(text, "git worktree add failed");
    }
    let message = format!("Work tree {name} at {}", tilde(&place, home));
    json!({ "ok": true, "message": message, "path": place })
}

/// An ISO time from `gh`, in Unix seconds, or 0.
fn stamp(iso: &Value) -> i64 {
    let text = iso.as_str().unwrap_or_default();
    let seconds = text.get(..19).unwrap_or(text);
    NaiveDateTime::parse_from_str(seconds, "%Y-%m-%dT%H:%M:%S")
        .map_or(0, |time| time.and_utc().timestamp())
}

/// The objects in `gh`'s JSON list.
fn items(text: &str) -> Vec<Value> {
    let list: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let list = list.as_array().cloned().unwrap_or_default();
    list.into_iter().filter(Value::is_object).collect()
}

fn word<'a>(item: &'a Value, key: &str) -> &'a str {
    item[key].as_str().unwrap_or_default()
}

fn pull_request(item: &Value) -> Value {
    json!({
        "number": item["number"].as_i64().unwrap_or(0),
        "title": word(item, "title"),
        "branch": word(item, "headRefName"),
        "author": word(&item["author"], "login"),
        "draft": item["isDraft"].as_bool().unwrap_or(false),
        "updated": stamp(&item["updatedAt"]),
    })
}

fn workflow_run(item: &Value) -> Value {
    json!({
        "id": item["databaseId"].as_i64().unwrap_or(0),
        "title": word(item, "displayTitle"),
        "workflow": word(item, "workflowName"),
        "branch": word(item, "headBranch"),
        "status": word(item, "status"),
        "conclusion": word(item, "conclusion"),
        "created": stamp(&item["createdAt"]),
    })
}

/// GitHub's pull requests and runs for `dir`, through `gh`.
pub(crate) fn ask_forge(dir: &str) -> Value {
    let prs = [
        "pr",
        "list",
        "--limit",
        "20",
        "--json",
        "number,title,headRefName,author,isDraft,updatedAt",
    ];
    let (ok, text) = run(dir, "gh", &prs);
    if !ok {
        let error = text.lines().next().unwrap_or("gh failed");
        return json!({ "path": dir, "prs": [], "runs": [], "error": error });
    }
    let prs: Vec<Value> = items(&text).iter().map(pull_request).collect();
    let runs = [
        "run",
        "list",
        "--limit",
        "15",
        "--json",
        "databaseId,displayTitle,workflowName,headBranch,status,conclusion,createdAt",
    ];
    let (ok, text) = run(dir, "gh", &runs);
    let runs: Vec<Value> = if ok {
        items(&text).iter().map(workflow_run).collect()
    } else {
        Vec::new()
    };
    json!({ "path": dir, "prs": prs, "runs": runs })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn gh_times_read_as_unix_seconds() {
        assert_eq!(stamp(&json!("2026-01-02T03:04:05Z")), 1_767_323_045);
        assert_eq!(stamp(&json!("not a time")), 0);
        assert_eq!(stamp(&Value::Null), 0);
    }

    #[test]
    fn gh_output_keeps_only_objects() {
        assert_eq!(items("[{\"a\":1},2]").len(), 1);
        assert!(items("oops").is_empty());
    }

    #[test]
    fn a_pull_request_with_nothing_known_reads_as_empty() {
        let row = pull_request(&json!({}));
        assert_eq!(row["number"], 0);
        assert_eq!(row["author"], "");
    }
}
