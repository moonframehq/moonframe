//! What the provider changes in the vault for a mod: notes made, moved and
//! trashed, captures, the inbox's review, today's note, and imports. Every
//! change is a file write through the vault (journaled, merged).

use {
    super::ImportState,
    crate::{Error, Saved, Vault, import, write::atomic_write},
    serde_json::{Value, json},
    std::{
        fs,
        path::Path,
        sync::{Arc, Mutex},
    },
};

const INBOX: &str = "inbox.md";

/// The vault a new user starts with (VAULT.md, "Layout").
pub(super) fn starter(root: &Path) -> Result<(), Error> {
    let files: &[(&str, &str)] = &[
        ("AGENTS.md", include_str!("starter/AGENTS.md")),
        (
            "index.md",
            "# Index\n\nThe wiki's pages, kept by the agent.\n",
        ),
        ("log.md", "# Log\n\nWhat the agent did, newest last.\n"),
        (
            INBOX,
            "- Welcome to Commonplace: write anything here, newest at the top. Review it now and then.\n",
        ),
        ("notes/Welcome.md", include_str!("starter/Welcome.md")),
    ];
    for folder in ["journal", "notes", "raw/assets", "wiki"] {
        fs::create_dir_all(root.join(folder))?;
    }
    for (path, text) in files {
        let file = root.join(path);
        if !file.exists() {
            atomic_write(&file, text.as_bytes())?;
        }
    }
    Ok(())
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

pub(super) fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H%M").to_string()
}

/// Does what `message` asks; the answer goes back to the mod.
pub(super) fn act(vault: &mut Vault, message: &Value, today: &str) -> Result<Value, Error> {
    if let Some(create) = message.get("create") {
        let mut path = text(create, "path").to_string();
        // `unique`: "Untitled.md" taken becomes "Untitled 2.md", and so on.
        if create
            .get("unique")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let stem = crate::text::strip_md(&path).to_string();
            let mut number = 2;
            while vault.root().join(&path).exists() {
                path = format!("{stem} {number}.md");
                number += 1;
            }
        }
        let path = path.as_str();
        let written = vault.create(path, text(create, "text"))?;
        return Ok(json!({ "action": "create", "ok": true, "path": path, "text": written }));
    }
    if let Some(rename) = message.get("rename") {
        let renamed = vault.rename(text(rename, "from"), text(rename, "to"))?;
        return Ok(json!({ "action": "rename", "ok": true, "renamed": renamed }));
    }
    if let Some(path) = message.get("trash").and_then(Value::as_str) {
        vault.trash(path)?;
        return Ok(json!({ "action": "trash", "ok": true, "path": path }));
    }
    if let Some(capture) = message.get("capture") {
        let line = text(capture, "text").trim().replace('\n', " ");
        if line.is_empty() {
            return Err(Error::Invalid("nothing to capture".to_string()));
        }
        let path = if text(capture, "to") == "today" {
            format!("journal/{today}.md")
        } else {
            INBOX.to_string()
        };
        let current = read_or_empty(vault, &path)?;
        let updated = if path == INBOX {
            prepend(&current, &format!("- {line}"))
        } else {
            append(&current, &format!("- {line}"))
        };
        vault.write(&path, &updated, Some(&current), &stamp())?;
        return Ok(json!({ "action": "capture", "ok": true, "path": path }));
    }
    if let Some(review) = message.get("inbox") {
        return inbox(vault, review, today);
    }
    if message.get("today").is_some() {
        let path = format!("journal/{today}.md");
        if vault.read(&path).is_err() {
            vault.write(&path, "", None, &stamp())?;
        }
        return Ok(json!({ "action": "today", "ok": true, "path": path }));
    }
    Err(Error::Invalid(format!(
        "not something the notes provider does: {message}"
    )))
}

fn read_or_empty(vault: &Vault, path: &str) -> Result<String, Error> {
    match vault.read(path) {
        Ok(text) => Ok(text),
        Err(Error::NotFound(_)) => Ok(String::new()),
        Err(error) => Err(error),
    }
}

/// The byte where a note's body starts, after any frontmatter.
fn body_start(text: &str) -> usize {
    crate::parse::body_start(text)
}

/// `line` as the first line of the body.
fn prepend(text: &str, line: &str) -> String {
    let at = body_start(text);
    format!("{}{line}\n{}", &text[..at], &text[at..])
}

/// `line` as the last line.
fn append(text: &str, line: &str) -> String {
    if text.is_empty() {
        format!("{line}\n")
    } else if text.ends_with('\n') {
        format!("{text}{line}\n")
    } else {
        format!("{text}\n{line}\n")
    }
}

