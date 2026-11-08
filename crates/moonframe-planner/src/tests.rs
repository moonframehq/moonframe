//! The planner's logic, on fixed days, and the provider driven as the app
//! drives it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use {
    super::*,
    crate::{
        dates::{civil, day, label},
        values::{integer, text, truthy},
    },
    chrono::{NaiveDate, NaiveDateTime},
    moonframe_providers::Event,
    serde_json::json,
    std::{
        fs,
        path::Path,
        sync::{Arc, Mutex},
    },
    tempfile::{TempDir, tempdir},
};

fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
}

fn at(text: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap()
}

/// A planner on `store`, in a fresh folder.
fn planner(store: Value) -> (TempDir, Planner) {
    let temp = tempdir().unwrap();
    let file = temp.path().join("daybook").join("planner.json");
    let planner = Planner {
        file,
        store: store.as_object().cloned().unwrap(),
        selected: None,
    };
    (temp, planner)
}

fn with_items(items: Value) -> (TempDir, Planner) {
    planner(json!({ "items": items, "next": 10 }))
}

fn item<'a>(published: &'a Value, id: &str) -> &'a Value {
    let items = published["items"].as_array().unwrap();
    items.iter().find(|item| item["id"] == id).unwrap()
}

#[test]
fn values_read_as_the_python_read_them() {
    let texts = [
        (json!(null), ""),
        (json!(true), "1"),
        (json!(false), "0"),
        (json!(7), "7"),
        (json!(1.0), "1"),
        (json!(1.5), "1.5"),
        (json!(-2.5), "-2.5"),
        (json!(1e20), "1e+20"),
        (json!(1e15), "1e+15"),
        (json!(1e14), "100000000000000"),
        (json!(0.0001), "0.0001"),
        (json!(1e-5), "1e-05"),
        (json!(0.1 + 0.2), "0.3"),
        (json!(123_456_789_012_345_678.0), "1.23456789012346e+17"),
        (json!([]), "\0ref"),
        (json!({}), "\0ref"),
    ];
    for (value, want) in texts {
        assert_eq!(text(&value), want, "{value}");
    }
    for (value, want) in [
        (json!(""), false),
        (json!("0"), false),
        (json!("no"), true),
        (json!(0), false),
        (json!(0.0), false),
        (json!(2), true),
        (json!([]), true),
        (json!({}), true),
        (json!(null), false),
    ] {
        assert_eq!(truthy(&value), want, "{value}");
    }
    for (value, want) in [
        (json!(1), Some(1)),
        (json!(-1), Some(-1)),
        (json!("2"), Some(2)),
        (json!("2\n"), Some(2)),
        (json!(3.0), Some(3)),
        (json!(true), Some(1)),
        (json!(1.5), None),
        (json!("x"), None),
        (json!("-"), None),
        (json!({}), None),
        (json!("99999999999999999999"), Some(i64::MAX)),
    ] {
        assert_eq!(integer(&value), want, "{value}");
    }
}

#[test]
fn dates_roll_over_as_mktime_does() {
    let today = date("2026-10-31");
    assert_eq!(day(today, None, 0), "2026-10-31");
    assert_eq!(day(today, None, 1), "2026-11-01");
    assert_eq!(day(today, Some("2026-03-01"), -1), "2026-02-28");
    assert_eq!(day(today, Some("2026-13-01"), 0), "2027-01-01");
    assert_eq!(day(today, Some("2026-02-30"), 0), "2026-03-02");
    assert_eq!(day(today, Some("2026-01-00"), 0), "2025-12-31");
    assert_eq!(
        day(today, Some("2026-1-1"), 0),
        "2026-10-31",
        "no date is today"
    );
    assert_eq!(day(today, Some("2026-01-05\n"), 0), "2026-01-05");
    assert_eq!(civil(2024, 2, 29), date("2024-02-29"));
    assert_eq!(label("2026-10-31", "%a, %b %-d"), "Sat, Oct 31");
    assert_eq!(label("2026-10-31", "%Y. %B %-d."), "2026. October 31.");
    assert_eq!(label("nope", "%A"), "");
}

