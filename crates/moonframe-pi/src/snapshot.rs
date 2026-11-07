//! `app.pi`: everything the window draws, in one small plain value. A long
//! list stops short and a long conversation shows its end, with more on
//! request, to stay under what the host takes.

use {
    crate::{
        engine::{Engine, PiStatus},
        limits::{fits, trimmed},
        live::Live,
        resources::{list_extensions, list_providers, list_skills},
        runtime::Runtime,
        schedule::describe,
        store::Meta,
        timeline::{Context, context_use, timeline, total_cost},
        value::{as_list, display, first_chars, now_seconds},
    },
    chrono::{Local, TimeZone},
    serde_json::{Map, Value, json},
};

const NO_PROJECT: &str = "No project";
const MAX_THREADS: usize = 120;
const MAX_MODELS: usize = 160;
const MAX_COMMANDS: usize = 80;
pub(crate) const SHOWN_ITEMS: usize = 160;
const TEXT_BUDGET: usize = 500_000;
const LONGEST_TEXT: usize = 100_000;

/// pi's own configuration, read when asked or when it changes.
#[derive(Clone, Debug, Default)]
pub(crate) struct Found {
    skills: Vec<Value>,
    extensions: Vec<Value>,
    providers: Value,
    error: Option<String>,
}

fn workspace_name(engine: &Engine, path: &str) -> String {
    if path == engine.chats {
        return NO_PROJECT.to_owned();
    }
    let known = engine
        .data()
        .workspaces
        .iter()
        .find(|item| item.path == path);
    known.map_or_else(|| crate::config::base_name(path), |item| item.name.clone())
}

fn thread_status(live: Option<&Live>, meta: &Meta) -> &'static str {
    match live {
        Some(live) if !live.dialogs.is_empty() => "waiting",
        Some(live) if live.running || live.sending.is_some() => "running",
        _ if meta.failed => "failed",
        _ => "idle",
    }
}

fn live_line(engine: &Engine, id: &str) -> Option<String> {
    engine.live.get(id).filter(|live| live.running)?;
    let list = engine.entries.get(id).map_or(&[][..], Vec::as_slice);
    let messages = list.iter().rev().filter_map(|entry| entry.message.as_ref());
    for message in messages {
        let blocks = as_list(&message["content"]).iter().rev();
        if let Some(block) = blocks.into_iter().find(|block| block["type"] == "toolCall") {
            let args = &block["arguments"];
            let target = if args["path"].is_null() {
                &args["command"]
            } else {
                &args["path"]
            };
            let line = format!("{} {}", display(&block["name"]), display(target));
            return Some(first_chars(line.trim(), 80).to_owned());
        }
    }
    Some("Working…".to_owned())
}

#[allow(clippy::cast_possible_truncation, reason = "whole seconds of a date")]
fn rounded(seconds: f64) -> i64 {
    seconds.round() as i64
}

fn thread_rows(engine: &Engine) -> Vec<Value> {
    let selected = engine.selected();
    let mut rows = Vec::new();
    for meta in engine.data().threads.values() {
        let id = meta.id.as_str();
        let info = engine.summaries.get(id);
        let status = thread_status(engine.live.get(id), meta);
        // A thread with only `!` output has no file yet, so no summary.
        let said = engine.entries.get(id).is_some_and(|list| !list.is_empty());
        if info.is_none() && !said && status == "idle" && Some(id) != selected {
            continue;
        }
        let updated = info
            .map(|info| info.updated)
            .or(meta.created)
            .unwrap_or(0.0);
        let preview = info.map(|info| info.preview.clone()).unwrap_or_default();
        rows.push(json!({
            "id": id,
            "title": engine.title_of(id),
            "workspace": workspace_name(engine, &meta.workspace),
            "path": meta.workspace,
            "branch": meta.worktree.as_ref().map(|worktree| &worktree.branch),
            "status": status,
            "unread": meta.unread,
            "pinned": meta.pinned,
            "archived": meta.archived,
            "updated": rounded(updated),
            "preview": live_line(engine, id).unwrap_or(preview),
        }));
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row["updated"].as_i64().unwrap_or(0)));
    rows.truncate(MAX_THREADS);
    rows
}

