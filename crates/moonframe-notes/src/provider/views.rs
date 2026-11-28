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
            // Where a wikilink written in `from` leads; `path` nil when nowhere
            // yet. `target` is the note's name alone (no `#heading`), the
            // name a missing note is made with.
            let target = text(query, "target");
            let path = index.resolve_path(&target, &text(query, "from"))?;
            json!({ "target": note_name(&target), "path": path })
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
            let mut paths = newest_days(vault, days + 1)?;
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

/// A wikilink target's note name: without its `#heading` or `#^block`.
fn note_name(target: &str) -> &str {
    target
        .split_once('#')
        .map_or(target, |(name, _)| name)
        .trim()
}

/// The paths of the `limit` newest daily notes (`journal/YYYY-MM-DD.md`),
/// newest first. Picked newest first before the limit, so a long journal
/// still shows its latest days.
fn newest_days(vault: &Vault, limit: usize) -> Result<Vec<String>, Error> {
    let mut statement = vault.index().db().prepare_cached(
        "SELECT path FROM notes
         WHERE path_key GLOB 'journal/[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
         ORDER BY path_key DESC, path DESC LIMIT ?1",
    )?;
    let rows = statement.query_map([limit as i64], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_resolved_target_is_the_notes_name_alone() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("notes")).unwrap();
        std::fs::write(temp.path().join("notes/A.md"), "# A\n").unwrap();
        let index = temp.path().with_extension("index.sqlite");
        let mut vault = Vault::open_with_index(temp.path(), &index).unwrap();
        vault.scan().unwrap();
        let resolve = |target: &str| {
            run(
                &vault,
                &json!({ "kind": "resolve", "target": target, "from": "notes/A.md" }),
                "2026-11-28",
            )
            .unwrap()
        };
        let missing = resolve("B#Heading");
        assert_eq!(missing["target"], "B");
        assert!(missing["path"].is_null(), "{missing}");
        assert_eq!(resolve(" B #^block")["target"], "B");
        let found = resolve("A#Heading");
        assert_eq!(
            (found["target"].as_str(), found["path"].as_str()),
            (Some("A"), Some("notes/A.md"))
        );
        assert_eq!(resolve("B")["target"], "B");
    }

    #[test]
    fn the_journal_shows_the_newest_days_of_a_long_journal() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("journal")).unwrap();
        for day in 1..=20 {
            let path = temp.path().join(format!("journal/2026-11-{day:02}.md"));
            std::fs::write(path, format!("day {day}\n")).unwrap();
        }
        // Not a day: never one of the journal's rows.
        std::fs::write(temp.path().join("journal/Ideas.md"), "x\n").unwrap();
        let index = temp.path().with_extension("index.sqlite");
        let mut vault = Vault::open_with_index(temp.path(), &index).unwrap();
        vault.scan().unwrap();
        let dates = |today: &str| -> Vec<String> {
            let answer = run(&vault, &json!({ "kind": "journal", "days": 3 }), today).unwrap();
            answer["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["date"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(dates("2026-11-28"), [
            "2026-11-28",
            "2026-11-20",
            "2026-11-19"
        ]);
        assert_eq!(dates("2026-11-20"), [
            "2026-11-20",
            "2026-11-19",
            "2026-11-18"
        ]);
    }
}