/// One list item of the inbox, by its line, with the lines under it.
struct Item {
    first: usize,
    /// Lines past the item (exclusive).
    end: usize,
    text: String,
}

fn item_at(lines: &[&str], line: usize) -> Option<Item> {
    let first = line.checked_sub(1)?;
    let head = lines.get(first)?;
    let indent = head.len() - head.trim_start().len();
    let content = head
        .trim_start()
        .strip_prefix("- ")
        .or_else(|| head.trim_start().strip_prefix("* "))?;
    let mut end = first + 1;
    while end < lines.len() {
        let next = lines[end];
        let next_indent = next.len() - next.trim_start().len();
        if next.trim().is_empty() || next_indent <= indent {
            break;
        }
        end += 1;
    }
    Some(Item {
        first,
        end,
        text: content.trim().to_string(),
    })
}

/// The inbox's review: `keep` moves an item to the top, `drop` removes it,
/// `task` moves it to today's note as an open task, `note` makes it a note
/// of its own. `text` must still be the item's, or the inbox changed since
/// and nothing is done.
fn inbox(vault: &mut Vault, review: &Value, today: &str) -> Result<Value, Error> {
    let op = text(review, "op");
    let line = review.get("line").and_then(Value::as_u64).unwrap_or(0) as usize;
    let current = read_or_empty(vault, INBOX)?;
    let lines: Vec<&str> = current.lines().collect();
    let Some(item) = item_at(&lines, line) else {
        return Err(Error::Invalid(format!("no inbox item on line {line}")));
    };
    let expected = text(review, "text").trim();
    if !expected.is_empty() && expected != item.text {
        return Err(Error::Invalid("the inbox changed; look again".to_string()));
    }
    let moved: Vec<&str> = lines[item.first..item.end].to_vec();
    let mut rest: Vec<&str> = lines[..item.first].to_vec();
    rest.extend_from_slice(&lines[item.end..]);
    let mut without = rest.join("\n");
    if current.ends_with('\n') && !without.is_empty() {
        without.push('\n');
    }
    let mut answer = json!({ "action": "inbox", "op": op, "ok": true });
    let updated = match op {
        "keep" => {
            let block = moved.join("\n");
            let at = body_start(&without);
            format!("{}{block}\n{}", &without[..at], &without[at..])
        },
        "drop" => without,
        "task" => {
            let path = format!("journal/{today}.md");
            let day = read_or_empty(vault, &path)?;
            vault.write(
                &path,
                &append(&day, &format!("- [ ] {}", item.text)),
                Some(&day),
                &stamp(),
            )?;
            answer["path"] = json!(path);
            without
        },
        "note" => {
            let title = import::safe_name(&item.text, "Untitled");
            let mut path = format!("notes/{title}.md");
            let mut number = 2;
            while vault.root().join(&path).exists() {
                path = format!("notes/{title} {number}.md");
                number += 1;
            }
            let body: Vec<String> = std::iter::once(item.text.clone())
                .chain(
                    moved[1..]
                        .iter()
                        .map(|line| line.trim().trim_start_matches("- ").to_string()),
                )
                .collect();
            vault.create(&path, &(body.join("\n") + "\n"))?;
            answer["path"] = json!(path);
            without
        },
        other => return Err(Error::Invalid(format!("no inbox review {other:?}"))),
    };
    if let Saved::Conflict { copy, .. } = vault.write(INBOX, &updated, Some(&current), &stamp())? {
        answer["conflict"] = json!(copy);
    }
    Ok(answer)
}

/// Imports Notes.app into the vault at `root`, telling `state` how far it is.
pub(super) fn import_apple_notes(
    root: &Path,
    into: &str,
    export: Option<&Path>,
    dry_run: bool,
    state: &Arc<Mutex<ImportState>>,
) -> Result<Value, Error> {
    if let Ok(mut state) = state.lock() {
        state.phase = "Reading Notes.app".to_string();
    }
    let report = import::apple_notes(root, into, export, dry_run, |step| {
        if let Ok(mut state) = state.lock() {
            let (phase, done, total) = match step {
                import::Progress::Reading { done, total, .. } => ("Reading Notes.app", done, total),
                import::Progress::Matching => ("Matching the export", 0, 0),
                import::Progress::Writing { done, total } => (
                    if dry_run {
                        "Checking"
                    } else {
                        "Writing notes"
                    },
                    done,
                    total,
                ),
            };
            state.phase = phase.to_string();
            state.done = done;
            state.total = total;
        }
    })?;
    // Lists are summed up; a long import names only the first few notes.
    let mut value = serde_json::to_value(&report).unwrap_or(Value::Null);
    if let Some(object) = value.as_object_mut() {
        for key in ["created", "updated", "kept"] {
            let count = report_len(&report, key);
            if let Some(list) = object.get_mut(key).and_then(Value::as_array_mut) {
                list.truncate(20);
            }
            object.insert(format!("{key}_count"), json!(count));
        }
        object.insert("skipped_count".to_string(), json!(report.skipped.len()));
        if let Some(list) = object.get_mut("skipped").and_then(Value::as_array_mut) {
            list.truncate(20);
        }
    }
    Ok(value)
}

