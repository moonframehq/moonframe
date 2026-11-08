//! When a scheduled task runs next. Tasks run while the app is open; one
//! missed while it was closed waits for its next time.

use {
    crate::value::display,
    chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone, Timelike},
    serde_json::{Map, Value, json},
};

const DAY: f64 = 86_400.0;

fn parts(task: &Map<String, Value>) -> (u32, u32) {
    let time = time_of(task);
    let mut numbers = time.split(':').map(|part| part.parse().unwrap_or(0));
    (numbers.next().unwrap_or(0), numbers.next().unwrap_or(0))
}

fn time_of(task: &Map<String, Value>) -> String {
    let time = display(task.get("time").unwrap_or(&Value::Null));
    if time.is_empty() {
        "09:00".to_owned()
    } else {
        time
    }
}

fn weekday_of(task: &Map<String, Value>) -> u64 {
    task.get("weekday").and_then(Value::as_u64).unwrap_or(1)
}

fn cadence_of(task: &Map<String, Value>) -> &str {
    task.get("cadence")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn local(seconds: f64) -> Option<DateTime<Local>> {
    #[allow(clippy::cast_possible_truncation, reason = "whole seconds of a date")]
    Local.timestamp_opt(seconds.floor() as i64, 0).earliest()
}

/// `moment`'s day at `hours:minutes`, on the wall clock.
fn at(moment: DateTime<Local>, hours: u32, minutes: u32) -> Option<DateTime<Local>> {
    let time = NaiveTime::from_hms_opt(hours, minutes, 0)?;
    moment
        .date_naive()
        .and_time(time)
        .and_local_timezone(Local)
        .earliest()
}

fn seconds(moment: DateTime<Local>) -> f64 {
    #[allow(clippy::cast_precision_loss, reason = "seconds of a date fit")]
    let whole = moment.timestamp() as f64;
    whole
}

/// The next time `task` runs after `after`, in Unix seconds.
pub(crate) fn next_run(task: &Map<String, Value>, after: f64) -> f64 {
    let Some(from) = local(after) else {
        return after + DAY;
    };
    let (hours, minutes) = parts(task);
    let cadence = cadence_of(task);
    if cadence == "hourly" {
        let next = from
            .with_minute(minutes)
            .and_then(|time| time.with_second(0));
        let Some(next) = next else {
            return after + DAY;
        };
        return seconds(if seconds(next) <= after {
            next + Duration::hours(1)
        } else {
            next
        });
    }
    for days in 0..8 {
        let Some(moment) =
            local(after + f64::from(days) * DAY).and_then(|day| at(day, hours, minutes))
        else {
            continue;
        };
        let weekday = u64::from(moment.weekday().num_days_from_sunday());
        if seconds(moment) <= after
            || (cadence == "weekdays" && (weekday == 0 || weekday == 6))
            || (cadence == "weekly" && weekday != weekday_of(task))
        {
            continue;
        }
        return seconds(moment);
    }
    after + DAY
}

/// At launch: what came due while the app was closed waits for its next
/// time. Says whether anything moved.
pub(crate) fn skip_missed(tasks: &mut [Map<String, Value>], now: f64) -> bool {
    let mut moved = false;
    for task in tasks {
        let due = task.get("next").and_then(Value::as_f64);
        if due.is_some_and(|due| due != 0.0 && due <= now) {
            let next = next_run(task, now);
            task.insert("next".to_owned(), json!(next));
            moved = true;
        }
    }
    moved
}

const WEEKDAYS: [&str; 7] = [
    "Sundays",
    "Mondays",
    "Tuesdays",
    "Wednesdays",
    "Thursdays",
    "Fridays",
    "Saturdays",
];

/// The task's timing in words.
pub(crate) fn describe(task: &Map<String, Value>) -> String {
    let time = time_of(task);
    match cadence_of(task) {
        "hourly" => {
            let minutes = time.split(':').nth(1).unwrap_or("00");
            format!("Every hour at :{minutes}")
        },
        "weekdays" => format!("Weekdays at {time}"),
        "weekly" => {
            let day = usize::try_from(weekday_of(task)).unwrap_or(1);
            let name = WEEKDAYS.get(day).copied().unwrap_or("Mondays");
            format!("{name} at {time}")
        },
        _ => format!("Every day at {time}"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn task(value: &Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    fn noon() -> f64 {
        seconds(Local.with_ymd_and_hms(2026, 11, 4, 12, 0, 0).unwrap())
    }

    #[test]
    fn a_task_that_came_due_while_the_app_was_closed_waits_for_its_next_time() {
        let now = noon();
        let mut tasks = vec![
            task(&json!({ "cadence": "daily", "time": "09:00", "next": now - 3600.0 })),
            task(&json!({ "cadence": "daily", "time": "09:00", "next": now + 3600.0 })),
        ];
        assert!(skip_missed(&mut tasks, now));
        let next = tasks[0]["next"].as_f64().unwrap();
        assert_eq!(next, next_run(&tasks[0], now));
        assert!(next > now);
        assert_eq!(tasks[1]["next"].as_f64().unwrap(), now + 3600.0);
        assert!(!skip_missed(&mut tasks[1..], now));
    }

    #[test]
    fn a_daily_task_is_today_or_tomorrow() {
        let now = noon();
        let later = next_run(&task(&json!({ "cadence": "daily", "time": "15:30" })), now);
        assert_eq!(later, now + 3.5 * 3600.0);
        let earlier = next_run(&task(&json!({ "cadence": "daily", "time": "09:00" })), now);
        assert_eq!(earlier, now + 21.0 * 3600.0);
    }

    #[test]
    fn an_hourly_task_keeps_its_minute() {
        let now = noon();
        let task = task(&json!({ "cadence": "hourly", "time": "09:15" }));
        assert_eq!(next_run(&task, now), now + 900.0);
        assert_eq!(next_run(&task, now + 900.0), now + 900.0 + 3600.0);
    }

    #[test]
    fn weekdays_skip_the_weekend_and_weekly_finds_its_day() {
        // 2026-11-06 is a Friday.
        let friday = seconds(Local.with_ymd_and_hms(2026, 11, 6, 12, 0, 0).unwrap());
        let next = next_run(
            &task(&json!({ "cadence": "weekdays", "time": "09:00" })),
            friday,
        );
        assert_eq!(local(next).unwrap().weekday(), chrono::Weekday::Mon);
        let sunday = task(&json!({ "cadence": "weekly", "time": "09:00", "weekday": 0 }));
        let next = next_run(&sunday, friday);
        assert_eq!(local(next).unwrap().weekday(), chrono::Weekday::Sun);
    }

    #[test]
    fn timing_reads_as_words() {
        let says = |value: Value| describe(&task(&value));
        assert_eq!(
            says(json!({ "cadence": "daily", "time": "03:00" })),
            "Every day at 03:00"
        );
        assert_eq!(
            says(json!({ "cadence": "hourly", "time": "03:20" })),
            "Every hour at :20"
        );
        assert_eq!(
            says(json!({ "cadence": "weekly", "time": "08:00", "weekday": 3 })),
            "Wednesdays at 08:00"
        );
        assert_eq!(says(json!({ "cadence": "weekdays" })), "Weekdays at 09:00");
    }
}
