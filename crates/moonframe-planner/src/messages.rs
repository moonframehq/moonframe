//! The app's messages, one key each; the first key that applies wins, in
//! the order the Python provider tried them.

use {
    crate::{
        Planner,
        dates::{civil, day, format, label, last_day, split},
        values::{field, integer, scalar, text, truthy},
    },
    chrono::NaiveDate,
    serde_json::{Map, Value, json},
    std::collections::BTreeSet,
};

/// The longest title a new task keeps, in characters.
const MAX_TITLE: usize = 200;

/// The longest body a note or task description keeps, in characters.
const MAX_BODY: usize = 20_000;

/// Curated tag colors cycled when a new tag is created without a color.
const TAG_COLORS: [&str; 6] = [
    "#3b6fd8", "#2f9e5b", "#8b5cf6", "#d97706", "#e5484d", "#0891b2",
];

/// Valid cover gradient presets for notes.
const COVERS: [&str; 7] = [
    "sunset", "ocean", "forest", "aurora", "amber", "plum", "none",
];

/// What a task's state becomes next: open, important, in progress, open.
fn following(state: &str) -> &'static str {
    match state {
        "open" => "important",
        "important" => "progress",
        _ => "open",
    }
}

/// Recomputes a habit's consecutive-day streak ending at `today` (or yesterday
/// if today is not yet checked off).
pub(crate) fn compute_streak(dates: &BTreeSet<String>, today: NaiveDate) -> i64 {
    let today_str = day(today, None, 0);
    let mut cursor = if dates.contains(&today_str) {
        0
    } else {
        -1
    };
    let mut streak = 0;
    while dates.contains(&day(today, None, cursor)) {
        streak += 1;
        cursor -= 1;
    }
    streak
}

