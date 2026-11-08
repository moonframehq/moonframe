//! What `app.planner` holds: the items as the views show them, the day strip
//! and month around the selected day, and the dates (views have no clock).

use {
    crate::{
        Planner,
        dates::{day, label, parts},
        values::{field, text, truthy},
    },
    chrono::{Datelike, NaiveDateTime, Timelike},
    serde_json::{Map, Value, json},
    std::collections::{BTreeMap, BTreeSet},
};

/// Days the strip shows, from the selected one.
const STRIP: i64 = 14;

fn is(item: &Map<String, Value>, key: &str, word: &str) -> bool {
    item.get(key).and_then(Value::as_str) == Some(word)
}

impl Planner {
    /// The items, as stored: only tables count as items.
    pub(crate) fn items(&self) -> impl Iterator<Item = &Map<String, Value>> {
        let items = self.store.get("items").and_then(Value::as_array);
        items.into_iter().flatten().filter_map(Value::as_object)
    }

    pub(crate) fn published(&mut self, now: NaiveDateTime) -> Value {
        let today = day(now.date(), None, 0);
        let selected = self.selected.get_or_insert_with(|| today.clone()).clone();
        let clock = now.format("%H:%M").to_string();
        let open_dated = |item: &&Map<String, Value>| {
            !is(item, "kind", "note") && !item.get("done").is_some_and(truthy)
        };
        let busy: BTreeSet<String> = self
            .items()
            .filter(open_dated)
            .map(|item| item.get("date").map(text).unwrap_or_default())
            .collect();
        let days: Vec<Value> = (0..STRIP)
            .map(|offset| {
                let date = day(now.date(), Some(&selected), offset);
                json!({
                    "id": date, "day": label(&date, "%-d"), "weekday": label(&date, "%a"),
                    "today": date == today, "dot": busy.contains(&date),
                })
            })
            .collect();
        let items: Vec<Value> = self
            .items()
            .map(|original| shown(original, &today, &clock))
            .collect();
        let mut count: BTreeMap<String, u64> = BTreeMap::new();
        for item in self.items().filter(open_dated) {
            if let Some(date) = item.get("date").filter(|date| truthy(date)) {
                *count.entry(text(date)).or_default() += 1;
            }
        }
        let month_key: String = selected.chars().take(7).collect();
        let first = format!("{month_key}-01");
        let back = parts(&first).map_or(0, |date| date.weekday().num_days_from_monday());
        let start = day(now.date(), Some(&first), -i64::from(back));
        let mut weeks = Vec::new();
        for week in 0..6 {
            let row: Vec<Value> = (0..7)
                .map(|weekday| {
                    let date = day(now.date(), Some(&start), week * 7 + weekday);
                    json!({
                        "id": date, "day": label(&date, "%-d"), "today": date == today,
                        "outside": !date.starts_with(&month_key),
                        "count": count.get(&date).copied().unwrap_or(0),
                    })
                })
                .collect();
            if week >= 4 && row[0]["outside"] == true {
                break;
            }
            weeks.push(row);
        }
        let greeting = match now.hour() {
            0..5 => "Good night",
            5..12 => "Good morning",
            12..18 => "Good afternoon",
            _ => "Good evening",
        };
        let title = if selected == today {
            "Today".to_owned()
        } else {
            label(&selected, "%A")
        };
        let kept = |key: &str, otherwise: Value| field_of(&self.store, key).unwrap_or(otherwise);
        json!({
            "available": true,
            "look": kept("look", json!("classic")),
            "month": { "title": label(&first, "%B %Y"), "weeks": weeks },
            "title": title,
            "subtitle": label(&selected, "%Y. %B %-d."),
            "today": today,
            "selected": selected,
            "greeting": greeting,
            "days": days,
            "items": items,
            "tags": kept("tags", json!([])),
            "folders": kept("folders", json!([])),
        })
    }
}

fn field_of(store: &Map<String, Value>, key: &str) -> Option<Value> {
    store.get(key).filter(|value| !value.is_null()).cloned()
}

/// One item as the views get it: its stored fields, with `state`, `done`,
/// `when`, `overdue` and `due` worked out, `time` without a leading zero and
/// `tags` always there.
fn shown(original: &Map<String, Value>, today: &str, clock: &str) -> Value {
    let mut item = Value::Object(original.clone());
    let done = item.get("done").is_some_and(truthy);
    let when = item.get("date").filter(|d| truthy(d)).map(text);
    let time = item.get("time").filter(|t| truthy(t)).map(text);
    let state = field(&item, "state").cloned().unwrap_or(json!("open"));
    item["state"] = if done {
        json!("done")
    } else {
        state
    };
    item["done"] = json!(done);
    item["when"] = when
        .as_ref()
        .map_or(Value::Null, |when| json!(label(when, "%a, %b %-d")));
    let late = when.as_deref().is_some_and(|when| {
        when < today || (when == today && time.as_deref().is_some_and(|time| time <= clock))
    });
    let earlier = when.as_deref().is_some_and(|when| when < today);
    item["overdue"] = json!(is(original, "kind", "task") && !done && earlier);
    item["due"] = json!(!is(original, "kind", "note") && !done && time.is_some() && late);
    if let Some(time) = field(&item, "time") {
        let time = text(time);
        item["time"] = json!(time.strip_prefix('0').unwrap_or(&time));
    }
    if field(&item, "tags").is_none() {
        item["tags"] = json!([]);
    }
    item
}
