//! The provider driven as the app drives it: started on a vault, mods
//! watching views, saving, capturing, and the file changed elsewhere.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use {
    super::*,
    moonframe_providers::{Event, from_json},
    std::{sync::Mutex, thread, time::Duration},
};

type Seen = Arc<Mutex<Vec<(String, Value)>>>;

fn host(vault: &Path) -> (Handle, Seen) {
    let seen: Seen = Arc::default();
    let sink = Arc::clone(&seen);
    let events: moonframe_providers::Events = Arc::new(move |event| match event {
        Event::Sent { to, msg, .. } => sink.lock().unwrap().push((to, to_json(&msg))),
        Event::Published { data, .. } => sink
            .lock()
            .unwrap()
            .push(("app".to_string(), to_json(&data))),
        _ => {},
    });
    let settings = from_json(&json!({
        "vault": vault.display().to_string(),
        "index": vault.with_extension("index.sqlite").display().to_string(),
        "writers": ["library"],
    }))
    .unwrap();
    let folder = vault.with_extension("provider");
    fs::create_dir_all(&folder).unwrap();
    (Handle::new("notes", &folder, settings, events), seen)
}

fn ask(provider: &mut Provider, host: &Handle, from: &str, message: Value) {
    provider.receive(host, from, from_json(&message).unwrap());
}

/// The newest message sent to `to` that `pick` accepts.
fn last(seen: &Seen, to: &str, pick: impl Fn(&Value) -> bool) -> Option<Value> {
    seen.lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(who, msg)| who == to && pick(msg))
        .map(|(_, msg)| msg.clone())
}

fn view(seen: &Seen, to: &str, id: &str) -> Option<Value> {
    last(seen, to, |msg| msg["view"] == id).map(|msg| msg["data"].clone())
}

#[test]
fn views_follow_saves_captures_and_edits_made_elsewhere() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Vault");
    let (host, seen) = host(&root);
    let mut provider = Provider::default();
    provider.start(&host).unwrap();
    provider.tick(&host);
    let summary = last(&seen, "app", |_| true).unwrap();
    assert_eq!(summary["available"], true);
    assert_eq!(summary["status"], "ready");

    // A list of notes, and one note's text.
    ask(
        &mut provider,
        &host,
        "library",
        json!({ "watch": { "id": "list", "kind": "list", "folder": "notes" } }),
    );
    let list = view(&seen, "library", "list").unwrap();
    assert_eq!(list["rows"][0]["title"], "Welcome");
    ask(
        &mut provider,
        &host,
        "library",
        json!({ "watch": { "id": "open", "kind": "note", "path": "notes/Welcome.md" } }),
    );
    let note = view(&seen, "library", "open").unwrap();
    let base = note["text"].as_str().unwrap().to_string();

    // Typing: written once it stops, and not sent back to the editor.
    let edited = format!("{base}\nA new line, see [[Inbox ideas]].\n");
    ask(
        &mut provider,
        &host,
        "library",
        json!({ "save": { "path": "notes/Welcome.md", "text": edited, "base": base }, "id": "s1" }),
    );
    provider.tick(&host);
    assert!(
        last(&seen, "library", |msg| msg["action"] == "save").is_none(),
        "written before typing stopped"
    );
    thread::sleep(QUIET + Duration::from_millis(50));
    provider.tick(&host);
    let saved = last(&seen, "library", |msg| msg["action"] == "save").unwrap();
    assert_eq!(
        (
            saved["ok"].clone(),
            saved["outcome"].clone(),
            saved["reply"].clone()
        ),
        (json!(true), json!("written"), json!("s1"))
    );
    assert_eq!(
        fs::read_to_string(root.join("notes/Welcome.md")).unwrap(),
        edited
    );
    let echoed = view(&seen, "library", "open").unwrap();
    assert!(
        echoed.get("text").is_none() && echoed["own"] == true,
        "{echoed}"
    );

    // Captured into the inbox, by a writer only.
    ask(
        &mut provider,
        &host,
        "library",
        json!({ "capture": { "text": "an idea" }, "id": "c1" }),
    );
    assert_eq!(
        last(&seen, "library", |msg| msg["reply"] == "c1").unwrap()["ok"],
        true
    );
    assert!(
        fs::read_to_string(root.join("inbox.md"))
            .unwrap()
            .starts_with("- an idea\n")
    );
    ask(
        &mut provider,
        &host,
        "stranger",
        json!({ "capture": { "text": "nope" }, "id": "c2" }),
    );
    assert_eq!(
        last(&seen, "stranger", |msg| msg["reply"] == "c2").unwrap()["ok"],
        false
    );

    // Changed in another editor: the view gets the new text.
    fs::write(root.join("notes/Welcome.md"), "Rewritten in Obsidian.\n").unwrap();
    let mut fresh = None;
    for _ in 0..40 {
        thread::sleep(Duration::from_millis(100));
        provider.tick(&host);
        fresh = view(&seen, "library", "open")
            .filter(|data| data["text"] == "Rewritten in Obsidian.\n");
        if fresh.is_some() {
            break;
        }
    }
    assert!(fresh.is_some(), "the watcher never saw the edit");
    let list = view(&seen, "library", "list").unwrap();
    assert!(
        list["rows"][0]["excerpt"]
            .as_str()
            .unwrap()
            .starts_with("Rewritten"),
        "{list}"
    );
}

#[test]
fn a_missing_vault_is_made_with_the_starting_layout() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("New vault");
    let (host, seen) = host(&root);
    let mut provider = Provider::default();
    provider.start(&host).unwrap();
    provider.tick(&host);
    assert!(root.join("AGENTS.md").is_file() && root.join("journal").is_dir());
    let summary = last(&seen, "app", |_| true).unwrap();
    assert_eq!(summary["counts"]["notes"], 1);
    assert_eq!(summary["vault"]["name"], "New vault");
}