#[test]
fn dates_past_four_digit_years_stay_dates() {
    // Python's mktime overflowed to the epoch, or made a five-digit year that
    // no later message could read and that stopped the provider.
    let today = date("2026-10-31");
    for offset in [i64::MAX, i64::MIN, 4_000_000] {
        let shifted = day(today, None, offset);
        assert!(dates::split(&shifted).is_some(), "{shifted}");
    }
    let (_temp, mut planner) = with_items(json!([]));
    planner.receive(&json!({ "shift": 4_000_000 }), today);
    let published = planner.published(at("2026-10-31 09:00"));
    assert!(published["month"]["weeks"].as_array().unwrap().len() >= 4);
    planner.receive(&json!({ "select": "today" }), today);
    assert!(!planner.receive(&json!({ "month": i64::MAX }), today));
    assert_eq!(planner.selected.as_deref(), Some("2026-10-31"));
}

#[test]
fn saves_the_python_format_byte_for_byte() {
    let (_temp, planner) = planner(json!({
        "items": [{"id": "i1", "kind": "task", "title": "Caf\u{e9} \u{2b50} \"q\"\n",
                   "tags": [], "done": true, "time": "07:30"}],
        "folders": [], "next": 2, "tags": [{"name": "Work", "color": "#3b6fd8"}], "look": {},
    }));
    store::save(&planner.file, &planner.store).unwrap();
    let python = "{\n   \"folders\" : [],\n   \"items\" : [\n      {\n         \"done\" : true,\n         \"id\" : \"i1\",\n         \"kind\" : \"task\",\n         \"tags\" : [],\n         \"time\" : \"07:30\",\n         \"title\" : \"Caf\u{e9} \u{2b50} \\\"q\\\"\\n\"\n      }\n   ],\n   \"look\" : {},\n   \"next\" : 2,\n   \"tags\" : [\n      {\n         \"color\" : \"#3b6fd8\",\n         \"name\" : \"Work\"\n      }\n   ]\n}\n";
    assert_eq!(fs::read_to_string(&planner.file).unwrap(), python);
    assert!(!Path::new(&format!("{}.tmp", planner.file.display())).exists());
    let reread = store::load(&planner.file, date("2026-10-31"), false);
    assert_eq!(reread, planner.store);
}

#[test]
fn a_new_planner_starts_empty() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("planner.json");
    let fresh = store::load(&file, date("2026-10-31"), false);
    assert_eq!(fresh, store::empty());
    assert_eq!(fresh["items"], json!([]));
    assert_eq!(fresh["next"], 1);
    assert!(file.is_file(), "it was written");
    assert_eq!(store::load(&file, date("2026-11-01"), false), fresh);
}

#[test]
fn a_broken_file_is_kept_before_a_new_planner_starts() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("planner.json");
    let kept = temp.path().join("planner.json.broken");
    for broken in ["not json", "[]", "{\"items\": {}}"] {
        fs::write(&file, broken).unwrap();
        assert_eq!(
            store::load(&file, date("2026-10-31"), false),
            store::empty(),
            "{broken}"
        );
        assert_eq!(fs::read_to_string(&kept).unwrap(), broken);
    }
}

#[test]
fn the_sample_is_there_when_asked_for() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("planner.json");
    let today = date("2026-10-31");
    let fresh = store::load(&file, today, true);
    let items = fresh["items"].as_array().unwrap();
    assert_eq!(fresh["next"], json!(items.len() + 1));
    let ids: std::collections::BTreeSet<_> = items.iter().map(|i| i["id"].to_string()).collect();
    assert_eq!(ids.len(), items.len(), "ids are unique");
    assert!(
        items
            .iter()
            .any(|i| i["title"] == "Book the train to Porto" && i["date"] == "2026-10-30")
    );
    assert!(
        items
            .iter()
            .any(|i| i["title"] == "Climbing" && i["date"] == "2026-11-04")
    );
    assert_eq!(
        store::load(&file, today, false),
        fresh,
        "it was written, and is kept"
    );
}

