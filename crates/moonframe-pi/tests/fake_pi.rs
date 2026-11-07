//! The pi provider against a fake pi a test writes: a shell script speaking
//! pi's protocol, so these run anywhere, with no pi installed. What only the
//! real pi can show is in `real_pi.rs`.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use {
    moonframe_pi::Config,
    serde_json::{Value, json},
    std::{
        fs,
        path::{Path, PathBuf},
        thread,
        time::{Duration, Instant},
    },
    support::{App, config, fixture_project, has_item},
};

const FAKE_PI: &str = r#"#!/bin/sh
# What a test needs of pi --mode rpc. $FAKE_DIR holds what it noticed.
case "$1" in --version) echo 9.9.9; exit 0;; esac
echo "$@" >> "$FAKE_DIR/args"
echo $$ > "$FAKE_DIR/pid"
ok() { printf '{"type":"response","id":"%s","success":true,"data":%s}\n' "$1" "$2"; }
user() {
    printf '{"type":"agent_start"}\n'
    printf '{"type":"message_start","message":{"role":"user","timestamp":1762550000123,"content":"%s"}}\n' "$1"
    printf '{"type":"message_end","message":{"role":"user","timestamp":1762550000123,"content":"%s"}}\n' "$1"
    printf '{"type":"message_start","message":{"role":"assistant","timestamp":1762550001123,"content":[]}}\n'
    printf '{"type":"message_update","assistantMessageEvent":{"type":"text_start","contentIndex":0}}\n'
    printf '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"partly"}}\n'
}
answer() {
    printf '{"type":"message_end","message":{"role":"assistant","timestamp":1762550002123,"stopReason":"%s","content":[{"type":"text","text":"partly done"}]}}\n' "$1"
    printf '{"type":"agent_settled","aborted":%s}\n' "$2"
}
while IFS= read -r line; do
    id=$(printf '%s' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
    case "$line" in
    *'"type":"get_state"'*)
        ok "$id" "{\"sessionFile\":\"$FAKE_DIR/none.jsonl\",\"thinkingLevel\":\"low\",\"model\":{\"provider\":\"fake\",\"id\":\"f\",\"name\":\"Fake\"}}" ;;
    *'"type":"get_available_models"'*) ok "$id" '{"models":[{"provider":"fake","id":"f","name":"Fake"}]}' ;;
    *'"type":"get_commands"'*) ok "$id" '{"commands":[{"name":"go","description":"Goes","source":"extension"}]}' ;;
    *'"type":"get_available_thinking_levels"'*) ok "$id" '{"levels":["off","low"]}' ;;
    *'"type":"prompt"'*)
        case "$line" in
        *silence*) ;;
        *slow*)
            ok "$id" '{}'
            user "go slow" ;;
        *)
            ok "$id" '{}'
            user "hello"
            answer stop false ;;
        esac ;;
    *'"type":"abort"'*)
        ok "$id" '{}'
        touch "$FAKE_DIR/aborted"
        answer aborted true ;;
    *) ok "$id" '{}' ;;
    esac
done
touch "$FAKE_DIR/stdin-closed"
"#;

struct Fake {
    dir: tempfile::TempDir,
    root: PathBuf,
    project: PathBuf,
}

fn fake(script: &str) -> (Fake, Config) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    let pi = root.join("pi");
    fs::write(&pi, script).unwrap();
    fs::set_permissions(&pi, fs::Permissions::from_mode(0o755)).unwrap();
    let project = fixture_project(&root);
    let agent = root.join("agent");
    fs::create_dir_all(&agent).unwrap();
    let mut config = config(&root, &agent, pi);
    let noted = root.to_string_lossy().into_owned();
    config.env.push(("FAKE_DIR".to_owned(), noted));
    let fake = Fake { dir, root, project };
    (fake, config)
}

fn started(config: Config) -> App {
    let app = App::start("pi", moonframe_pi::pi_with(config));
    app.when("pi and its models", |v| {
        v["runtime"]["status"] == "ready" && v["models"][0]["id"] == "f"
    });
    app
}

