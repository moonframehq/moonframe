//! What `app.planner` holds: the items as the views show them, the day strip
//! and month around the selected day, and the dates (views have no clock).

use {
    crate::{
        Planner,
        dates::{day, label, parts},
        messages::{compute_streak, done_dates_set, habit_progress_for_day, is_done_today},
        values::{field, integer, text, truthy},
    },
    chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike},
    serde_json::{Map, Value, json},
    std::collections::{BTreeMap, BTreeSet},
};

/// Days the strip shows, from the selected one.
const STRIP: i64 = 14;

/// Default block length on the hourly calendar, in minutes.
const DEFAULT_DURATION_MINS: i64 = 60;

fn is(item: &Map<String, Value>, key: &str, word: &str) -> bool {
    item.get(key).and_then(Value::as_str) == Some(word)
}

fn parse_hour(time: &str) -> Option<f64> {
    let (h, m) = time.split_once(':')?;
    let h: f64 = h.parse().ok()?;
    let m: f64 = m.parse().ok()?;
    if (0.0..24.0).contains(&h) && (0.0..60.0).contains(&m) {
        Some(h + m / 60.0)
    } else {
        None
    }
}

impl Planner {
    /// The items, as stored: only tables count as items.
    pub(crate) fn items(&self) -> impl Iterator<Item = &Map<String, Value>> {
        let items = self.store.get("items").and_then(Value::as_array);
        items.into_iter().flatten().filter_map(Value::as_object)
    }

    pub(crate) fn published(&mut self, now: NaiveDateTime) -> Value {
        let today_date = now.date();
        let today = day(today_date, None, 0);
        let selected = self.selected.get_or_insert_with(|| today.clone()).clone();
        let clock = now.format("%H:%M").to_string();
        let now_hour = f64::from(now.hour()) + f64::from(now.minute()) / 60.0;
        let open_dated =
            |item: &&Map<String, Value>| !is(item, "kind", "note") && !is_done_today(item, &today);
        let busy: BTreeSet<String> = self
            .items()
            .filter(open_dated)
            .map(|item| item.get("date").map(text).unwrap_or_default())
            .collect();
        let mut count: BTreeMap<String, u64> = BTreeMap::new();
        let mut done_count: BTreeMap<String, u64> = BTreeMap::new();
        for item in self.items() {
            if is(item, "kind", "note") {
                continue;
            }
            if let Some(date) = item.get("date").filter(|date| truthy(date)) {
                let key = text(date);
                if is_done_today(item, &today) {
                    *done_count.entry(key).or_default() += 1;
                } else {
                    *count.entry(key).or_default() += 1;
                }
            }
        }
        let days: Vec<Value> = (0..STRIP)
            .map(|offset| {
                let date = day(today_date, Some(&selected), offset);
                json!({
                    "id": date, "day": label(&date, "%-d"), "weekday": label(&date, "%a"),
                    "today": date == today, "dot": busy.contains(&date),
                })
            })
            .collect();
        let day_details: Vec<Value> = (0..STRIP)
            .map(|offset| {
                let date = day(today_date, Some(&selected), offset);
                json!({
                    "id": date,
                    "day": label(&date, "%-d"),
                    "weekday": label(&date, "%a"),
                    "full_weekday": label(&date, "%A"),
                    "short_date": label(&date, "%b %-d"),
                    "today": date == today,
                    "count": count.get(&date).copied().unwrap_or(0),
                    "done_count": done_count.get(&date).copied().unwrap_or(0),
                })
            })
            .collect();
        let week_history: Vec<Value> = (-6..=0)
            .map(|offset| {
                let date = day(today_date, None, offset);
                json!({
                    "id": date,
                    "day": label(&date, "%-d"),
                    "weekday": label(&date, "%a"),
                    "today": date == today,
                })
            })
            .collect();
        let items: Vec<Value> = self
            .items()
            .map(|original| shown(original, today_date, &today, &clock))
            .collect();
        let month_key: String = selected.chars().take(7).collect();
        let first = format!("{month_key}-01");
        let back = parts(&first).map_or(0, |date| date.weekday().num_days_from_monday());
        let start = day(today_date, Some(&first), -i64::from(back));
        let mut weeks = Vec::new();
        for week in 0..6 {
            let row: Vec<Value> = (0..7)
                .map(|weekday| {
                    let date = day(today_date, Some(&start), week * 7 + weekday);
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
        let quick_dates = json!({
            "today": today,
            "tomorrow": day(today_date, None, 1),
            "plus3": day(today_date, None, 3),
        });
        let kept = |key: &str, otherwise: Value| field_of(&self.store, key).unwrap_or(otherwise);
        json!({
            "available": true,
            "look": kept("look", json!("classic")),
            "month": { "title": label(&first, "%B %Y"), "weeks": weeks },
            "title": title,
            "subtitle": label(&selected, "%Y. %B %-d."),
            "today": today,
            "selected": selected,
            "quick_dates": quick_dates,
            "now_hour": now_hour,
            "greeting": greeting,
            "days": days,
            "day_details": day_details,
            "week_history": week_history,
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
/// `when`, `overdue`, `due`, `days_left`, `start_hour`/`end_hour` worked out,
/// `time` without a leading zero and `tags`/`subtasks`/`done_dates` always there.
fn shown(original: &Map<String, Value>, today_date: NaiveDate, today: &str, clock: &str) -> Value {
    let mut item = Value::Object(original.clone());
    let is_habit = original.get("habit").is_some_and(truthy);
    let done = is_done_today(original, today);
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
    if !is(original, "kind", "note")
        && !done
        && let Some(date_str) = when.as_deref()
        && let Some(parsed) = parts(date_str)
    {
        let diff = (parsed - today_date).num_days();
        if diff > 0 {
            item["days_left"] = json!(format!("{diff}d left"));
        }
    }
    if let Some(time_val) = field(&item, "time") {
        let time_str = text(time_val);
        if let Some(start) = parse_hour(&time_str) {
            let duration = item
                .get("duration")
                .and_then(integer)
                .filter(|&d| (15..=720).contains(&d))
                .unwrap_or(DEFAULT_DURATION_MINS);
            let end = (start + duration as f64 / 60.0).min(24.0);
            item["start_hour"] = json!(start);
            item["end_hour"] = json!(end);
        }
        item["time"] = json!(time_str.strip_prefix('0').unwrap_or(&time_str));
    }
    if field(&item, "tags").is_none() {
        item["tags"] = json!([]);
    }
    if field(&item, "subtasks").is_none() {
        item["subtasks"] = json!([]);
    }
    if field(&item, "done_dates").is_none() {
        item["done_dates"] = json!([]);
    }
    if is_habit {
        let mut dates = done_dates_set(original);
        if original.get("done_dates").is_none() && done {
            dates.insert(today.to_owned());
            item["done_dates"] = Value::Array(dates.iter().cloned().map(Value::String).collect());
        }
        item["streak"] = json!(compute_streak(&dates, today_date));
        if original
            .get("goal")
            .and_then(integer)
            .is_some_and(|g| g > 0)
        {
            item["progress"] = json!(habit_progress_for_day(original, today));
        }
    }
    item
}