fn report_len(report: &import::ImportReport, key: &str) -> usize {
    match key {
        "created" => report.created.len(),
        "updated" => report.updated.len(),
        _ => report.kept.len(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn vault() -> (tempfile::TempDir, Vault) {
        let temp = tempfile::tempdir().unwrap();
        starter(temp.path()).unwrap();
        let index = temp.path().with_extension("index.sqlite");
        let mut vault = Vault::open_with_index(temp.path(), &index).unwrap();
        vault.scan().unwrap();
        (temp, vault)
    }

    #[test]
    fn captures_go_to_the_top_of_the_inbox_or_the_end_of_today() {
        let (_temp, mut vault) = vault();
        act(
            &mut vault,
            &json!({ "capture": { "text": "first idea" } }),
            "2026-11-10",
        )
        .unwrap();
        act(
            &mut vault,
            &json!({ "capture": { "text": "second idea" } }),
            "2026-11-10",
        )
        .unwrap();
        let inbox = vault.read(INBOX).unwrap();
        assert!(
            inbox.starts_with("- second idea\n- first idea\n- Welcome"),
            "{inbox}"
        );
        act(
            &mut vault,
            &json!({ "capture": { "text": "met Ana", "to": "today" } }),
            "2026-11-10",
        )
        .unwrap();
        act(
            &mut vault,
            &json!({ "capture": { "text": "read", "to": "today" } }),
            "2026-11-10",
        )
        .unwrap();
        assert_eq!(
            vault.read("journal/2026-11-10.md").unwrap(),
            "- met Ana\n- read\n"
        );
    }

    #[test]
    fn inbox_review_moves_items() {
        let (_temp, mut vault) = vault();
        vault
            .write(INBOX, "- a\n- b\n  - under b\n- c\n", None, "s")
            .unwrap();
        act(
            &mut vault,
            &json!({ "inbox": { "op": "keep", "line": 4, "text": "c" } }),
            "2026-11-10",
        )
        .unwrap();
        assert_eq!(vault.read(INBOX).unwrap(), "- c\n- a\n- b\n  - under b\n");
        let answer = act(
            &mut vault,
            &json!({ "inbox": { "op": "note", "line": 3, "text": "b" } }),
            "2026-11-10",
        )
        .unwrap();
        assert_eq!(answer["path"], "notes/b.md");
        assert!(
            vault
                .read("notes/b.md")
                .unwrap()
                .ends_with("---\nb\nunder b\n")
        );
        assert_eq!(vault.read(INBOX).unwrap(), "- c\n- a\n");
        act(
            &mut vault,
            &json!({ "inbox": { "op": "task", "line": 2, "text": "a" } }),
            "2026-11-10",
        )
        .unwrap();
        assert_eq!(vault.read("journal/2026-11-10.md").unwrap(), "- [ ] a\n");
        act(
            &mut vault,
            &json!({ "inbox": { "op": "drop", "line": 1, "text": "c" } }),
            "2026-11-10",
        )
        .unwrap();
        assert_eq!(vault.read(INBOX).unwrap(), "");
        // A stale review does nothing.
        vault.write(INBOX, "- x\n", None, "s").unwrap();
        assert!(
            act(
                &mut vault,
                &json!({ "inbox": { "op": "drop", "line": 1, "text": "y" } }),
                "2026-11-10"
            )
            .is_err()
        );
    }

    #[test]
    fn the_starter_vault_has_its_layout() {
        let (temp, vault) = vault();
        for path in ["AGENTS.md", "index.md", "log.md", INBOX, "notes/Welcome.md"] {
            assert!(temp.path().join(path).is_file(), "{path}");
        }
        assert!(vault.index().count("").unwrap() >= 5);
    }
}