#[test]
fn items_show_their_state_and_when_they_are_due() {
    let (_temp, mut planner) = with_items(json!([
        {"id": "late", "kind": "task", "title": "a", "date": "2026-10-30"},
        {"id": "soon", "kind": "task", "title": "b", "date": "2026-10-31", "time": "07:30"},
        {"id": "later", "kind": "task", "title": "c", "date": "2026-10-31", "time": "17:00", "state": "important"},
        {"id": "done", "kind": "task", "title": "d", "date": "2026-10-30", "done": 1},
        {"id": "event", "kind": "event", "title": "e", "date": "2026-10-29", "time": "09:00"},
        {"id": "note", "kind": "note", "title": "f", "date": "2026-10-29", "time": "09:00"},
        {"id": "inbox", "kind": "task", "title": "g", "extra": {"kept": true}},
        "not an item",
    ]));
    let published = planner.published(at("2026-10-31 09:15"));
    let late = item(&published, "late");
    assert_eq!(late["overdue"], true);
    assert_eq!(late["due"], false, "no time");
    assert_eq!(late["state"], "open");
    assert_eq!(late["when"], "Fri, Oct 30");
    assert_eq!(late["tags"], json!([]));
    let soon = item(&published, "soon");
    assert_eq!(
        (soon["due"].clone(), soon["time"].clone()),
        (json!(true), json!("7:30"))
    );
    assert_eq!(soon["overdue"], false);
    let later = item(&published, "later");
    assert_eq!(
        (later["due"].clone(), later["state"].clone()),
        (json!(false), json!("important"))
    );
    let done = item(&published, "done");
    assert_eq!(
        (done["state"].clone(), done["done"].clone()),
        (json!("done"), json!(true))
    );
    assert_eq!(done["overdue"], false);
    assert_eq!(item(&published, "event")["due"], true);
    assert_eq!(item(&published, "event")["overdue"], false, "only tasks");
    assert_eq!(item(&published, "note")["due"], false);
    let inbox = item(&published, "inbox");
    assert_eq!(inbox["when"], Value::Null);
    assert_eq!(inbox["extra"], json!({"kept": true}));
    assert_eq!(
        published["items"].as_array().unwrap().len(),
        7,
        "only tables"
    );
}

#[test]
fn the_strip_and_month_surround_the_selected_day() {
    let (_temp, mut planner) = with_items(json!([
        {"id": "a", "kind": "task", "title": "a", "date": "2026-11-02"},
        {"id": "b", "kind": "event", "title": "b", "date": "2026-11-02"},
        {"id": "c", "kind": "task", "title": "c", "date": "2026-11-03", "done": true},
        {"id": "d", "kind": "note", "title": "d", "date": "2026-11-04"},
    ]));
    let published = planner.published(at("2026-10-31 20:00"));
    assert_eq!(published["today"], "2026-10-31");
    assert_eq!(published["selected"], "2026-10-31");
    assert_eq!(published["title"], "Today");
    assert_eq!(published["subtitle"], "2026. October 31.");
    assert_eq!(published["greeting"], "Good evening");
    assert_eq!(published["look"], "classic");
    assert_eq!(published["available"], true);
    let days = published["days"].as_array().unwrap();
    assert_eq!(days.len(), 14);
    assert_eq!(
        days[0],
        json!({"id": "2026-10-31", "day": "31", "weekday": "Sat", "today": true, "dot": false})
    );
    assert_eq!(days[2]["dot"], true);
    assert_eq!(days[3]["dot"], false, "done");
    assert_eq!(days[4]["dot"], false, "a note");
    // October 2026 starts on a Thursday and needs five weeks.
    let month = &published["month"];
    assert_eq!(month["title"], "October 2026");
    let weeks = month["weeks"].as_array().unwrap();
    assert_eq!(weeks.len(), 5);
    assert_eq!(
        weeks[0][0],
        json!({"id": "2026-09-28", "day": "28", "today": false, "outside": true, "count": 0})
    );
    assert_eq!(weeks[4][6]["id"], "2026-11-01");
    planner.receive(&json!({ "select": "2026-11-02" }), date("2026-10-31"));
    let published = planner.published(at("2026-10-31 04:00"));
    assert_eq!(published["title"], "Monday");
    assert_eq!(published["greeting"], "Good night");
    // November 2026 starts on a Sunday and ends on a Monday: six weeks.
    let weeks = published["month"]["weeks"].as_array().unwrap();
    assert_eq!(weeks.len(), 6);
    assert_eq!(weeks[0][0]["id"], "2026-10-26");
    assert_eq!(weeks[5][0]["id"], "2026-11-30");
    assert_eq!(
        weeks[1][0],
        json!({"id": "2026-11-02", "day": "2", "today": false, "outside": false, "count": 2})
    );
    // Four when it fits them: February 2027 starts on a Monday.
    planner.receive(&json!({ "select": "2027-02-10" }), date("2026-10-31"));
    let published = planner.published(at("2026-10-31 12:00"));
    assert_eq!(published["month"]["weeks"].as_array().unwrap().len(), 4);
    assert_eq!(published["greeting"], "Good afternoon");
}

