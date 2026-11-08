use {
    super::*,
    crate::{Config, rpc::Rpc},
    serde_json::json,
};

fn message(value: Value) -> Entry {
    Entry {
        id: value["id"].as_str().map(str::to_owned),
        message: Some(value),
        ..Entry::default()
    }
}

fn user(text: &str, stamp: i64) -> Entry {
    let mut entry = message(json!({ "role": "user", "content": text, "timestamp": stamp }));
    entry.id = Some(format!("u{stamp}"));
    entry
}

fn tool_call(id: &str, name: &str, args: &Value, stamp: i64) -> Entry {
    message(json!({
        "role": "assistant",
        "timestamp": stamp,
        "content": [{ "type": "toolCall", "id": id, "name": name, "arguments": args }],
    }))
}

fn result(id: &str, details: &Value, stamp: i64) -> Entry {
    message(json!({
        "role": "toolResult", "toolCallId": id, "timestamp": stamp,
        "content": [{ "type": "text", "text": "output" }], "details": details,
    }))
}

fn answer(text: &str, stamp: i64) -> Entry {
    message(json!({
        "role": "assistant", "timestamp": stamp,
        "content": [{ "type": "text", "text": text }],
    }))
}

fn built(
    entries: &[Entry],
    turns: &Map<String, Value>,
    open: Option<&HashSet<String>>,
) -> Vec<Value> {
    let context = Context {
        cwd: "/code",
        live: None,
        turns,
        expanded: open,
        now: 2000.0,
    };
    let items = timeline(entries, &context).into_iter();
    items.map(Value::Object).collect()
}

fn run_of_three() -> Vec<Entry> {
    let patch = json!({ "patch": "--- a\n+++ b\n-old\n+new\n+more\n" });
    vec![
        user("fix it", 1_000_000),
        tool_call(
            "t1",
            "read",
            &json!({ "path": "/code/src/a.js" }),
            1_001_000,
        ),
        result("t1", &Value::Null, 1_002_000),
        tool_call("t2", "edit", &json!({ "path": "src/a.js" }), 1_003_000),
        result("t2", &patch, 1_004_000),
        tool_call(
            "t3",
            "bash",
            &json!({ "command": "echo   hi\n there" }),
            1_005_000,
        ),
        result("t3", &Value::Null, 1_006_000),
        answer("Done.", 1_010_000),
    ]
}