fn wait_for(path: &Path, label: &str) {
    let started = Instant::now();
    while !path.exists() {
        assert!(started.elapsed() < Duration::from_secs(10), "{label}");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn pilot_pi_is_the_pi_it_runs() {
    let (_fake, config) = fake(FAKE_PI);
    let app = started(config);
    let value = app.value();
    assert_eq!(value["available"], true);
    assert_eq!(value["runtime"]["version"], "9.9.9");
    assert_eq!(value["commands"][0]["name"], "go");
}

// What the composer really sends: the host turns an empty Luau table into `{}`.
#[test]
fn an_empty_table_for_a_list_is_an_empty_list() {
    let (fake, config) = fake(FAKE_PI);
    let mut app = started(config);
    app.send(&json!({ "add_workspace": fake.project }));
    app.when("workspace", |v| v["workspaces"][0]["name"] == "tidy-cart");
    app.send(&json!({ "new_thread": {
        "workspace": fake.project, "mode": "local", "prompt": "hello there", "images": {},
    }}));
    app.when("answered", |v| {
        v["thread"]["status"] == "idle" && has_item(v, "text", |item| item["text"] == "partly done")
    });
    app.send(&json!({ "prompt": "and again", "images": {} }));
    app.when("the event that says so", |v| {
        let events = v["events"].as_array().unwrap();
        events
            .iter()
            .filter(|event| event["kind"] == "finished")
            .count()
            == 2
    });
    assert!(
        !app.value()["events"]
            .to_string()
            .contains("That didn't work")
    );
    let args = fs::read_to_string(fake.root.join("args")).unwrap();
    assert!(args.contains("--mode rpc --session-id"), "{args}");
}

#[test]
fn a_run_streams_and_stop_reaches_pi_at_once() {
    let (fake, config) = fake(FAKE_PI);
    let mut app = started(config);
    app.send(&json!({ "new_thread": { "mode": "local", "prompt": "go slow", "images": {} } }));
    app.when("streaming", |v| {
        v["thread"]["running"] == true
            && has_item(v, "text", |item| {
                item["text"] == "partly" && item["streaming"] == true
            })
    });
    assert_eq!(app.value()["thread"]["cwd"], app.value()["chats"]["path"]);
    let asked = Instant::now();
    app.send(&json!({ "abort": true }));
    assert!(
        asked.elapsed() < Duration::from_millis(50),
        "receive waited"
    );
    wait_for(&fake.root.join("aborted"), "pi never heard");
    assert!(
        asked.elapsed() < Duration::from_secs(1),
        "it took {:?}",
        asked.elapsed()
    );
    app.when("stopped", |v| v["thread"]["running"] == false);
}

#[test]
fn a_pi_that_never_answers_holds_nothing_up() {
    let (fake, config) = fake(FAKE_PI);
    let mut app = started(config);
    app.send(&json!({ "new_thread": { "mode": "local", "images": {} } }));
    app.when("a thread", |v| v["thread"]["ready"] == true);
    let before = app.publishes();
    let asked = Instant::now();
    app.send(&json!({ "prompt": "silence please", "images": {} }));
    app.send(&json!({ "set_pref": { "group": "project" } }));
    assert!(
        asked.elapsed() < Duration::from_millis(50),
        "receive waited"
    );
    app.when("what came after went ahead", |v| {
        v["prefs"]["group"] == "project"
    });
    assert!(app.publishes() > before);
    assert!(fake.dir.path().exists());
}

#[test]
fn closing_stops_each_pi_by_closing_its_stdin() {
    let (fake, config) = fake(FAKE_PI);
    let mut app = started(config);
    app.send(&json!({ "new_thread": { "mode": "local", "images": {} } }));
    app.when("a thread", |v| v["thread"]["ready"] == true);
    let asked = Instant::now();
    app.stop();
    assert!(
        asked.elapsed() < Duration::from_secs(1),
        "{:?}",
        asked.elapsed()
    );
    wait_for(&fake.root.join("stdin-closed"), "pi was not told");
}

#[test]
fn a_pi_that_ignores_its_stdin_gets_a_signal() {
    let deaf = "#!/bin/sh\ncase \"$1\" in --version) echo 9.9.9; exit 0;; esac\n\
                echo $$ > \"$FAKE_DIR/pid\"\nexec sleep 60\n";
    let (fake, config) = fake(deaf);
    let mut app = App::start("pi", moonframe_pi::pi_with(config));
    app.when("up", |v| v["runtime"]["status"] == "ready");
    wait_for(&fake.root.join("pid"), "pi did not start");
    let pid = fs::read_to_string(fake.root.join("pid")).unwrap();
    let asked = Instant::now();
    app.stop();
    assert!(
        asked.elapsed() >= Duration::from_millis(1400),
        "{:?}",
        asked.elapsed()
    );
    assert!(
        asked.elapsed() < Duration::from_secs(3),
        "{:?}",
        asked.elapsed()
    );
    let alive = std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!alive.success(), "it outlived the provider");
}

#[test]
fn with_no_pi_the_app_says_so() {
    if Path::new("/opt/homebrew/bin/pi").exists() || Path::new("/usr/local/bin/pi").exists() {
        return;
    }
    let (fake, mut config) = fake("");
    config.pi = None;
    let empty = fake.root.join("empty");
    fs::create_dir(&empty).unwrap();
    config
        .env
        .push(("PATH".to_owned(), empty.to_string_lossy().into_owned()));
    config.env.push((
        "MISE_DATA_DIR".to_owned(),
        empty.to_string_lossy().into_owned(),
    ));
    let app = App::start("pi", moonframe_pi::pi_with(config));
    app.when("the reason", |v| v["available"] == false);
    let value = app.value();
    let reason = value["reason"].as_str().unwrap();
    assert!(reason.starts_with("pi was not found."), "{reason}");
    // Views read these before they get to `available`.
    assert_eq!(value["prefs"]["group"], "time");
    assert_eq!(value["threads"], json!([]));
    assert_eq!(value["runtime"]["status"], "missing");
}

#[test]
fn state_left_by_the_node_provider_opens_as_it_was() {
    let (fake, config) = fake(FAKE_PI);
    let data = fake.root.join("data");
    fs::create_dir_all(&data).unwrap();
    let project = fake.project.to_string_lossy();
    let state = json!({
        "version": 1,
        "workspaces": [{ "path": project, "name": "tidy-cart" }],
        "threads": { "aaaa": {
            "id": "aaaa", "cwd": project, "workspace": project, "title": "Old thread",
            "created": 1_762_550_000.5, "pinned": true,
        }},
        "schedules": [],
        "selected": "aaaa",
        "prefs": { "group": "project", "last_model": { "provider": "fake", "id": "f" } },
    });
    fs::write(
        data.join("state.json"),
        serde_json::to_string_pretty(&state).unwrap(),
    )
    .unwrap();
    let app = started(config);
    let value: Value = app.value();
    assert_eq!(value["selected"], "aaaa");
    assert_eq!(value["threads"][0]["title"], "Old thread");
    assert_eq!(value["threads"][0]["pinned"], true);
    assert_eq!(value["prefs"]["group"], "project");
    assert_eq!(value["prefs"]["sounds"], true);
}