#[test]
fn moving_between_days_and_months() {
    let today = date("2026-10-31");
    let (_temp, mut planner) = with_items(json!([]));
    let selected = |planner: &mut Planner, msg: Value| {
        assert!(!planner.receive(&msg, today), "nothing to save");
        planner.selected.clone().unwrap()
    };
    assert_eq!(selected(&mut planner, json!({ "shift": 1 })), "2026-11-01");
    assert_eq!(
        selected(&mut planner, json!({ "shift": "-2" })),
        "2026-10-30"
    );
    assert_eq!(
        selected(&mut planner, json!({ "select": "2026-01-31" })),
        "2026-01-31"
    );
    assert_eq!(selected(&mut planner, json!({ "month": 1 })), "2026-02-28");
    assert_eq!(selected(&mut planner, json!({ "month": -2 })), "2025-12-28");
    assert_eq!(
        selected(&mut planner, json!({ "month": 1.0 })),
        "2026-01-28"
    );
    assert_eq!(
        selected(&mut planner, json!({ "select": "today" })),
        "2026-10-31"
    );
    assert_eq!(
        selected(&mut planner, json!({ "select": "garbage" })),
        "2026-10-31"
    );
    assert_eq!(
        selected(&mut planner, json!({ "select": "2024-01-31" })),
        "2024-01-31"
    );
    assert_eq!(selected(&mut planner, json!({ "month": 1 })), "2024-02-29");
}

#[test]
fn the_31st_of_a_shorter_month_is_its_last_day() {
    // The Python said so but took the 28th: March 31 became April 28.
    let today = date("2026-10-31");
    let (_temp, mut planner) = with_items(json!([]));
    planner.receive(&json!({ "select": "2026-03-31" }), today);
    planner.receive(&json!({ "month": 1 }), today);
    assert_eq!(planner.selected.as_deref(), Some("2026-04-30"));
    planner.receive(&json!({ "select": "2026-08-31" }), today);
    planner.receive(&json!({ "month": 1 }), today);
    assert_eq!(planner.selected.as_deref(), Some("2026-09-30"));
}

#[test]
fn tasks_are_added_toggled_cycled_and_removed() {
    let today = date("2026-10-31");
    let (_temp, mut planner) = with_items(json!([
        {"id": "i1", "kind": "task", "title": "a"},
        {"id": 7, "kind": "task", "title": "b", "state": "progress"},
    ]));
    planner.published(at("2026-10-31 09:00"));
    assert!(planner.receive(&json!({ "toggle": "i1" }), today));
    assert_eq!(planner.items().next().unwrap()["done"], true);
    assert!(planner.receive(&json!({ "toggle": "i1" }), today));
    assert_eq!(planner.items().next().unwrap()["done"], false);
    assert!(!planner.receive(&json!({ "toggle": "nobody" }), today));
    let states: Vec<_> = (0..4)
        .map(|_| {
            assert!(planner.receive(&json!({ "cycle": "i1" }), today));
            planner.items().next().unwrap()["state"].clone()
        })
        .collect();
    assert_eq!(states, ["important", "progress", "open", "important"]);
    // A number finds the id written as a number, as the Python's text did.
    assert!(planner.receive(&json!({ "cycle": 7.0 }), today));
    assert_eq!(planner.items().nth(1).unwrap()["state"], "open");

    assert!(planner.receive(&json!({ "add": "  Buy milk  " }), today));
    assert!(planner.receive(&json!({ "add": 42, "inbox": true }), today));
    assert!(!planner.receive(&json!({ "add": "   " }), today), "blank");
    assert!(!planner.receive(&json!({ "add": true }), today), "not text");
    let added: Vec<_> = planner.items().skip(2).cloned().collect();
    assert_eq!(
        added[0],
        *json!({"id": "i10", "kind": "task", "title": "Buy milk", "date": "2026-10-31"})
            .as_object()
            .unwrap()
    );
    assert_eq!(
        added[1],
        *json!({"id": "i11", "kind": "task", "title": "42"})
            .as_object()
            .unwrap()
    );
    assert_eq!(planner.store["next"], 12);
    assert!(planner.receive(&json!({ "add": "x".repeat(300) }), today));
    let long = planner.items().last().unwrap()["title"]
        .as_str()
        .unwrap()
        .chars()
        .count();
    assert_eq!(long, 200);

    assert!(planner.receive(&json!({ "remove": "i10" }), today));
    assert!(planner.items().all(|item| item["id"] != "i10"));
    assert!(planner.receive(&json!({ "remove": 7 }), today));
    assert!(planner.items().all(|item| item["id"] != 7));
}