pub(crate) fn done_dates_set(item: &Map<String, Value>) -> BTreeSet<String> {
    item.get("done_dates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// Whether an item counts as completed on `today`. For habits, derives
/// completion from `done_dates` when present, falling back to a same-day
/// `done` flag for older habits that predate `done_dates`.
pub(crate) fn is_done_today(item: &Map<String, Value>, today: &str) -> bool {
    if item.get("habit").is_some_and(truthy) {
        if item.get("done_dates").is_some_and(Value::is_array) {
            return done_dates_set(item).contains(today);
        }
        return item.get("done").is_some_and(truthy)
            && item.get("date").filter(|d| truthy(d)).map(text).as_deref() == Some(today);
    }
    item.get("done").is_some_and(truthy)
}

/// Effective progress for a habit on `today`, resetting stale values from
/// earlier days.
pub(crate) fn habit_progress_for_day(item: &Map<String, Value>, today_str: &str) -> i64 {
    if item.get("progress_date").and_then(Value::as_str) == Some(today_str) {
        return item.get("progress").and_then(integer).unwrap_or(0).max(0);
    }
    if is_done_today(item, today_str) {
        return item.get("goal").and_then(integer).unwrap_or(0).max(0);
    }
    0
}

fn sync_habit_today(item: &mut Map<String, Value>, today: NaiveDate, checked: bool) {
    let today_str = day(today, None, 0);
    let mut dates = done_dates_set(item);
    if checked {
        dates.insert(today_str.clone());
    } else {
        dates.remove(&today_str);
    }
    let streak = compute_streak(&dates, today);
    let list: Vec<Value> = dates.into_iter().map(Value::String).collect();
    item.insert("done_dates".to_owned(), Value::Array(list));
    item.insert("streak".to_owned(), json!(streak));
    if let Some(goal) = item.get("goal").and_then(integer).filter(|&g| g > 0) {
        let progress = if item.get("progress_date").and_then(Value::as_str) == Some(&today_str) {
            item.get("progress").and_then(integer).unwrap_or(0)
        } else {
            0
        };
        let next = if checked && progress < goal {
            goal
        } else if !checked && progress >= goal {
            0
        } else {
            progress.clamp(0, goal)
        };
        item.insert("progress".to_owned(), json!(next));
        item.insert("progress_date".to_owned(), json!(today_str));
    }
}

impl Planner {
    fn find(&mut self, id: &Value) -> Option<&mut Map<String, Value>> {
        let id = text(id);
        let items = self.store.get_mut("items")?.as_array_mut()?;
        items
            .iter_mut()
            .filter_map(Value::as_object_mut)
            .find(|item| item.get("id").map(text).unwrap_or_default() == id)
    }

    fn selected_or(&self, today: NaiveDate) -> String {
        self.selected.clone().unwrap_or_else(|| day(today, None, 0))
    }

    /// Acts on one message; whether the file changed.
    pub(crate) fn receive(&mut self, msg: &Value, today: NaiveDate) -> bool {
        if !msg.is_object() {
            return false;
        }
        if let Some(pick) = msg.get("select").filter(|pick| scalar(pick)) {
            let pick = text(pick);
            let date = (pick != "today").then_some(pick.as_str());
            self.selected = Some(day(today, date, 0));
            return false;
        }
        if let Some(shift) = msg.get("shift").and_then(integer) {
            self.selected = Some(day(today, self.selected.as_deref(), shift));
            return false;
        }
        if let Some(months) = msg.get("month").and_then(integer) {
            if let Some(date) = self.month(today, months) {
                self.selected = Some(date);
            }
            return false;
        }
        if let Some(id) = field(msg, "toggle")
            && let Some(item) = self.find(id)
        {
            let today_str = day(today, None, 0);
            let is_habit = item.get("habit").is_some_and(truthy);
            let done = !is_done_today(item, &today_str);
            item.insert("done".to_owned(), json!(done));
            if is_habit {
                sync_habit_today(item, today, done);
            }
            return true;
        }
        if let Some(id) = field(msg, "cycle")
            && let Some(task) = self.find(id)
        {
            let state = field_text(task.get("state")).unwrap_or_else(|| "open".to_owned());
            task.insert("state".to_owned(), json!(following(&state)));
            return true;
        }
        if let Some(add) = msg.get("add").filter(|add| scalar(add)) {
            let add = text(add);
            if add.chars().any(|c| !c.is_whitespace()) {
                self.add_item(add.trim(), msg, today);
                return true;
            }
        }
        if let Some(look) = msg.get("look").and_then(Value::as_str)
            && ["classic", "sepia", "light"].contains(&look)
        {
            self.store.insert("look".to_owned(), json!(look));
            return true;
        }
        if let Some(gone) = field(msg, "remove") {
            let gone = text(gone);
            if let Some(items) = self.store.get_mut("items").and_then(Value::as_array_mut) {
                items.retain(|item| item.get("id").map(text).unwrap_or_default() != gone);
            }
            return true;
        }
        if let Some(id) = field(msg, "update") {
            return self.update_item(id, msg, today);
        }
        if let Some(id) = field(msg, "subtask_add") {
            return self.subtask_add(id, msg);
        }
        if let Some(id) = field(msg, "subtask_toggle") {
            return self.subtask_toggle(id, msg);
        }
        if let Some(id) = field(msg, "subtask_remove") {
            return self.subtask_remove(id, msg);
        }
        if let Some(id) = field(msg, "habit_checkin") {
            return self.habit_checkin(id, msg, today);
        }
        if let Some(id) = field(msg, "habit_step") {
            return self.habit_step(id, msg, today);
        }
        if let Some(name) = msg.get("folder_add").filter(|v| scalar(v)) {
            return self.folder_add(&text(name));
        }
        if let Some(name) = msg.get("folder_remove").filter(|v| scalar(v)) {
            return self.folder_remove(&text(name));
        }
        if let Some(name) = msg.get("tag_add").filter(|v| scalar(v)) {
            return self.tag_add(&text(name), msg.get("color").and_then(Value::as_str));
        }
        if let Some(name) = msg.get("tag_remove").filter(|v| scalar(v)) {
            return self.tag_remove(&text(name));
        }
        if msg.get("reschedule_overdue").is_some_and(truthy) {
            return self.reschedule_overdue(today);
        }
        false
    }

    /// The same day `months` months on; the 31st of a shorter month is its
    /// last day. `None` past a four-digit year.
    fn month(&self, today: NaiveDate, months: i64) -> Option<String> {
        let (y, m, d) = split(&self.selected_or(today))?;
        let months = y.checked_mul(12)?.checked_add(m - 1)?.checked_add(months)?;
        let (y, m) = (months.div_euclid(12), months.rem_euclid(12) + 1);
        if !(0..=9999).contains(&y) {
            return None;
        }
        Some(format(civil(y, m, d.min(last_day(y, m)))))
    }

    /// A task, event or note; its id comes from the file's `next`.
    fn add_item(&mut self, title: &str, msg: &Value, today: NaiveDate) {
        let title: String = title.chars().take(MAX_TITLE).collect();
        let number = self.store.get("next").cloned().unwrap_or(Value::Null);
        let next = integer(&number).unwrap_or(0).saturating_add(1);
        self.store.insert("next".to_owned(), json!(next));
        let id = if number.is_null() {
            "0".to_owned()
        } else {
            text(&number)
        };
        let kind = match msg.get("kind").and_then(Value::as_str) {
            Some("event") => "event",
            Some("note") => "note",
            _ => "task",
        };
        let mut item = Map::new();
        item.insert("id".to_owned(), json!(format!("i{id}")));
        item.insert("kind".to_owned(), json!(kind));
        item.insert("title".to_owned(), json!(title));

        if let Some(explicit_date) = msg.get("date").and_then(Value::as_str) {
            if !explicit_date.is_empty() && split(explicit_date).is_some() {
                item.insert("date".to_owned(), json!(day(today, Some(explicit_date), 0)));
            }
        } else if kind != "note" && !msg.get("inbox").is_some_and(truthy) {
            item.insert("date".to_owned(), json!(self.selected_or(today)));
        }

        if let Some(time) = msg.get("time").and_then(Value::as_str)
            && !time.is_empty()
        {
            item.insert("time".to_owned(), json!(time));
        }
        if let Some(duration) = msg.get("duration").and_then(integer)
            && (15..=720).contains(&duration)
        {
            item.insert("duration".to_owned(), json!(duration));
        }
        if let Some(state) = msg.get("state").and_then(Value::as_str)
            && ["open", "important", "progress"].contains(&state)
        {
            item.insert("state".to_owned(), json!(state));
        }
        if msg.get("habit").is_some_and(truthy) {
            item.insert("habit".to_owned(), json!(true));
            item.insert("streak".to_owned(), json!(0));
            item.insert("done_dates".to_owned(), json!([]));
            if let Some(goal) = msg.get("goal").and_then(integer).filter(|&g| g > 0) {
                item.insert("goal".to_owned(), json!(goal));
                item.insert("progress".to_owned(), json!(0));
                item.insert("progress_date".to_owned(), json!(day(today, None, 0)));
            }
        }
        if let Some(folder) = msg.get("folder").and_then(Value::as_str)
            && !folder.trim().is_empty()
        {
            item.insert("folder".to_owned(), json!(folder.trim()));
        }
        if let Some(tags) = msg.get("tags").and_then(Value::as_array) {
            let clean: Vec<Value> = tags
                .iter()
                .filter_map(Value::as_str)
                .filter(|t| !t.trim().is_empty())
                .map(|t| json!(t.trim()))
                .collect();
            if !clean.is_empty() {
                item.insert("tags".to_owned(), Value::Array(clean));
            }
        }
        if let Some(emoji) = msg.get("emoji").and_then(Value::as_str)
            && !emoji.is_empty()
        {
            item.insert("emoji".to_owned(), json!(emoji));
        }
        if let Some(cover) = msg.get("cover").and_then(Value::as_str)
            && COVERS.contains(&cover)
        {
            item.insert("cover".to_owned(), json!(cover));
        }
        if let Some(body) = msg.get("body").and_then(Value::as_str)
            && !body.is_empty()
        {
            let clipped: String = body.chars().take(MAX_BODY).collect();
            item.insert("body".to_owned(), json!(clipped));
        }
        if let Some(note_id) = msg.get("note_id").and_then(Value::as_str)
            && !note_id.is_empty()
        {
            item.insert("note_id".to_owned(), json!(note_id));
        }
        if msg.get("favorite").is_some_and(truthy) {
            item.insert("favorite".to_owned(), json!(true));
        }
        if kind == "note" {
            let today_str = day(today, None, 0);
            item.insert("created".to_owned(), json!(label(&today_str, "%b %-d, %Y")));
            item.insert("edited".to_owned(), json!("Edited now"));
            if !item.contains_key("emoji") {
                item.insert("emoji".to_owned(), json!("📝"));
            }
            if !item.contains_key("cover") {
                item.insert("cover".to_owned(), json!("sunset"));
            }
        }

        if let Some(items) = self.store.get_mut("items").and_then(Value::as_array_mut) {
            items.push(Value::Object(item));
        }
    }

    fn update_item(&mut self, id: &Value, msg: &Value, today: NaiveDate) -> bool {
        let Some(item) = self.find(id) else {
            return false;
        };
        let mut changed = false;
        if let Some(title) = msg.get("title").filter(|v| scalar(v)) {
            let trimmed = text(title);
            let trimmed = trimmed.trim();
            if !trimmed.is_empty() {
                let clipped: String = trimmed.chars().take(MAX_TITLE).collect();
                item.insert("title".to_owned(), json!(clipped));
                changed = true;
            }
        }
        if let Some(date) = msg.get("date").and_then(Value::as_str) {
            if date.is_empty() {
                item.remove("date");
                changed = true;
            } else if split(date).is_some() {
                item.insert("date".to_owned(), json!(day(today, Some(date), 0)));
                changed = true;
            }
        }
        if let Some(time) = msg.get("time").and_then(Value::as_str) {
            if time.is_empty() {
                item.remove("time");
            } else {
                item.insert("time".to_owned(), json!(time));
            }
            changed = true;
        }
        if let Some(duration) = msg.get("duration").and_then(integer)
            && (15..=720).contains(&duration)
        {
            item.insert("duration".to_owned(), json!(duration));
            changed = true;
        }
        if let Some(state) = msg.get("state").and_then(Value::as_str)
            && ["open", "important", "progress"].contains(&state)
        {
            item.insert("state".to_owned(), json!(state));
            changed = true;
        }
        if let Some(done) = msg.get("done").and_then(Value::as_bool) {
            item.insert("done".to_owned(), json!(done));
            if item.get("habit").is_some_and(truthy) {
                sync_habit_today(item, today, done);
            }
            changed = true;
        }
        if let Some(favorite) = msg.get("favorite").and_then(Value::as_bool) {
            item.insert("favorite".to_owned(), json!(favorite));
            changed = true;
        }
        if let Some(folder) = msg.get("folder").and_then(Value::as_str) {
            if folder.trim().is_empty() {
                item.remove("folder");
            } else {
                item.insert("folder".to_owned(), json!(folder.trim()));
            }
            changed = true;
        }
        if let Some(note_id) = msg.get("note_id").and_then(Value::as_str) {
            if note_id.is_empty() {
                item.remove("note_id");
            } else {
                item.insert("note_id".to_owned(), json!(note_id));
            }
            changed = true;
        }
        if let Some(emoji) = msg.get("emoji").and_then(Value::as_str)
            && !emoji.is_empty()
        {
            item.insert("emoji".to_owned(), json!(emoji));
            changed = true;
        }
        if let Some(cover) = msg.get("cover").and_then(Value::as_str)
            && COVERS.contains(&cover)
        {
            item.insert("cover".to_owned(), json!(cover));
            changed = true;
        }
        if let Some(body) = msg.get("body").and_then(Value::as_str) {
            let clipped: String = body.chars().take(MAX_BODY).collect();
            item.insert("body".to_owned(), json!(clipped));
            changed = true;
        }
        if let Some(tags) = msg.get("tags").and_then(Value::as_array) {
            let clean: Vec<Value> = tags
                .iter()
                .filter_map(Value::as_str)
                .filter(|t| !t.trim().is_empty())
                .map(|t| json!(t.trim()))
                .collect();
            item.insert("tags".to_owned(), Value::Array(clean));
            changed = true;
        }
        if let Some(toggle_tag) = msg.get("toggle_tag").and_then(Value::as_str)
            && !toggle_tag.trim().is_empty()
        {
            let tag = toggle_tag.trim().to_owned();
            let entry = item
                .entry("tags".to_owned())
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Some(list) = entry.as_array_mut() {
                if let Some(pos) = list.iter().position(|v| v.as_str() == Some(tag.as_str())) {
                    list.remove(pos);
                } else {
                    list.push(Value::String(tag));
                }
                changed = true;
            }
        }
        if changed && item.get("kind").and_then(Value::as_str) == Some("note") {
            item.insert("edited".to_owned(), json!("Edited now"));
        }
        changed
    }

    fn subtask_add(&mut self, id: &Value, msg: &Value) -> bool {
        let Some(title) = msg.get("title").filter(|v| scalar(v)) else {
            return false;
        };
        let title = text(title);
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return false;
        }
        let clipped: String = trimmed.chars().take(MAX_TITLE).collect();
        let next_num = self
            .store
            .get("next")
            .and_then(integer)
            .unwrap_or(1)
            .saturating_add(1);
        let Some(item) = self.find(id) else {
            return false;
        };
        let entry = item
            .entry("subtasks".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(list) = entry.as_array_mut() else {
            return false;
        };
        let sub_id = format!("s{}", next_num.saturating_sub(1));
        list.push(json!({ "id": sub_id, "title": clipped, "done": false }));
        self.store.insert("next".to_owned(), json!(next_num));
        true
    }

    fn subtask_toggle(&mut self, id: &Value, msg: &Value) -> bool {
        let Some(sub_id) = field(msg, "subtask") else {
            return false;
        };
        let sub_id = text(sub_id);
        let Some(item) = self.find(id) else {
            return false;
        };
        let Some(list) = item.get_mut("subtasks").and_then(Value::as_array_mut) else {
            return false;
        };
        for sub in list.iter_mut().filter_map(Value::as_object_mut) {
            if sub.get("id").map(text).unwrap_or_default() == sub_id {
                let done = sub.get("done").is_some_and(truthy);
                sub.insert("done".to_owned(), json!(!done));
                return true;
            }
        }
        false
    }

    fn subtask_remove(&mut self, id: &Value, msg: &Value) -> bool {
        let Some(sub_id) = field(msg, "subtask") else {
            return false;
        };
        let sub_id = text(sub_id);
        let Some(item) = self.find(id) else {
            return false;
        };
        let Some(list) = item.get_mut("subtasks").and_then(Value::as_array_mut) else {
            return false;
        };
        let before = list.len();
        list.retain(|sub| sub.get("id").map(text).unwrap_or_default() != sub_id);
        list.len() != before
    }

    fn habit_checkin(&mut self, id: &Value, msg: &Value, today: NaiveDate) -> bool {
        let target_date = msg
            .get("date")
            .and_then(Value::as_str)
            .filter(|d| split(d).is_some())
            .map(|d| day(today, Some(d), 0))
            .unwrap_or_else(|| day(today, None, 0));
        let today_str = day(today, None, 0);
        let Some(item) = self.find(id) else {
            return false;
        };
        let mut dates = done_dates_set(item);
        if item.get("done_dates").is_none() && is_done_today(item, &today_str) {
            dates.insert(today_str.clone());
        }
        let now_checked = if dates.contains(&target_date) {
            dates.remove(&target_date);
            false
        } else {
            dates.insert(target_date.clone());
            true
        };
        let streak = compute_streak(&dates, today);
        let list: Vec<Value> = dates.into_iter().map(Value::String).collect();
        item.insert("done_dates".to_owned(), Value::Array(list));
        item.insert("streak".to_owned(), json!(streak));
        if target_date == today_str {
            item.insert("done".to_owned(), json!(now_checked));
            if let Some(goal) = item.get("goal").and_then(integer).filter(|&g| g > 0) {
                item.insert(
                    "progress".to_owned(),
                    json!(if now_checked {
                        goal
                    } else {
                        0
                    }),
                );
                item.insert("progress_date".to_owned(), json!(today_str));
            }
        }
        true
    }

    fn habit_step(&mut self, id: &Value, msg: &Value, today: NaiveDate) -> bool {
        let Some(delta) = msg.get("delta").and_then(integer) else {
            return false;
        };
        let today_str = day(today, None, 0);
        let Some(item) = self.find(id) else {
            return false;
        };
        let goal = item.get("goal").and_then(integer).unwrap_or(100).max(1);
        let current = habit_progress_for_day(item, &today_str);
        let next = (current + delta).clamp(0, goal);
        item.insert("progress".to_owned(), json!(next));
        item.insert("progress_date".to_owned(), json!(today_str));
        let done = next >= goal;
        item.insert("done".to_owned(), json!(done));
        sync_habit_today(item, today, done);
        true
    }

    fn folder_add(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let clipped: String = name.chars().take(60).collect();
        let entry = self
            .store
            .entry("folders".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(folders) = entry.as_array_mut() else {
            return false;
        };
        if folders.iter().any(|f| f.as_str() == Some(clipped.as_str())) {
            return false;
        }
        folders.push(Value::String(clipped));
        true
    }

    fn folder_remove(&mut self, name: &str) -> bool {
        let Some(folders) = self.store.get_mut("folders").and_then(Value::as_array_mut) else {
            return false;
        };
        let before = folders.len();
        folders.retain(|f| f.as_str() != Some(name));
        folders.len() != before
    }

    fn tag_add(&mut self, name: &str, color: Option<&str>) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let clipped: String = name.chars().take(40).collect();
        let entry = self
            .store
            .entry("tags".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(tags) = entry.as_array_mut() else {
            return false;
        };
        if tags
            .iter()
            .any(|t| t.get("name").and_then(Value::as_str) == Some(clipped.as_str()))
        {
            return false;
        }
        let chosen = color
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| TAG_COLORS[tags.len() % TAG_COLORS.len()]);
        tags.push(json!({ "name": clipped, "color": chosen }));
        true
    }

    fn tag_remove(&mut self, name: &str) -> bool {
        let Some(tags) = self.store.get_mut("tags").and_then(Value::as_array_mut) else {
            return false;
        };
        let before = tags.len();
        tags.retain(|t| t.get("name").and_then(Value::as_str) != Some(name));
        tags.len() != before
    }

    fn reschedule_overdue(&mut self, today: NaiveDate) -> bool {
        let today_str = day(today, None, 0);
        let Some(items) = self.store.get_mut("items").and_then(Value::as_array_mut) else {
            return false;
        };
        let mut changed = false;
        for item in items.iter_mut().filter_map(Value::as_object_mut) {
            let is_task = item.get("kind").and_then(Value::as_str) == Some("task");
            let done = is_done_today(item, &today_str);
            let earlier = item
                .get("date")
                .filter(|d| truthy(d))
                .map(text)
                .is_some_and(|d| d < today_str);
            if is_task && !done && earlier {
                item.insert("date".to_owned(), json!(today_str));
                if item.get("habit").is_some_and(truthy) {
                    item.insert("done".to_owned(), json!(false));
                }
                changed = true;
            }
        }
        changed
    }
}

/// A field's text, or `None` when it is missing or null.
fn field_text(value: Option<&Value>) -> Option<String> {
    value.filter(|value| !value.is_null()).map(text)
}
