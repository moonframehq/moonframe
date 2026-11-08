//! The planner's file: `{ items, tags, folders, next, look? }`, as the Python
//! provider wrote it (pretty, sorted, three spaces, `"key" : value`), so a
//! file either wrote reads back the same.

use {
    crate::dates::day,
    chrono::NaiveDate,
    serde_json::{Map, Value, json},
    std::{fs, io, path::Path},
};

/// What a new planner starts with: nothing yet.
pub(crate) fn empty() -> Map<String, Value> {
    let store = json!({ "tags": [], "folders": [], "items": [], "next": 1 });
    store.as_object().cloned().unwrap_or_default()
}

/// A planner to look at instead (`DAYBOOK_SAMPLE`): a day of tasks and
/// events around today, an inbox, tags, folders and a few notes.
pub(crate) fn sample(today: NaiveDate) -> Map<String, Value> {
    let today_text = day(today, None, 0);
    let near = |offset| day(today, Some(&today_text), offset);
    let mut items: Vec<Value> = Vec::new();
    let mut item = |mut fields: Value| {
        fields["id"] = json!(format!("i{}", items.len() + 1));
        items.push(fields);
    };
    item(json!({"kind": "task", "title": "Water the plants", "date": today_text, "done": true}));
    item(
        json!({"kind": "task", "title": "Finish the quarterly review", "date": today_text,
        "time": "17:00", "state": "progress", "tags": ["Work"]}),
    );
    item(
        json!({"kind": "task", "title": "Call Sam about the offsite", "date": today_text,
        "time": "16:00", "state": "important", "tags": ["Work"]}),
    );
    item(
        json!({"kind": "task", "title": "Morning run", "date": today_text, "time": "07:30",
        "habit": true, "tags": ["Health"]}),
    );
    item(json!({"kind": "event", "title": "Dinner with Lea", "date": today_text, "time": "19:00"}));
    item(
        json!({"kind": "event", "title": "Design critique", "date": today_text, "time": "10:00",
        "tags": ["Work"]}),
    );
    item(
        json!({"kind": "task", "title": "Book the train to Porto", "date": near(-1),
        "state": "important", "tags": ["Home"]}),
    );
    item(json!({"kind": "task", "title": "Sketch the onboarding flow", "tags": ["Ideas"]}));
    item(json!({"kind": "task", "title": "Read the research notes"}));
    item(
        json!({"kind": "event", "title": "Team standup", "date": near(1), "time": "09:30",
        "tags": ["Work"]}),
    );
    item(json!({"kind": "task", "title": "Renew the passport", "date": near(3), "tags": ["Home"]}));
    item(
        json!({"kind": "event", "title": "Climbing", "date": near(4), "time": "18:30",
        "tags": ["Health"]}),
    );
    item(
        json!({"kind": "note", "title": "Getting started", "emoji": "\u{2b50}", "favorite": true}),
    );
    item(json!({"kind": "note", "title": "Trip to Porto", "emoji": "\u{1f9f3}"}));
    item(json!({"kind": "note", "title": "Launch plan", "emoji": "\u{1f680}"}));
    item(json!({"kind": "note", "title": "App ideas", "emoji": "\u{1f4a1}"}));
    let next = items.len() + 1;
    let store = json!({
        "tags": [
            {"name": "Work", "color": "#3b6fd8"},
            {"name": "Health", "color": "#2f9e5b"},
            {"name": "Ideas", "color": "#8b5cf6"},
            {"name": "Home", "color": "#d97706"},
        ],
        "folders": ["Work", "Personal"],
        "items": items,
        "next": next,
    });
    store.as_object().cloned().unwrap_or_default()
}

/// The file's planner. A new one starts empty, or as the sample when
/// `sample` is asked for. A file that holds no planner is kept beside it as
/// `planner.json.broken` first, so what it held is never written over.
pub(crate) fn load(file: &Path, today: NaiveDate, sample_wanted: bool) -> Map<String, Value> {
    let text = fs::read_to_string(file).ok();
    let read = text
        .as_deref()
        .and_then(|text| serde_json::from_str::<Value>(text).ok());
    match read {
        Some(Value::Object(store)) if store.get("items").is_some_and(Value::is_array) => store,
        _ => {
            if text.is_some() {
                let mut kept = file.as_os_str().to_owned();
                kept.push(".broken");
                let _ = fs::rename(file, kept);
            }
            let store = if sample_wanted {
                sample(today)
            } else {
                empty()
            };
            // As before, a planner that cannot be written still runs.
            let _ = save(file, &store);
            store
        },
    }
}

/// Written whole to a temporary file, then renamed over the old one, so a
/// crash never leaves half a file.
pub(crate) fn save(file: &Path, store: &Map<String, Value>) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut text = String::new();
    pretty(&mut text, &Value::Object(store.clone()), 0);
    text.push('\n');
    let mut temporary = file.as_os_str().to_owned();
    temporary.push(".tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, file)
}

/// Python's `json.dumps(sort_keys=True, ensure_ascii=False, indent=3,
/// separators=(",", " : "))`.
pub(crate) fn pretty(out: &mut String, value: &Value, level: usize) {
    const INDENT: &str = "   ";
    let open = |out: &mut String, level: usize| {
        out.push('\n');
        out.push_str(&INDENT.repeat(level));
    };
    match value {
        Value::Array(items) if !items.is_empty() => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                open(out, level + 1);
                pretty(out, item, level + 1);
            }
            open(out, level);
            out.push(']');
        },
        Value::Object(fields) if !fields.is_empty() => {
            out.push('{');
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                open(out, level + 1);
                out.push_str(&Value::String(key.clone()).to_string());
                out.push_str(" : ");
                pretty(out, &fields[key], level + 1);
            }
            open(out, level);
            out.push('}');
        },
        scalar => out.push_str(&scalar.to_string()),
    }
}