#[test]
fn ids_come_from_whatever_next_holds() {
    let today = date("2026-10-31");
    for (next, id, after) in [
        (json!(null), "i0", json!(1)),
        (json!(5.0), "i5", json!(6)),
        (json!("3"), "i3", json!(4)),
        (json!("x"), "ix", json!(1)),
    ] {
        let (_temp, mut planner) = planner(json!({ "items": [], "next": next }));
        if next.is_null() {
            planner.store.remove("next");
        }
        planner.receive(&json!({ "add": "t" }), today);
        assert_eq!(planner.items().next().unwrap()["id"], id, "{next}");
        assert_eq!(planner.store["next"], after, "{next}");
    }
}

#[test]
fn the_look_is_kept_and_odd_messages_do_nothing() {
    let today = date("2026-10-31");
    let (_temp, mut planner) = with_items(json!([]));
    assert!(planner.receive(&json!({ "look": "sepia" }), today));
    assert_eq!(planner.published(at("2026-10-31 09:00"))["look"], "sepia");
    assert!(!planner.receive(&json!({ "look": "neon" }), today));
    for odd in [
        json!({}),
        json!([]),
        json!("today"),
        json!({ "select": true }),
        json!({ "shift": "x" }),
    ] {
        let before = planner.selected.clone();
        assert!(!planner.receive(&odd, today), "{odd}");
        assert_eq!(planner.selected, before, "{odd}");
    }
}

#[test]
fn profiles_become_the_folder_names_they_always_were() {
    assert_eq!(folder_name(b"daybook"), "daybook");
    assert_eq!(folder_name(b"my app_2-x"), "my-app_2-x");
    assert_eq!(folder_name("caf\u{e9}".as_bytes()), "caf--", "one per byte");
}

type Seen = Arc<Mutex<Vec<Event>>>;

fn host() -> (Handle, Seen) {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let events: moonframe_providers::Events = {
        let seen = Arc::clone(&seen);
        Arc::new(move |event| seen.lock().unwrap().push(event))
    };
    (
        Handle::new("planner", Path::new("."), Data::Nil, events),
        seen,
    )
}

fn published(seen: &Seen) -> Vec<Value> {
    let seen = seen.lock().unwrap();
    seen.iter()
        .filter_map(|event| match event {
            Event::Published { data, .. } => Some(to_json(data)),
            Event::Failed { .. } => panic!("the provider failed"),
            _ => None,
        })
        .collect()
}

fn message(value: Value) -> Data {
    moonframe_providers::from_json(&value).unwrap()
}

fn provider(file: &Path) -> Provider {
    Provider {
        file: Some(file.to_owned()),
        ..Provider::default()
    }
}

#[test]
fn the_provider_publishes_and_keeps_what_mods_send() {
    let temp = tempdir().unwrap();
    let file = temp
        .path()
        .join("moonframe")
        .join("daybook")
        .join("planner.json");
    let (handle, seen) = host();
    let mut planner = provider(&file);
    planner.start(&handle).unwrap();
    let first = published(&seen);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["available"], true);
    assert!(file.is_file(), "the planner is written at the first start");
    assert_eq!(first[0]["items"], json!([]), "a new planner is empty");

    // As the inbox mod sends it, and an empty Luau table as the host does.
    planner.receive(
        &handle,
        "inbox",
        message(json!({ "add": "From the inbox", "inbox": true })),
    );
    planner.receive(&handle, "agenda", message(json!({})));
    planner.tick(&handle);
    let after = published(&seen);
    assert_eq!(after.len(), 2, "nothing new, nothing published");
    let items = after[1]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let added = items
        .iter()
        .find(|item| item["title"] == "From the inbox")
        .unwrap();
    assert_eq!(added["when"], Value::Null);

    planner.receive(&handle, "sidebar", message(json!({ "look": "light" })));
    drop(planner);

    // A new run reads it back.
    let (handle, seen) = host();
    let mut again = provider(&file);
    again.start(&handle).unwrap();
    let reread = &published(&seen)[0];
    assert_eq!(reread["look"], "light");
    assert_eq!(reread["items"].as_array().unwrap().len(), 1);
}

#[test]
fn moving_days_publishes_without_writing() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("planner.json");
    let (handle, seen) = host();
    let mut planner = provider(&file);
    planner.start(&handle).unwrap();
    let written = fs::read_to_string(&file).unwrap();
    planner.receive(&handle, "agenda", message(json!({ "shift": 1 })));
    let all = published(&seen);
    assert_eq!(all.len(), 2);
    assert_ne!(all[0]["selected"], all[1]["selected"]);
    assert_eq!(fs::read_to_string(&file).unwrap(), written);
}
