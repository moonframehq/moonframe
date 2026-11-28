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
/// events around today, an inbox, habits, tags, folders and rich notes.
pub(crate) fn sample(today: NaiveDate) -> Map<String, Value> {
    let today_text = day(today, None, 0);
    let near = |offset| day(today, Some(&today_text), offset);
    let mut items: Vec<Value> = Vec::new();
    let mut item = |mut fields: Value| -> String {
        let id = format!("i{}", items.len() + 1);
        fields["id"] = json!(id);
        items.push(fields);
        id
    };

    // Rich notes first so tasks/events can link to their ids.
    let note_start = item(json!({
        "kind": "note",
        "title": "Getting started",
        "emoji": "\u{2b50}",
        "favorite": true,
        "folder": "Personal",
        "cover": "ocean",
        "tags": ["Ideas"],
        "created": "Oct 18, 2026",
        "edited": "Edited today",
        "body": "# Welcome to Daybook\n\nKeep your **tasks**, **events**, **habits**, and **notes** in one calm place.\n\n- Click any task to open its inspector on the right\n- Switch **Upcoming** between Board columns and List\n- Switch **Calendar** between 3-day, Week, and Month views\n- Open the **AI Assistant** from the top right sparkles button",
    }));
    let note_porto = item(json!({
        "kind": "note",
        "title": "Trip to Porto",
        "emoji": "\u{1f9f3}",
        "folder": "Personal",
        "cover": "sunset",
        "tags": ["Ideas"],
        "created": "Oct 21, 2026",
        "edited": "Edited now",
        "body": "## Itinerary\n\n- **Day 1**: Arrive at Campanha, walk down to Ribeira for sunset dinner.\n- **Day 2**: Livraria Lello in the morning, port tasting across the river in Vila Nova de Gaia.\n- **Day 3**: Tram out to Foz do Douro and coffee by the lighthouse.",
    }));
    let note_launch = item(json!({
        "kind": "note",
        "title": "Launch plan",
        "emoji": "\u{1f680}",
        "folder": "Work",
        "cover": "aurora",
        "tags": ["Work"],
        "created": "Oct 24, 2026",
        "edited": "Edited 2h ago",
        "body": "## Milestones\n\nFinal checks before the public beta goes live.\n\n- Confirm changelog copy with design\n- Record 45-second walkthrough clip\n- Post release thread and monitor telemetry",
    }));
    let _note_ideas = item(json!({
        "kind": "note",
        "title": "App ideas",
        "emoji": "\u{1f4a1}",
        "folder": "Personal",
        "cover": "plum",
        "tags": ["Ideas"],
        "created": "Oct 12, 2026",
        "edited": "Edited 3d ago",
        "body": "## Sparks\n\n- Offline-first reading queue with highlight sync\n- Ambient focus timer tied to daily habits\n- Keyboard-first trip planner with map pins",
    }));

    // Today's tasks, habits, and events.
    item(json!({
        "kind": "task", "title": "Water the plants", "date": today_text,
        "done": true, "folder": "Personal", "tags": ["Home"],
    }));
    item(json!({
        "kind": "task", "title": "Finish the quarterly review", "date": today_text,
        "time": "17:00", "duration": 60, "state": "progress", "folder": "Work", "tags": ["Work"],
        "body": "Pull final Q3 retention numbers and highlight the onboarding lift.",
        "subtasks": [
            {"id": "s1", "title": "Export revenue cohort table", "done": true},
            {"id": "s2", "title": "Draft executive summary", "done": false},
            {"id": "s3", "title": "Share deck with leadership", "done": false},
        ],
    }));
    item(json!({
        "kind": "task", "title": "Call Sam about the offsite", "date": today_text,
        "time": "16:00", "duration": 45, "state": "important", "folder": "Work", "tags": ["Work"],
        "body": "Confirm venue dates, dietary needs, and travel stipends.",
    }));
    item(json!({
        "kind": "task", "title": "Morning run", "date": today_text, "time": "07:30",
        "duration": 45, "habit": true, "streak": 5, "goal": 5, "progress": 3,
        "progress_date": today_text, "folder": "Personal", "tags": ["Health"],
        "done_dates": [near(-5), near(-4), near(-3), near(-2), near(-1)],
        "body": "5 km easy pace along the waterfront.",
    }));
    item(json!({
        "kind": "task", "title": "Pushups x100", "date": today_text, "time": "08:30",
        "duration": 30, "habit": true, "streak": 4, "goal": 100, "progress": 40,
        "progress_date": today_text, "folder": "Personal", "tags": ["Health"],
        "done_dates": [near(-4), near(-3), near(-2), near(-1)],
        "body": "Split into 4 sets of 25 across the morning.",
    }));
    item(json!({
        "kind": "task", "title": "Read 20 pages", "date": today_text,
        "habit": true, "streak": 6, "goal": 20, "progress": 20, "progress_date": today_text,
        "done": true, "folder": "Personal", "tags": ["Health"],
        "done_dates": [near(-5), near(-4), near(-3), near(-2), near(-1), today_text],
    }));
    item(json!({
        "kind": "event", "title": "Design critique", "date": today_text,
        "time": "10:00", "duration": 90, "folder": "Work", "tags": ["Work"],
        "body": "Walk through the new calendar time-blocking prototype.",
    }));
    item(json!({
        "kind": "event", "title": "Dinner with Lea", "date": today_text,
        "time": "19:00", "duration": 90, "folder": "Personal",
    }));

    // Overdue & Inbox items.
    item(json!({
        "kind": "task", "title": "Book the train to Porto", "date": near(-1),
        "state": "important", "folder": "Personal", "tags": ["Home"],
        "note_id": note_porto,
    }));
    item(json!({
        "kind": "task", "title": "Sketch the onboarding flow", "folder": "Work",
        "tags": ["Ideas"], "note_id": note_start,
    }));
    item(json!({
        "kind": "task", "title": "Read the research notes", "folder": "Work",
    }));

    // Upcoming days (+1 to +5) including note-linked tasks & events.
    item(json!({
        "kind": "event", "title": "Team standup", "date": near(1), "time": "09:30",
        "duration": 45, "folder": "Work", "tags": ["Work"],
    }));
    item(json!({
        "kind": "task", "title": "Renew car insurance", "date": near(1), "time": "15:00",
        "duration": 45, "state": "important", "folder": "Personal", "tags": ["Home"],
    }));
    item(json!({
        "kind": "event", "title": "Team lunch", "date": near(2), "time": "12:30",
        "duration": 60, "folder": "Work", "tags": ["Work"],
    }));
    item(json!({
        "kind": "task", "title": "Write release notes", "date": near(2), "time": "14:00",
        "duration": 60, "folder": "Work", "tags": ["Work"], "note_id": note_launch,
    }));
    item(json!({
        "kind": "task", "title": "Prepare launch slides", "date": near(3), "time": "11:00",
        "duration": 60, "folder": "Work", "tags": ["Work"], "note_id": note_launch,
    }));
    item(json!({
        "kind": "task", "title": "Renew the passport", "date": near(3),
        "folder": "Personal", "tags": ["Home"],
    }));
    item(json!({
        "kind": "task", "title": "Check in online", "date": near(4),
        "folder": "Personal", "tags": ["Ideas"], "note_id": note_porto,
    }));
    item(json!({
        "kind": "task", "title": "Pack camera + charger", "date": near(4),
        "folder": "Personal", "note_id": note_porto,
    }));
    item(json!({
        "kind": "task", "title": "Buy travel insurance", "date": near(2),
        "done": true, "folder": "Personal", "note_id": note_porto,
    }));
    item(json!({
        "kind": "event", "title": "Climbing", "date": near(4), "time": "18:30",
        "duration": 90, "folder": "Personal", "tags": ["Health"],
    }));
    item(json!({
        "kind": "event", "title": "Porto flight departure", "date": near(5), "time": "08:40",
        "duration": 120, "folder": "Personal", "note_id": note_porto,
    }));

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
