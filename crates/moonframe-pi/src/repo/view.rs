//! `app.repo` as a value the host takes.

use {
    super::Repo,
    crate::limits::{MAX_BYTES, count_values, fits, trimmed},
    serde_json::{Value, json},
};

impl Repo {
    pub(super) fn view(&self) -> Value {
        json!({
            "available": true,
            "at": self.at,
            "git": self.is_git,
            "branch": self.branch,
            "head": self.head,
            "base": self.base,
            "base_name": self.base_name,
            "files": self.files,
            "more_files": self.more_files,
            "compare": {
                "mode": self.compare.mode,
                "from": self.compare.from,
                "to": self.compare.to,
            },
            "picked": self.picked,
            "diff": self.diff,
            "open": self.open,
            "tree": self.tree,
            "file": self.file,
            "found": self.found,
            "worktrees": self.worktrees,
            "busy": self.busy,
            "error": self.error,
            "message": self.message,
        })
    }
}

/// One step of giving something up.
type Shed = fn(&mut Value);

fn shed_file(view: &mut Value) {
    if view["file"]["text"].is_string() {
        view["file"] = json!({ "path": view["file"]["path"], "tooLarge": true });
    }
}

fn shed_diff(view: &mut Value) {
    if view["diff"].is_object() {
        view["diff"]["hunks"] = json!([]);
        view["diff"]["truncated"] = json!(true);
    }
}

fn shed_tree(view: &mut Value) {
    let root = view["tree"].get("").cloned().unwrap_or_else(|| json!([]));
    view["tree"] = json!({ "": root });
    view["open"] = json!([""]);
}

fn shed_files(view: &mut Value) {
    let Value::Array(files) = &mut view["files"] else {
        return;
    };
    let cut = files.len().saturating_sub(100);
    files.truncate(100);
    let more = view["more_files"].as_u64().unwrap_or(0);
    view["more_files"] = json!(more + cut as u64);
}

/// What to give up, in order, until the line is one the host takes.
const SHEDS: [Shed; 4] = [shed_file, shed_diff, shed_tree, shed_files];

/// The line for `view`, and the value it says.
pub(super) fn encoded(mut view: Value) -> (String, Value) {
    let mut line = view.to_string();
    for (index, shed) in SHEDS.iter().enumerate() {
        let bytes = line.len() > MAX_BYTES;
        if fits(&line, &view) {
            break;
        }
        // One long file is a few values: it only goes for its bytes.
        if index == 0 && !bytes {
            continue;
        }
        shed(&mut view);
        line = view.to_string();
    }
    for limit in [50, 10, 2] {
        if line.len() <= MAX_BYTES && count_values(&view) <= crate::limits::MAX_VALUES {
            break;
        }
        view = trimmed(&view, limit, 0);
        line = view.to_string();
    }
    (line, view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_file_goes_first() {
        let text = "\"\n".repeat(400_000);
        let (line, view) = encoded(json!({ "file": { "path": "a", "text": text }, "tree": {} }));
        assert!(line.len() <= MAX_BYTES);
        assert_eq!(view["file"]["tooLarge"], true);
    }

    #[test]
    fn many_values_keep_the_file_and_lose_the_rest() {
        let files: Vec<Value> = (0..4000)
            .map(|at| json!({ "path": at, "added": 0 }))
            .collect();
        let view = json!({
            "file": { "path": "a", "text": "kept" },
            "diff": { "hunks": [] },
            "files": files,
            "tree": { "": [] },
        });
        let (_, view) = encoded(view);
        assert_eq!(view["file"]["text"], "kept");
        assert_eq!(view["files"].as_array().map(Vec::len), Some(100));
        assert_eq!(view["more_files"], 3900);
    }
}
