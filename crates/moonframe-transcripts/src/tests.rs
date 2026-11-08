//! The provider on transcripts laid out as the CLI writes them, driven as
//! Cinder's mods drive it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use {
    super::*,
    moonframe_providers::{Event, from_json},
    serde_json::json,
    std::{
        fs::{self, File},
        path::Path,
        sync::Mutex,
        time::{Duration, SystemTime},
    },
    tempfile::{TempDir, tempdir},
};

/// A `~/.claude/projects` of its own.
struct Fixture {
    temp: TempDir,
    root: String,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempdir().unwrap();
        let root = format!(
            "{}/.claude/projects",
            temp.path().to_string_lossy().replace('\\', "/")
        );
        fs::create_dir_all(&root).unwrap();
        Self { temp, root }
    }

    /// A folder that exists, as a session's `cwd`.
    fn folder(&self, name: &str) -> String {
        let path = format!(
            "{}/{name}",
            self.temp.path().to_string_lossy().replace('\\', "/")
        );
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Writes a transcript, `ago` seconds old; a string is a line as it is.
    fn session(&self, dir: &str, id: &str, lines: &[Value], ago: u64) -> String {
        let text: String = lines
            .iter()
            .map(|line| match line {
                Value::String(raw) => format!("{raw}\n"),
                entry => format!("{entry}\n"),
            })
            .collect();
        self.raw(dir, id, text.as_bytes(), ago)
    }

    fn raw(&self, dir: &str, id: &str, bytes: &[u8], ago: u64) -> String {
        fs::create_dir_all(format!("{}/{dir}", self.root)).unwrap();
        let path = format!("{}/{dir}/{id}.jsonl", self.root);
        fs::write(&path, bytes).unwrap();
        let when = SystemTime::now() - Duration::from_secs(ago);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(when)
            .unwrap();
        path
    }

    fn sessions(&self) -> Sessions {
        let mut sessions = Sessions::new(self.root.clone(), false, Arc::default());
        sessions.scan();
        sessions
    }
}

fn user(text: &str, cwd: &str) -> Value {
    json!({ "type": "user", "cwd": cwd, "gitBranch": "main", "message": { "role": "user", "content": text } })
}

fn said(content: Value) -> Value {
    json!({ "type": "assistant", "message": { "role": "assistant", "content": content } })
}

fn tool_result() -> Value {
    json!({ "type": "user", "message": { "role": "user", "content": [{ "type": "tool_result", "content": "ok" }] } })
}

fn ids(view: &Value) -> Vec<(String, Vec<String>)> {
    view["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| {
            let sessions = group["sessions"].as_array().unwrap();
            let ids = sessions
                .iter()
                .map(|s| s["id"].as_str().unwrap().to_owned())
                .collect();
            (group["name"].as_str().unwrap().to_owned(), ids)
        })
        .collect()
}

#[test]
fn sessions_are_grouped_by_repository_newest_first() {
    let fixture = Fixture::new();
    let app = fixture.folder("code/app");
    let tool = fixture.folder("code/tool");
    let worktree = format!("{app}/.claude/worktrees/agent-1");
    fixture.session("-code-app", "a1", &[user("First in app", &app)], 300);
    fixture.session("-code-tool", "t1", &[user("In tool", &tool)], 100);
    fixture.session(
        "-code-app-wt",
        "a2",
        &[user("In a worktree", &worktree)],
        50,
    );
    // No title at all: not listed, still counted.
    fixture.session("-code-app", "empty", &[tool_result()], 10);
    fs::write(format!("{}/.hidden.jsonl", fixture.root), "").unwrap();
    fs::create_dir_all(format!("{}/.cache", fixture.root)).unwrap();
    fixture.session(".cache", "skipped", &[user("Hidden folder", &app)], 0);
    let view = fixture.sessions().view();
    assert_eq!(view["available"], true);
    assert_eq!(view["source"], "cli");
    assert_eq!(view["total"], 4);
    assert_eq!(ids(&view), vec![
        ("app".to_owned(), vec!["a2".to_owned(), "a1".to_owned()]),
        ("tool".to_owned(), vec!["t1".to_owned()]),
    ]);
    let group = &view["groups"][0];
    // The folder of the group's latest session.
    assert_eq!(group["path"], worktree);
    let session = &group["sessions"][0];
    assert_eq!(session["title"], "In a worktree");
    assert_eq!(session["branch"], "main");
    assert_eq!(session["age"], "now");
    assert_eq!(session["pr"], false);
    // The worktree folder was never made.
    assert_eq!(session["gone"], true);
    assert_eq!(group["sessions"][1]["age"], "5m");
    assert_eq!(group["sessions"][1]["gone"], false);
    assert!(session.get("path").is_none(), "the file stays private");
    assert!(view.get("transcript").is_none());
}

