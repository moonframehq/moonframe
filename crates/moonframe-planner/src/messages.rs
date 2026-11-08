//! The app's messages, one key each; the first key that applies wins, in
//! the order the Python provider tried them.

use {
    crate::{
        Planner,
        dates::{civil, day, format, last_day, split},
        values::{field, integer, scalar, text, truthy},
    },
    chrono::NaiveDate,
    serde_json::{Map, Value, json},
};

/// The longest title a new task keeps, in characters.
const MAX_TITLE: usize = 200;

/// What a task's state becomes next: open, important, in progress, open.
fn following(state: &str) -> &'static str {
    match state {
        "open" => "important",
        "important" => "progress",
        _ => "open",
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
            let done = item.get("done").is_some_and(truthy);
            item.insert("done".to_owned(), json!(!done));
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
                self.add(add.trim(), !msg.get("inbox").is_some_and(truthy), today);
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

    /// A task for the day shown, or with no date from the inbox; its id
    /// comes from the file's `next`.
    fn add(&mut self, title: &str, dated: bool, today: NaiveDate) {
        let title: String = title.chars().take(MAX_TITLE).collect();
        let number = self.store.get("next").cloned().unwrap_or(Value::Null);
        let next = integer(&number).unwrap_or(0).saturating_add(1);
        self.store.insert("next".to_owned(), json!(next));
        let id = if number.is_null() {
            "0".to_owned()
        } else {
            text(&number)
        };
        let mut task = json!({ "id": format!("i{id}"), "kind": "task", "title": title });
        if dated {
            task["date"] = json!(self.selected_or(today));
        }
        if let Some(items) = self.store.get_mut("items").and_then(Value::as_array_mut) {
            items.push(task);
        }
    }
}

/// A field's text, or `None` when it is missing or null.
fn field_text(value: Option<&Value>) -> Option<String> {
    value.filter(|value| !value.is_null()).map(text)
}