fn model_of(engine: &Engine, meta: &Meta, live: Option<&Live>) -> Value {
    let state = live.and_then(|live| live.state.as_ref());
    let current = state
        .map(|state| &state["model"])
        .filter(|model| model.is_object());
    let summary = engine
        .summaries
        .get(&meta.id)
        .and_then(|info| info.model.as_ref());
    let last = engine
        .data()
        .prefs
        .get("last_model")
        .filter(|model| model.is_object());
    let chosen = meta.model.as_ref().filter(|model| model.is_object());
    let Some(known) = current.or(chosen).or(summary).or(last) else {
        return Value::Null;
    };
    let same =
        |model: &&Value| model["provider"] == known["provider"] && model["id"] == known["id"];
    let full = engine.models.iter().find(same);
    let either = |key: &str| {
        let own = &known[key];
        if own.is_null() {
            full.map_or(&Value::Null, |full| &full[key])
        } else {
            own
        }
    };
    let name = if either("name").is_null() {
        &known["id"]
    } else {
        either("name")
    };
    let window = either("contextWindow");
    let reasoning = either("reasoning").as_bool().unwrap_or(false);
    let images = as_list(either("input")).iter().any(|kind| kind == "image");
    json!({
        "provider": known["provider"],
        "id": known["id"],
        "name": name,
        "window": window,
        "reasoning": reasoning,
        "images": images,
    })
}

fn text_size(item: &Map<String, Value>) -> usize {
    let size = |key: &str| item.get(key).and_then(Value::as_str).map_or(0, str::len);
    size("text") + size("output") + size("patch")
}

/// The end of the conversation, as much as fits, whole items only.
fn visible_items(items: Vec<Map<String, Value>>, limit: usize) -> (usize, Vec<Value>) {
    let mut text = 0;
    let mut first = items.len();
    while first > 0 && items.len() - first < limit {
        text += text_size(&items[first - 1]);
        if text > TEXT_BUDGET && first < items.len() {
            break;
        }
        first -= 1;
    }
    let kept = items.into_iter().skip(first).map(|mut item| {
        // Tool arguments are for the titles; the window has no use for them.
        item.remove("args");
        Value::Object(item)
    });
    (first, kept.collect())
}

fn thread_detail(engine: &Engine, id: &str, now: f64) -> Value {
    let Some(meta) = engine.thread(id) else {
        return Value::Null;
    };
    let live = engine.live.get(id);
    let entries = engine.entries.get(id).map_or(&[][..], Vec::as_slice);
    let context = Context {
        cwd: &meta.cwd,
        live,
        turns: &meta.turns,
        expanded: engine.expanded.get(id),
        now,
    };
    let mut items = timeline(entries, &context);
    if let Some(sending) = live.and_then(|live| live.sending.as_ref()) {
        let row = json!({ "k": "user", "id": "sending", "text": sending });
        items.extend(row.as_object().cloned());
    }
    let limit = engine.shown.get(id).copied().unwrap_or(SHOWN_ITEMS);
    let model = model_of(engine, meta, live);
    let branch = engine.branches.get(&meta.cwd);
    let summary = engine.summaries.get(id);
    let state = live.and_then(|live| live.state.as_ref());
    let thinking = state
        .and_then(|state| state["thinkingLevel"].as_str())
        .or(meta.thinking.as_deref())
        .or(summary.and_then(|info| info.thinking.as_deref()))
        .or(engine
            .data()
            .prefs
            .get("last_thinking")
            .and_then(Value::as_str))
        .unwrap_or("medium");
    let (earlier, items) = visible_items(items, limit);
    let updated = summary
        .map(|info| info.updated)
        .or(meta.created)
        .unwrap_or(now);
    let widgets: Vec<Value> = live
        .into_iter()
        .flat_map(|live| live.widgets.values())
        .flat_map(|lines| match lines {
            Value::Array(lines) => lines.clone(),
            one => vec![one.clone()],
        })
        .take(20)
        .collect();
    let dialog = live
        .and_then(|live| live.dialogs.first())
        .map(|dialog| &dialog.shown);
    let git = meta.workspace != engine.chats && branch.is_none_or(|branch| branch.git);
    json!({
        "id": id,
        "title": engine.title_of(id),
        "workspace": workspace_name(engine, &meta.workspace),
        "path": meta.workspace,
        "cwd": meta.cwd,
        "branch": meta.worktree.as_ref().map(|worktree| worktree.branch.clone())
            .or_else(|| branch.and_then(|branch| branch.branch.clone())),
        "worktree": meta.worktree.is_some(),
        "git": git,
        "status": thread_status(live, meta),
        "updated": rounded(updated),
        "ready": state.is_some(),
        "running": live.is_some_and(|live| live.running),
        "compacting": live.is_some_and(|live| live.compacting),
        "pinned": meta.pinned,
        "model": model,
        "thinking": thinking,
        "levels": live.map_or(&[][..], |live| live.levels.as_slice()),
        "context": context_use(entries, model["window"].as_f64()),
        "cost": total_cost(entries),
        "retry": live.and_then(|live| live.retry.as_ref()),
        "queued": live.map_or_else(|| json!({ "steering": [], "followUp": [] }), |live| live.queued.clone()),
        "dialog": dialog,
        "statuses": live.map_or_else(Vec::new, |live| live.statuses.values().cloned().collect()),
        "widgets": widgets,
        "editor_text": live.and_then(|live| live.editor_text.as_ref()),
        "earlier": earlier,
        "items": items,
    })
}