#[test]
fn titles_come_from_the_latest_ai_title_then_a_summary_then_what_was_typed() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    fixture.session(
        "-app",
        "ai",
        &[
            user("Typed", &app),
            // In the CLI's order of keys, which the provider searches.
            json!(r#"{"type":"ai-title","aiTitle":"Old title"}"#),
            json!(r#"{"type":"summary","summary":"A summary"}"#),
            json!(r#"{"type":"ai-title","aiTitle":"  New\n title"}"#),
        ],
        3,
    );
    fixture.session(
        "-app",
        "summary",
        &[
            user("Typed", &app),
            json!(r#"{"type":"summary","summary":"A summary"}"#),
        ],
        2,
    );
    fixture.session(
        "-app",
        "typed",
        &[
            json!({ "type": "user", "isMeta": true, "message": { "content": "meta" } }),
            user("<command-name>/clear</command-name>", &app),
            tool_result(),
            user("What I typed\n  first", &app),
            said(json!([{ "type": "text", "text": "Run gh pr create now" }])),
        ],
        1,
    );
    let view = fixture.sessions().view();
    let titles: Vec<(&str, &str, bool)> = view["groups"][0]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["id"].as_str().unwrap(),
                s["title"].as_str().unwrap(),
                s["pr"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(titles, vec![
        ("typed", "What I typed first", true),
        ("summary", "A summary", false),
        ("ai", "New title", false),
    ]);
}

#[test]
fn a_session_without_a_folder_is_named_after_its_cli_folder() {
    let fixture = Fixture::new();
    fixture.session(
        "-Users-me-code-cinder",
        "s",
        &[json!({ "type": "user", "message": { "content": "Hi" } })],
        0,
    );
    let view = fixture.sessions().view();
    assert_eq!(view["groups"][0]["name"], "cinder");
    assert_eq!(view["groups"][0]["path"], "");
    assert_eq!(view["groups"][0]["sessions"][0]["branch"], "");
    assert_eq!(view["groups"][0]["sessions"][0]["gone"], false);
}

#[test]
fn only_the_newest_sessions_are_listed() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    let many = sessions::MOST + 5;
    for n in 0..many {
        fixture.session("-app", &format!("s{n}"), &[user("Hi", &app)], n as u64 * 60);
    }
    let view = fixture.sessions().view();
    assert_eq!(view["total"], many);
    let listed = &ids(&view)[0].1;
    assert_eq!(listed.len(), sessions::MOST);
    assert_eq!(listed[0], "s0");
}

fn transcript_of(sessions: &mut Sessions, id: &str) -> Value {
    sessions.receive(&json!({ "open": id }));
    sessions.view()["transcript"].clone()
}

#[test]
fn a_transcript_has_the_turns_and_tool_calls() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    let big = "x".repeat(20_000);
    fixture.session("-app", "s", &[
        user("Fix the bug", &app),
        said(json!([{ "type": "thinking", "thinking": "hmm" }])),
        said(json!([{ "type": "text", "text": "Looking." }, { "type": "tool_use", "id": "1", "name": "Read", "input": { "file_path": format!("{app}/src/main.rs") } }])),
        tool_result(),
        said(json!([{ "type": "tool_use", "id": "2", "name": "Bash", "input": { "description": "Run   the tests", "command": "cargo test" } }])),
        said(json!([{ "type": "tool_use", "id": "3", "input": { "pattern": 5 } }])),
        // Large, and in the CLI's order of keys, which the provider searches.
        Value::String(format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"4","name":"Write","input":{{"file_path":"{app}/big.txt","content":"{big}"}}}}]}}}}"#
        )),
        json!({ "type": "assistant", "isSidechain": true, "message": { "role": "assistant", "content": [{ "type": "text", "text": "side" }] } }),
        said(json!([{ "type": "text", "text": " \n " }])),
        user("Thanks", &app),
        said(json!("not a list")),
    ], 0);
    let mut sessions = fixture.sessions();
    let transcript = transcript_of(&mut sessions, "s");
    assert_eq!(transcript["id"], "s");
    assert_eq!(transcript["title"], "Fix the bug");
    assert_eq!(transcript["project"], "app");
    assert_eq!(transcript["branch"], "main");
    assert_eq!(transcript["hidden"], 0);
    assert_eq!(
        transcript["turns"],
        json!([
            { "from": "user", "parts": [{ "kind": "text", "text": "Fix the bug", "tools": [] }] },
            { "from": "assistant", "parts": [
                { "kind": "text", "text": "Looking.", "tools": [] },
                { "kind": "tools", "text": "", "tools": [
                    { "name": "Read", "detail": "src/main.rs" },
                    { "name": "Bash", "detail": "Run the tests" },
                    { "name": "Tool", "detail": 5 },
                    { "name": "Write", "detail": "big.txt" },
                ] },
            ] },
            { "from": "user", "parts": [{ "kind": "text", "text": "Thanks", "tools": [] }] },
        ])
    );
}

