//! Live queries: what a mod shows, asked once with `{ watch = { id, kind,
//! ... } }` and answered again each time the vault changes what it holds,
//! as `{ view = id, data }` sent to that mod alone. Only what changed is
//! sent again, and every answer is a page, so a view of a vault of any
//! size stays small.

use {
    crate::{Error, Order, Vault, digest},
    serde_json::{Value, json},
};

/// Largest note text sent to a view; bigger notes come without their text.
const MAX_TEXT: usize = 900 * 1024;

/// Most rows a view may ask for.
const MAX_ROWS: usize = 500;

/// One mod's live query.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct View {
    pub owner: String,
    pub id: String,
    pub query: Value,
    /// A digest of what was sent last, to send only what changed.
    pub sent: Option<String>,
}

fn text(query: &Value, key: &str) -> String {
    query
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn number(query: &Value, key: &str, default: usize) -> usize {
    query
        .get(key)
        .and_then(Value::as_u64)
        .map_or(default, |value| value as usize)
}

fn limit(query: &Value, default: usize) -> usize {
    number(query, "limit", default).clamp(1, MAX_ROWS)
}

/// The data for `query` now.
pub(super) fn run(vault: &Vault, query: &Value, today: &str) -> Result<Value, Error> {
    let index = vault.index();
    let kind = text(query, "kind");
    let offset = number(query, "offset", 0);
    Ok(match kind.as_str() {
        "list" => {
            let folder = text(query, "folder");
            let order = if text(query, "order") == "title" {
                Order::Title
            } else {
                Order::Modified
            };
            json!({
                "rows": index.list(&folder, order, offset, limit(query, 100))?,
                "total": index.count(&folder)?,
                "offset": offset,
            })
        },
        "search" => {
            let rows = index.search(&text(query, "q"), offset, limit(query, 50))?;
            json!({ "rows": rows, "offset": offset })
        },
        "switch" => {
            let rows: Vec<Value> = index
                .switch(&text(query, "q"), limit(query, 20))?
                .into_iter()
                .map(|(path, title, alias)| json!({ "path": path, "title": title, "alias": alias }))
                .collect();
            json!({ "rows": rows })
        },
        "note" => note(vault, &text(query, "path"))?,
        "resolve" => {
            // Where a wikilink written in `from` leads; `path` nil when nowhere yet.
            let target = text(query, "target");
            json!({ "target": target, "path": index.resolve_path(&target, &text(query, "from"))? })
        },
        "context" => {
            // Everything a side panel shows of one note, in one answer.
            let path = text(query, "path");
            json!({
                "path": path,
                "info": index.info(&path)?,
                "backlinks": index.backlinks(&path, 0, limit(query, 50))?,
                "mentions": index.mentions(&path, 10)?,
                "outgoing": index.outgoing(&path)?,
            })
        },
        "backlinks" => {
            json!({ "rows": index.backlinks(&text(query, "path"), offset, limit(query, 50))? })
        },
        "tags" => {
            json!({ "rows": index.tags()?.into_iter().take(limit(query, 100)).collect::<Vec<_>>() })
        },
        "tasks" => {
            json!({ "rows": index.tasks(query.get("done").and_then(Value::as_bool).unwrap_or(false), offset, limit(query, 100))? })
        },
        "health" => json!({
            "unresolved": index.unresolved(limit(query, 20))?,
            "orphans": index.orphans(&text(query, "folder"), limit(query, 20))?,
        }),
        "journal" => {
            // The newest daily notes, today's first even before it exists.
            let days = number(query, "days", 3).clamp(1, 31);
            let mut rows = Vec::new();
            let today_path = format!("journal/{today}.md");
            let listed = index.list("journal", Order::Title, 0, days + 1)?;
            let mut paths: Vec<String> = listed.into_iter().map(|row| row.path).collect();
            paths.sort_by(|a, b| b.cmp(a));
            if !paths.contains(&today_path) {
                paths.insert(0, today_path.clone());
            }
            for path in paths.into_iter().take(days) {
                let mut day = note(vault, &path)?;
                if let Some(object) = day.as_object_mut() {
                    object.insert("date".to_string(), json!(crate::stem(&path)));
                    object.insert("today".to_string(), json!(path == today_path));
                }
                rows.push(day);
            }
            json!({ "rows": rows })
        },
        other => return Err(Error::Invalid(format!("no view of kind {other:?}"))),
    })
}

/// A note's text and details; `exists = false` when there is no such file.
fn note(vault: &Vault, path: &str) -> Result<Value, Error> {
    match vault.read(path) {
        Ok(text) if text.len() > MAX_TEXT => {
            Ok(json!({ "path": path, "exists": true, "too_large": true, "hash": digest(&text) }))
        },
        Ok(text) => Ok(json!({
            "path": path,
            "exists": true,
            "title": crate::stem(path),
            "hash": digest(&text),
            "text": text,
            "info": vault.index().info(path)?,
            "favicons": super::enhance::favicons(&text),
        })),
        Err(Error::NotFound(_)) => {
            Ok(json!({ "path": path, "exists": false, "title": crate::stem(path), "text": "" }))
        },
        Err(error) => Err(error),
    }
}

/// A short digest of an answer, to tell whether it changed.
pub(super) fn fingerprint(value: &Value) -> String {
    digest(&value.to_string())[..16].to_string()
}