fn shortened(item: &Value) -> Value {
    let Value::Object(fields) = item else {
        return item.clone();
    };
    let cut = fields.iter().map(|(key, value)| {
        let value = match value {
            Value::String(text) if text.chars().count() > LONGEST_TEXT => {
                let kept = first_chars(text, LONGEST_TEXT);
                json!(format!("{kept}\n… cut, too long to show"))
            },
            other => other.clone(),
        };
        (key.clone(), value)
    });
    Value::Object(cut.collect())
}

fn line_of(value: &Value) -> String {
    value.to_string()
}

/// The line for a snapshot, one the host takes, and the value it says: what
/// it can't hold of the conversation goes, the oldest first, as older
/// entries do when they scroll off.
pub(crate) fn encode(mut snapshot: Value) -> (String, Value) {
    let mut line = line_of(&snapshot);
    if snapshot["thread"].is_object() {
        let thread_len = |snapshot: &Value| as_list(&snapshot["thread"]["items"]).len();
        while !fits(&line, &snapshot) && thread_len(&snapshot) > 1 {
            let dropped = thread_len(&snapshot).div_ceil(4);
            let thread = &mut snapshot["thread"];
            let kept = as_list(&thread["items"])[dropped..].to_vec();
            thread["items"] = Value::Array(kept);
            let earlier = thread["earlier"].as_u64().unwrap_or(0);
            thread["earlier"] = json!(earlier + dropped as u64);
            line = line_of(&snapshot);
        }
        if !fits(&line, &snapshot) {
            let cut: Vec<Value> = as_list(&snapshot["thread"]["items"])
                .iter()
                .map(shortened)
                .collect();
            snapshot["thread"]["items"] = Value::Array(cut);
            line = line_of(&snapshot);
        }
        if !fits(&line, &snapshot) {
            let count = thread_len(&snapshot) as u64;
            let thread = &mut snapshot["thread"];
            let earlier = thread["earlier"].as_u64().unwrap_or(0);
            thread["earlier"] = json!(earlier + count);
            thread["items"] = json!([]);
            line = line_of(&snapshot);
        }
    }
    for limit in [50, 10, 2] {
        if fits(&line, &snapshot) {
            break;
        }
        snapshot = trimmed(&snapshot, limit, 0);
        line = line_of(&snapshot);
    }
    (line, snapshot)
}

fn schedules(engine: &Engine) -> Vec<Value> {
    let tasks = engine.data().schedules.iter();
    tasks
        .map(|task| {
            let field = |key: &str| task.get(key).cloned().unwrap_or(Value::Null);
            let workspace = task
                .get("workspace")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let name = if workspace.is_empty() {
                String::new()
            } else {
                workspace_name(engine, workspace)
            };
            let seconds = |key: &str| {
                let value = task.get(key).and_then(Value::as_f64);
                value.filter(|value| *value != 0.0).map(rounded)
            };
            json!({
                "id": field("id"),
                "name": field("name"),
                "prompt": field("prompt"),
                "workspace": field("workspace"),
                "workspace_name": name,
                "cadence": field("cadence"),
                "time": field("time"),
                "weekday": task.get("weekday").cloned().unwrap_or(json!(1)),
                "enabled": field("enabled"),
                "worktree": task.get("worktree") == Some(&json!(true)),
                "next": seconds("next"),
                "last_run": seconds("last_run"),
                "last_thread": field("last_thread"),
                "when": describe(task),
            })
        })
        .collect()
}

fn commands_for(engine: &Engine, selected: Option<&str>) -> Vec<Value> {
    let live = selected.and_then(|id| engine.live.get(id));
    let own = live
        .map(|live| &live.commands)
        .filter(|list| !list.is_empty());
    let list = own.unwrap_or(&engine.commands);
    list.iter()
        .take(MAX_COMMANDS)
        .map(|command| {
            json!({
                "name": command["name"],
                "description": first_chars(&display(&command["description"]), 120),
                "source": command["source"],
            })
        })
        .collect()
}

