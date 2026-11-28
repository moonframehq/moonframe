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
        "writers": ["library", "workspace"],
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

/// A provider started on a new vault, past its first scan.
fn started(root: &Path) -> (Provider, Handle, Seen) {
    let (host, seen) = host(root);
    let mut provider = Provider::default();
    provider.start(&host).unwrap();
    provider.tick(&host);
    (provider, host, seen)
}

/// Ticks until edits waiting for typing to stop are due, and once more.
fn after_typing(provider: &mut Provider, host: &Handle) {
    thread::sleep(QUIET + Duration::from_millis(50));
    provider.tick(host);
}

fn save(
    provider: &mut Provider,
    host: &Handle,
    from: &str,
    path: &str,
    text: &str,
    base: Option<&str>,
) {
    let mut save = json!({ "path": path, "text": text });
    if let Some(base) = base {
        save["base"] = json!(base);
    }
    ask(provider, host, from, json!({ "save": save, "id": "save" }));
}

#[test]
fn edits_still_waiting_are_written_when_the_provider_stops() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Vault");
    let (mut provider, host, _) = started(&root);
    save(
        &mut provider,
        &host,
        "library",
        "notes/Welcome.md",
        "Typed just before quitting.\n",
        None,
    );
    // The app quits within the half second typing is given to stop.
    drop(provider);
    assert_eq!(
        fs::read_to_string(root.join("notes/Welcome.md")).unwrap(),
        "Typed just before quitting.\n"
    );
}

#[test]
fn a_save_is_news_to_every_editor_but_the_one_that_made_it() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Vault");
    let (mut provider, host, seen) = started(&root);
    let path = "notes/Welcome.md";
    fs::write(root.join(path), "one\n\ntwo\n\nthree\n").unwrap();
    provider.vault.as_mut().unwrap().refresh(path).unwrap();
    for layout in ["library", "workspace"] {
        ask(
            &mut provider,
            &host,
            layout,
            json!({ "watch": { "id": "note", "kind": "note", "path": path } }),
        );
    }
    let old = view(&seen, "workspace", "note").unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();

    // Library changes the first block.
    save(
        &mut provider,
        &host,
        "library",
        path,
        "ONE\n\ntwo\n\nthree\n",
        Some(&old),
    );
    after_typing(&mut provider, &host);
    let library = view(&seen, "library", "note").unwrap();
    assert!(
        library.get("text").is_none() && library["own"] == true,
        "{library}"
    );
    let workspace = view(&seen, "workspace", "note").unwrap();
    assert_eq!(workspace["text"], "ONE\n\ntwo\n\nthree\n", "{workspace}");

    // Workspace, still on its old copy, changes the last block: Library's
    // change stays, merged from the text Workspace started from.
    save(
        &mut provider,
        &host,
        "workspace",
        path,
        "one\n\ntwo\n\nTHREE\n",
        Some(&old),
    );
    after_typing(&mut provider, &host);
    assert_eq!(
        fs::read_to_string(root.join(path)).unwrap(),
        "ONE\n\ntwo\n\nTHREE\n"
    );
    let answer = last(&seen, "workspace", |msg| msg["action"] == "save").unwrap();
    assert_eq!(answer["outcome"], "merged", "{answer}");

    // Library is given the merged text in turn.
    let library = view(&seen, "library", "note").unwrap();
    assert_eq!(library["text"], "ONE\n\ntwo\n\nTHREE\n", "{library}");
}

#[test]
fn a_failed_save_is_answered_kept_and_tried_again() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Vault");
    let (mut provider, host, seen) = started(&root);
    // A folder where the note should be: the write fails.
    let path = "notes/Blocked.md";
    fs::create_dir_all(root.join(path)).unwrap();
    save(
        &mut provider,
        &host,
        "library",
        path,
        "Words worth keeping.\n",
        None,
    );
    after_typing(&mut provider, &host);
    let failed = last(&seen, "library", |msg| msg["action"] == "save").unwrap();
    assert_eq!(
        (
            failed["ok"].clone(),
            failed["kept"].clone(),
            failed["path"].clone()
        ),
        (json!(false), json!(true), json!(path)),
        "{failed}"
    );
    assert!(failed["error"].is_string());

    // Not tried again before its wait is over.
    provider.tick(&host);
    let count = |seen: &Seen| {
        seen.lock()
            .unwrap()
            .iter()
            .filter(|(to, msg)| to == "library" && msg["action"] == "save")
            .count()
    };
    assert_eq!(count(&seen), 1);

    // The way is clear again: the next try writes it.
    fs::remove_dir(root.join(path)).unwrap();
    thread::sleep(save::retry_delay(1) + Duration::from_millis(50));
    provider.tick(&host);
    let saved = last(&seen, "library", |msg| msg["action"] == "save").unwrap();
    assert_eq!(saved["ok"], true, "{saved}");
    assert_eq!(
        fs::read_to_string(root.join(path)).unwrap(),
        "Words worth keeping.\n"
    );
}

#[test]
fn todays_views_move_on_at_midnight() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Vault");
    let (host, seen) = host(&root);
    let mut provider = Provider::default();
    provider.clock = Some("2026-11-10".to_string());
    provider.start(&host).unwrap();
    provider.tick(&host);
    ask(
        &mut provider,
        &host,
        "journal",
        json!({ "watch": { "id": "days", "kind": "journal", "days": 2 } }),
    );
    let days = view(&seen, "journal", "days").unwrap();
    assert_eq!(days["rows"][0]["date"], "2026-11-10");
    assert_eq!(days["rows"][0]["today"], true);

    // Left open overnight, with nothing else changing.
    provider.clock = Some("2026-11-11".to_string());
    provider.tick(&host);
    let days = view(&seen, "journal", "days").unwrap();
    assert_eq!(days["rows"][0]["date"], "2026-11-11", "{days}");
    let summary = last(&seen, "app", |_| true).unwrap();
    assert_eq!(summary["today"], "2026-11-11");
}