#[test]
fn a_large_tool_call_is_read_without_decoding_it() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    // Cut off: a large line need not be whole JSON.
    let large = format!(
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"t1","name":"Edit","input":{{"command":"{}","description":"Write \"it\"","content":"{}"#,
        "c".repeat(400),
        "y".repeat(20_000)
    );
    let path = fixture.session("-app", "s", &[user("Go", &app)], 0);
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(large.as_bytes());
    bytes.push(b'\n');
    fixture.raw("-app", "s", &bytes, 0);
    let mut sessions = fixture.sessions();
    let tools = &transcript_of(&mut sessions, "s")["turns"][1]["parts"][0]["tools"];
    // The first of the three keys in the line wins: here the command, 300
    // characters of it, cut to fit one line.
    let detail = tools[0]["detail"].as_str().unwrap();
    assert_eq!(tools[0]["name"], "Edit");
    assert_eq!(detail, format!("{}…", "c".repeat(89)));
}

#[test]
fn only_the_latest_turns_are_kept_and_long_text_is_clipped() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    let mut lines = Vec::new();
    for n in 0..(transcript::TURNS + 3) {
        lines.push(user(&format!("Turn {n}"), &app));
    }
    lines.push(said(
        json!([{ "type": "text", "text": "z".repeat(transcript::TEXT + 10) }]),
    ));
    fixture.session("-app", "s", &lines, 0);
    let mut sessions = fixture.sessions();
    let transcript = transcript_of(&mut sessions, "s");
    let turns = transcript["turns"].as_array().unwrap();
    assert_eq!(transcript["hidden"], 4);
    assert_eq!(turns.len(), transcript::TURNS);
    assert_eq!(turns[0]["parts"][0]["text"], "Turn 4");
    let last = turns.last().unwrap()["parts"][0]["text"].as_str().unwrap();
    assert_eq!(last.chars().count(), transcript::TEXT);
    assert!(last.ends_with('…'));
}