#[test]
fn a_run_folds_under_one_line_and_keeps_its_last_words() {
    let items = built(&run_of_three(), &Map::new(), None);
    let kinds: Vec<&str> = items
        .iter()
        .map(|item| item["k"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(kinds, ["user", "run", "tool", "tool", "tool", "text"]);
    assert_eq!(items[0]["entry"], "u1000000");
    assert_eq!(
        items[1]["summary"],
        "Explored 1 file, edited 1 file, ran 1 command"
    );
    assert_eq!(items[1]["label"], "Worked for 10s");
    assert_eq!(items[1]["steps"], 3);
    assert_eq!(items[2]["title"], "Read src/a.js");
    assert_eq!(items[3]["title"], "Edited src/a.js");
    assert_eq!(items[3]["added"], 2);
    assert_eq!(items[3]["deleted"], 1);
    assert_eq!(items[4]["title"], "Ran echo hi there");
    assert_eq!(items[5]["answer"], true);
    assert!(items[2]["output"].is_null(), "output waits to be asked for");
}

#[test]
fn an_open_row_brings_its_output_and_patch() {
    let open: HashSet<String> = ["t2".to_owned()].into();
    let items = built(&run_of_three(), &Map::new(), Some(&open));
    assert_eq!(items[3]["open"], true);
    assert_eq!(items[3]["output"], "output");
    assert!(
        items[3]["patch"]
            .as_str()
            .is_some_and(|patch| patch.contains("+new"))
    );
}

#[test]
fn a_run_s_changes_follow_it() {
    let turn = json!({
        "files": [{ "path": "a", "added": 1, "deleted": 0 }],
        "added": 1, "deleted": 0, "before": "b", "after": "c",
    });
    let turns: Map<String, Value> = [("1000000".to_owned(), turn)].into_iter().collect();
    let items = built(&run_of_three(), &turns, None);
    let last = items.last().cloned().unwrap_or_default();
    assert_eq!(last["k"], "changes");
    assert_eq!(last["count"], 1);
    assert_eq!(last["from"], "b");
}

#[test]
fn a_run_with_no_steps_has_no_line() {
    let items = built(
        &[user("hi", 1000), answer("hello", 2000)],
        &Map::new(),
        None,
    );
    let kinds: Vec<&str> = items
        .iter()
        .map(|item| item["k"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(kinds, ["user", "text"]);
}

#[test]
fn errors_and_stops_show_as_themselves() {
    let failed = message(json!({
        "role": "assistant", "timestamp": 2000, "content": [],
        "stopReason": "error", "errorMessage": "rate limited",
    }));
    let stopped = message(json!({
        "role": "assistant", "timestamp": 3000, "content": [], "stopReason": "aborted",
    }));
    let items = built(&[user("hi", 1000), failed, stopped], &Map::new(), None);
    assert!(
        items
            .iter()
            .any(|item| item["k"] == "error" && item["text"] == "rate limited")
    );
    assert!(
        items
            .iter()
            .any(|item| item["k"] == "note" && item["text"] == "Stopped")
    );
}

#[test]
fn shell_output_and_notes_stand_on_their_own() {
    let shell = message(json!({
        "role": "bashExecution", "command": "ls", "exitCode": 0, "output": "a\n", "timestamp": 5,
    }));
    let note = Entry {
        id: Some("n".to_owned()),
        note: Some("Compacted 1k tokens of context".to_owned()),
        ..Entry::default()
    };
    let items = built(&[shell, note], &Map::new(), None);
    assert_eq!(items[0]["k"], "bash");
    assert_eq!(items[0]["code"], 0);
    assert_eq!(items[1]["tone"], "muted");
}

#[test]
fn a_live_run_shows_as_working_with_what_streams() {
    let dir = std::env::temp_dir();
    let config = Config {
        agent_dir: dir.clone(),
        data_dir: dir.clone(),
        home: dir.clone(),
        pi: None,
        login_shell: false,
        env: Vec::new(),
    };
    let runtime = crate::runtime::Runtime::new(&config);
    let rpc = Rpc::spawn(crate::rpc::Start {
        program: Path::new("cat"),
        args: &[],
        cwd: &dir.to_string_lossy(),
        runtime: &runtime,
        on_event: Box::new(|_| {}),
        on_exit: Box::new(|_, _| {}),
    });
    let Ok(rpc) = rpc else {
        return;
    };
    let mut live = Live::new(
        "t".to_owned(),
        "/code".to_owned(),
        std::sync::Arc::clone(&rpc),
    );
    live.running = true;
    live.partial = Some(json!({
        "role": "assistant", "timestamp": 1_001_000,
        "content": [{ "type": "text", "text": "so far" }],
    }));
    let entries = [user("go", 1_000_000)];
    let turns = Map::new();
    let context = Context {
        cwd: "/code",
        live: Some(&live),
        turns: &turns,
        expanded: None,
        now: 1070.0,
    };
    let items = timeline(&entries, &context);
    assert_eq!(items[1]["label"], "Working for 1m 10s");
    assert_eq!(items[1]["running"], true);
    assert_eq!(items[2]["streaming"], true);
    assert_eq!(items[2]["text"], "so far");
    rpc.stop();
}

#[test]
fn durations_read_as_a_person_would_say_them() {
    assert_eq!(duration(0.2), "1s");
    assert_eq!(duration(59.0), "59s");
    assert_eq!(duration(61.0), "1m 1s");
    assert_eq!(duration(3700.0), "1h 1m");
}

#[test]
fn context_and_cost_come_from_usage() {
    let priced = message(json!({
        "role": "assistant",
        "usage": { "input": 100, "output": 50, "cacheRead": 25, "cacheWrite": 25,
            "cost": { "total": 0.123_46 } },
    }));
    let free = message(json!({ "role": "assistant", "usage": { "input": 0 } }));
    let entries = [priced, free];
    let used = context_use(&entries, Some(400.0));
    assert_eq!(used["tokens"], 200.0);
    assert_eq!(used["used"], 0.5);
    assert_eq!(total_cost(&entries), 0.1235);
    assert_eq!(context_use(&[], None), Value::Null);
}