/// Reading skills means touching the disk, so it happens when asked for or
/// when pi's configuration changes, not on each streamed word.
fn resources(engine: &mut Engine, runtime: &Runtime) -> Found {
    let selected = engine.selected().unwrap_or("undefined").to_owned();
    let stamp = format!("{}|{}|{selected}", engine.stamp, engine.resources_asked);
    if let Some((held, found)) = &engine.resources
        && *held == stamp
    {
        return found.clone();
    }
    let selected_cwd = engine.thread(&selected).map(|meta| meta.cwd.clone());
    let first = engine
        .data()
        .workspaces
        .first()
        .map(|item| item.path.clone());
    let cwd = selected_cwd.or(first);
    let config = std::sync::Arc::clone(&engine.config);
    let read = || -> Result<Found, String> {
        Ok(Found {
            skills: list_skills(&config, cwd.as_deref())?,
            extensions: list_extensions(&config, cwd.as_deref())?,
            providers: list_providers(&config, runtime)?,
            error: None,
        })
    };
    let found = read().unwrap_or_else(|error| Found {
        providers: json!({ "known": [], "endpoints": [] }),
        error: Some(error),
        ..Found::default()
    });
    engine.resources = Some((stamp, found.clone()));
    found
}

/// A short mark that changes when an item does, so a view can keep what it
/// built for all the others while one streams.
fn item_version(item: &Value) -> String {
    let length = |key: &str| {
        item[key]
            .as_str()
            .map_or(0, |text| text.encode_utf16().count())
    };
    format!(
        "{}.{}.{}.{}.{}.{}.{}.{}",
        length("text"),
        display(&item["status"]),
        display(&item["open"]),
        length("output"),
        display(&item["label"]),
        display(&item["summary"]),
        display(&item["count"]),
        display(&item["streaming"]),
    )
}

fn start_of_day() -> i64 {
    let today = Local::now().date_naive().and_hms_opt(0, 0, 0);
    let midnight = today.and_then(|day| Local.from_local_datetime(&day).earliest());
    midnight.map_or(0, |day| day.timestamp())
}

/// Rows change far less often than a conversation streams.
fn versioned(engine: &mut Engine, rows: &[Value]) -> u64 {
    let text = json!(rows).to_string();
    if text != engine.rows_text {
        engine.rows_text = text;
        engine.rows_version += 1;
    }
    engine.rows_version
}

/// What to publish, or `None` while pi is still being looked for.
pub(crate) fn snapshot_of(engine: &mut Engine, runtime: &Runtime) -> Option<Value> {
    // With no pi the shape stays: views read `prefs` and the lists before
    // they get to `available`.
    let (runtime_view, reason) = match &engine.pi {
        PiStatus::Looking => return None,
        PiStatus::Missing { reason } => (
            json!({ "status": "missing", "path": null, "version": null, "error": null }),
            Some(reason.clone()),
        ),
        PiStatus::Ready { path, version } => {
            let short = engine.config.short_path(&path.to_string_lossy());
            let view =
                json!({ "status": "ready", "path": short, "version": version, "error": null });
            (view, None)
        },
    };
    let now_float = now_seconds();
    let now = now_float.floor();
    let selected = engine.selected().map(str::to_owned);
    let found = resources(engine, runtime);
    let threads = thread_rows(engine);
    let mut thread = selected
        .as_ref()
        .map_or(Value::Null, |id| thread_detail(engine, id, now));
    if let Some(Value::Array(items)) = thread.get_mut("items") {
        for item in items {
            item["v"] = json!(item_version(item));
        }
    }
    let threads_version = versioned(engine, &threads);
    let config = std::sync::Arc::clone(&engine.config);
    let workspaces: Vec<Value> = engine
        .data()
        .workspaces
        .iter()
        .map(|workspace| {
            let mut row = serde_json::to_value(workspace).unwrap_or_default();
            let branch = engine.branches.get(&workspace.path);
            row["short"] = json!(config.short_path(&workspace.path));
            row["branch"] = json!(branch.and_then(|branch| branch.branch.as_ref()));
            row["git"] = json!(branch.is_some_and(|branch| branch.git));
            row
        })
        .collect();
    let models: Vec<Value> = engine
        .models
        .iter()
        .take(MAX_MODELS)
        .map(|model| {
            json!({
                "provider": model["provider"],
                "id": model["id"],
                "name": model["name"],
                "reasoning": model["reasoning"] == true,
            })
        })
        .collect();
    let returned = selected.as_ref().and_then(|id| engine.returned.get(id));
    Some(json!({
        "available": reason.is_none(),
        "reason": reason,
        "now": rounded(now),
        "day_start": start_of_day(),
        "threads_version": threads_version,
        "runtime": runtime_view,
        "prefs": engine.data().prefs,
        "chats": {
            "path": engine.chats,
            "name": NO_PROJECT,
            "short": config.short_path(&engine.chats),
            "git": false,
        },
        "workspaces": workspaces,
        "threads": threads,
        "selected": selected,
        "thread": thread,
        "prefill": engine.prefill,
        "returned": returned,
        "models": models,
        "commands": commands_for(engine, selected.as_deref()),
        "skills": found.skills,
        "extensions": found.extensions,
        "providers": found.providers,
        "config_error": found.error,
        "schedules": schedules(engine),
        "events": engine.events,
    }))
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