#[test]
fn tool_calls_past_the_most_are_left_out() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    let calls: Vec<Value> = (0..transcript::TOOLS + 5)
        .map(|n| json!({ "type": "tool_use", "id": n.to_string(), "name": "Read", "input": { "path": n.to_string() } }))
        .collect();
    fixture.session(
        "-app",
        "s",
        &[user("Go", &app), said(Value::Array(calls))],
        0,
    );
    let mut sessions = fixture.sessions();
    let tools = &transcript_of(&mut sessions, "s")["turns"][1]["parts"][0]["tools"];
    assert_eq!(tools.as_array().unwrap().len(), transcript::TOOLS);
}

#[test]
fn messages_open_and_close_a_transcript() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    fixture.session("-app", "abc-1", &[user("One", &app)], 0);
    fixture.session("-app", "42", &[user("Two", &app)], 5);
    let mut sessions = fixture.sessions();
    // A Luau number reaches it as a float.
    sessions.receive(&json!({ "open": 42.0 }));
    assert_eq!(sessions.view()["transcript"]["id"], "42");
    // Not an id: left as it was. An empty Luau table arrives as `{}`.
    for ignored in [
        json!({ "open": "../x" }),
        json!({ "open": {} }),
        json!({}),
        json!("open"),
    ] {
        sessions.receive(&ignored);
        assert_eq!(sessions.view()["transcript"]["id"], "42", "{ignored}");
    }
    sessions.receive(&json!({ "open": "missing" }));
    assert!(sessions.view().get("transcript").is_none());
    sessions.receive(&json!({ "open": "abc-1" }));
    assert_eq!(sessions.view()["transcript"]["id"], "abc-1");
    // An invalid open with a close closes.
    sessions.receive(&json!({ "open": "a b", "close": true }));
    assert!(sessions.view().get("transcript").is_none());
}

#[test]
fn an_opened_transcript_is_read_again_when_its_file_changes() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    fixture.session("-app", "s", &[user("One", &app)], 120);
    let mut sessions = fixture.sessions();
    let turns = |sessions: &mut Sessions| {
        sessions.view()["transcript"]["turns"]
            .as_array()
            .unwrap()
            .len()
    };
    sessions.receive(&json!({ "open": "s" }));
    assert_eq!(turns(&mut sessions), 1);
    // Rewritten with the same time: still the one read.
    fixture.session("-app", "s", &[user("One", &app), user("Two", &app)], 120);
    assert_eq!(turns(&mut sessions), 1);
    fixture.session("-app", "s", &[user("One", &app), user("Two", &app)], 60);
    assert_eq!(turns(&mut sessions), 2);
}

/// The Python provider documented `{ refresh = true }` as looking again, but
/// only looked every 30 seconds: Look for New Sessions did nothing.
#[test]
fn refresh_looks_again_now() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    fixture.session("-app", "one", &[user("One", &app)], 10);
    let mut sessions = fixture.sessions();
    fixture.session("-app", "two", &[user("Two", &app)], 0);
    assert_eq!(sessions.view()["total"], 1);
    sessions.receive(&json!({ "refresh": true }));
    let view = sessions.view();
    assert_eq!(view["total"], 2);
    assert_eq!(ids(&view)[0].1, vec!["two".to_owned(), "one".to_owned()]);
}

#[test]
fn samples_stand_in_without_sessions() {
    let mut sessions = Sessions::new("/nowhere".to_owned(), true, Arc::default());
    sessions.scan();
    let view = sessions.view();
    assert_eq!(view["source"], "sample");
    assert_eq!(view["total"], sample::TOTAL);
    let groups = ids(&view);
    let names: Vec<&str> = groups.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["lantern", "DragKit", "tideline"]);
    assert_eq!(groups[0].1[0], "sample-1");
    let merge = &view["groups"][2]["sessions"][5];
    assert_eq!(
        (merge["pr"].clone(), merge["gone"].clone()),
        (json!(true), json!(true))
    );
    sessions.receive(&json!({ "open": "sample-3" }));
    let transcript = sessions.view()["transcript"].clone();
    assert_eq!(transcript["project"], "DragKit");
    assert_eq!(
        transcript["turns"][0]["parts"][0]["text"],
        "Cmd-click drag not working on Sonoma. Start by reading how it works today, then propose the change."
    );
    assert_eq!(transcript["turns"].as_array().unwrap().len(), 4);
}

