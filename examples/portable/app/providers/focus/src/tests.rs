//! Plain Rust tests: the provider is an ordinary type natively, so they
//! call it directly, with no host and no wasm.

use {super::*, serde_json::json};

fn started(minutes: u64) -> Focus {
    Focus::start(json!({ "minutes": minutes })).expect("valid settings")
}

#[test]
fn settings_are_checked() {
    assert_eq!(
        Focus::start(json!({})).map(|focus| focus.length),
        Ok(25 * 60)
    );
    assert!(Focus::start(json!({ "minutes": 0 })).is_err());
    assert!(Focus::start(json!({ "minutes": "five" })).is_err());
}

#[test]
fn it_counts_only_while_running() {
    let mut focus = started(1);
    assert_eq!(focus.tick().expect("shown once")["clock"], "01:00");
    assert_eq!(focus.tick(), None, "paused: nothing new to publish");
    let shown = focus.receive(json!("start")).expect("an answer");
    assert_eq!(shown["clock"], "01:00");
    let shown = focus.tick().expect("running publishes");
    assert_eq!(shown["clock"], "00:59");
    focus.receive(json!("pause"));
    assert_eq!(focus.tick(), None);
}

#[test]
fn a_finished_session_is_counted_and_starts_over() {
    let mut focus = started(1);
    focus.receive(json!("start"));
    let mut shown = Value::Null;
    for _ in 0..60 {
        shown = focus.tick().expect("running publishes");
    }
    assert_eq!(shown["sessions"], 1);
    assert_eq!(shown["running"], false);
    assert_eq!(shown["clock"], "01:00");
}

#[test]
fn a_new_length_keeps_the_sessions() {
    let mut focus = started(1);
    focus.sessions = 3;
    let shown = focus.receive(json!({ "minutes": 5 })).expect("an answer");
    assert_eq!(shown["clock"], "05:00");
    assert_eq!(shown["sessions"], 3);
    let shown = focus.receive(json!({ "minutes": 0 })).expect("an answer");
    assert_eq!(shown["clock"], "05:00", "a bad length changes nothing");
}

#[test]
fn saved_state_restores_where_it_was() {
    let mut focus = started(1);
    focus.receive(json!("start"));
    focus.tick();
    let saved = focus.save().expect("it saves");
    let mut next = started(25);
    next.restore(&saved).expect("it restores");
    assert_eq!(next.view()["clock"], "00:59");
    assert_eq!(next.view()["running"], true);
    assert!(next.restore("not json").is_err());
}

/// Each top-level key with its JSON kind.
fn shape(value: &Value) -> Vec<(String, &'static str)> {
    let kind = |value: &Value| match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    let mut found: Vec<_> = value
        .as_object()
        .expect("an object")
        .iter()
        .map(|(key, value)| (key.clone(), kind(value)))
        .collect();
    found.sort();
    found
}

/// types.luau's `Published` is checked against published.json (`just
/// provider-types-check`); this holds what the timer publishes to it.
#[test]
fn it_publishes_what_published_json_shows() {
    let example: Value = serde_json::from_str(include_str!("../published.json")).unwrap();
    let mut focus = started(25);
    assert_eq!(shape(&focus.tick().expect("shown once")), shape(&example));
    assert_eq!(
        shape(&focus.receive(json!("start")).expect("an answer")),
        shape(&example)
    );
}
