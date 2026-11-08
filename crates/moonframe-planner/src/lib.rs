//! The `planner` native provider: Daybook's tasks, events and notes, kept in
//! a JSON file, and the dates the views need (views have no clock). It
//! replaces a Python process provider and keeps its file, its format and
//! what it published.
//!
//! Publishes `app.planner`:
//!
//! - `today`, `selected`: dates as YYYY-MM-DD; `selected` is the day shown
//! - `title`, `subtitle`: "Today" (or the weekday) and "2026. October 31."
//! - `greeting`: "Good evening"
//! - `days`: two weeks from the selected day, for `day-strip`: `{ id, day,
//!   weekday, today, dot }`
//! - `items`: every task, event and note: `{ id, kind (task, event, note),
//!   title, date?, time?, when (the date as "Sat, Oct 31"), state (open,
//!   important, progress, done), due, overdue, habit, tags, emoji?,
//!   favorite }`
//! - `tags`: `{ name, color }`
//! - `folders`: folder names
//! - `month`: the selected day's month, Monday first: `{ title, weeks = { {
//!   { id, day, today, outside, count } x7 } } }`
//! - `look`: the window's look: classic, sepia or light
//!
//! Messages, one key each: `select = "YYYY-MM-DD" | "today"` shows that day;
//! `shift = 1 | -1` the next or previous day; `month = 1 | -1` the same day
//! of the next or previous month; `toggle = id` a task done or open again;
//! `cycle = id` open, important, in progress, open; `add = "title"` a task
//! for the day shown (`inbox = true`: with no date); `remove = id`; `look =
//! "classic" | "sepia" | "light"`.
//!
//! The file is `$XDG_DATA_HOME/moonframe/<profile>/planner.json` (by default
//! under `~/.local/share`), started with sample items the first time. Day
//! and month names are English.

mod dates;
mod messages;
mod store;
mod values;
mod view;

use {
    chrono::Local,
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, profile, to_json},
    serde_json::{Map, Value},
    std::{
        env,
        ffi::OsString,
        path::PathBuf,
        time::{Duration, Instant},
    },
};

/// How often the clock is looked at, for a new day or a task come due.
const LOOK: Duration = Duration::from_secs(30);

/// The profile when the app runs as none, as the Python provider took it.
const PROFILE: &str = "daybook";

pub fn native() -> Box<dyn Native> {
    Box::new(Provider::default())
}

#[derive(Default)]
struct Provider {
    /// The file, when not the profile's own (tests).
    file: Option<PathBuf>,
    planner: Option<Planner>,
    /// What was published last.
    last: Option<String>,
    looked: Option<Instant>,
}

impl Provider {
    fn publish(&mut self, host: &Handle) {
        let Some(planner) = &mut self.planner else {
            return;
        };
        self.looked = Some(Instant::now());
        let value = planner.published(Local::now().naive_local());
        let line = value.to_string();
        if self.last.as_ref() != Some(&line) {
            host.publish_json(&value);
            self.last = Some(line);
        }
    }
}

impl Native for Provider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let file = self.file.clone().unwrap_or_else(file);
        self.planner = Some(Planner::open(file, Local::now().date_naive()));
        self.publish(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        if self.looked.is_none_or(|looked| looked.elapsed() >= LOOK) {
            self.publish(host);
        }
    }

    fn receive(&mut self, host: &Handle, _from: &str, message: Data) {
        let Some(planner) = &mut self.planner else {
            return;
        };
        if planner.receive(&to_json(&message), Local::now().date_naive()) {
            // As before, a planner that cannot be written keeps running.
            let _ = store::save(&planner.file, &planner.store);
        }
        self.publish(host);
    }
}

/// `$XDG_DATA_HOME/moonframe/<profile>/planner.json`. The profile is cleaned
/// byte by byte, as the Python provider cleaned it, so a non-ASCII name
/// maps to the folder it always did.
fn file() -> PathBuf {
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() || path.to_string_lossy().starts_with('/'))
        .unwrap_or_else(|| {
            let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"));
            PathBuf::from(home.unwrap_or_default()).join(".local/share")
        });
    let name = profile()
        .map(OsString::from)
        .or_else(|| env::var_os("MOONFRAME_PROFILE"))
        .unwrap_or_else(|| PROFILE.into());
    data.join("moonframe")
        .join(folder_name(name.as_encoded_bytes()))
        .join("planner.json")
}

/// A profile as a folder name: anything but `A-Za-z0-9_-` becomes `-`, one
/// for each byte.
fn folder_name(profile: &[u8]) -> String {
    profile
        .iter()
        .map(|&b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' => char::from(b),
            _ => '-',
        })
        .collect()
}

/// The planner and the day shown, which is not kept.
struct Planner {
    file: PathBuf,
    store: Map<String, Value>,
    selected: Option<String>,
}

impl Planner {
    fn open(file: PathBuf, today: chrono::NaiveDate) -> Self {
        let store = store::load(&file, today);
        Self {
            file,
            store,
            selected: None,
        }
    }
}

#[cfg(test)]
mod tests;