/// Publishes, as the host would see them.
type Seen = Arc<Mutex<Vec<Value>>>;

fn host() -> (Handle, Seen) {
    let seen: Seen = Arc::default();
    let events: moonframe_providers::Events = {
        let seen = Arc::clone(&seen);
        Arc::new(move |event| {
            if let Event::Published { data, .. } = event {
                seen.lock().unwrap().push(to_json(&data));
            }
        })
    };
    let handle = Handle::new("history", Path::new("."), Data::Nil, events);
    (handle, seen)
}

/// Waits for a publish that `want` takes, past the first `skip`.
fn wait_for(seen: &Seen, skip: usize, want: impl Fn(&Value) -> bool) -> Value {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        let seen = seen.lock().unwrap();
        if let Some(found) = seen.iter().skip(skip).rev().find(|value| want(value)) {
            return found.clone();
        }
        drop(seen);
        thread::sleep(Duration::from_millis(10));
    }
    panic!("never published: {:?}", seen.lock().unwrap().last());
}

#[test]
fn the_provider_answers_mods_from_its_thread_and_stops_when_dropped() {
    let fixture = Fixture::new();
    let app = fixture.folder("app");
    fixture.session("-app", "s1", &[user("Hello there", &app)], 0);
    let (handle, seen) = host();
    let mut provider = Provider::default();
    provider.looks_in = Some((fixture.root.clone(), false));
    provider.start(&handle).unwrap();
    let first = wait_for(&seen, 0, |value| value["total"] == 1);
    assert_eq!(first["groups"][0]["sessions"][0]["title"], "Hello there");
    let open = from_json(&json!({ "open": "s1" })).unwrap();
    provider.receive(&handle, "sidebar", open);
    let opened = wait_for(&seen, 0, |value| value.get("transcript").is_some());
    assert_eq!(
        opened["transcript"]["turns"][0]["parts"][0]["text"],
        "Hello there"
    );
    // Nothing changed: nothing published.
    let count = seen.lock().unwrap().len();
    provider.receive(
        &handle,
        "sidebar",
        from_json(&json!({ "open": "s1" })).unwrap(),
    );
    provider.receive(
        &handle,
        "sidebar",
        from_json(&json!({ "close": true })).unwrap(),
    );
    wait_for(&seen, count, |value| value.get("transcript").is_none());
    // Once more: the open changed nothing.
    thread::sleep(Duration::from_millis(50));
    assert_eq!(seen.lock().unwrap().len(), count + 1);
    drop(provider);
    let after = seen.lock().unwrap().len();
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        seen.lock().unwrap().len(),
        after,
        "published after its stop"
    );
}

#[test]
fn samples_only_when_asked() {
    let (handle, seen) = host();
    let mut provider = Provider::default();
    provider.looks_in = Some(("/nowhere".to_owned(), true));
    provider.start(&handle).unwrap();
    let view = wait_for(&seen, 0, |value| value["source"] == "sample");
    assert_eq!(view["total"], sample::TOTAL);

    assert!(samples(Some(std::ffi::OsStr::new("1"))));
    assert!(!samples(Some(std::ffi::OsStr::new(""))));
    assert!(!samples(None));
}

#[test]
fn with_no_sessions_folder_the_provider_shares_none() {
    let (handle, seen) = host();
    let mut provider = Provider::default();
    provider.looks_in = Some(("/nowhere".to_owned(), samples(None)));
    provider.start(&handle).unwrap();
    let view = wait_for(&seen, 0, |value| value.get("source").is_some());
    assert_eq!(view["source"], "cli");
    assert_eq!(view["total"], 0);
    assert_eq!(view["groups"], json!([]));
}
