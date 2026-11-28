use {
    super::*,
    crate::commands::{MAX_CONTEXT, command},
    herdr_chat_protocol::{Change, Entry, EntryId, EntryKind},
    moonframe_providers::Event,
};

type Seen = Arc<Mutex<Vec<Event>>>;

fn host(settings: &str, dir: &Path) -> (Handle, Seen) {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let events: moonframe_providers::Events = {
        let seen = Arc::clone(&seen);
        Arc::new(move |event| seen.lock().unwrap().push(event))
    };
    let settings =
        moonframe_providers::from_json(&serde_json::from_str(settings).unwrap()).unwrap();
    (Handle::new("chat", dir, settings, events), seen)
}

fn message(text: &str) -> Data {
    moonframe_providers::from_json(&serde_json::from_str(text).unwrap()).unwrap()
}

fn replies(seen: &Seen) -> Vec<Value> {
    seen.lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            Event::Sent { msg, .. } => Some(to_json(msg)),
            _ => None,
        })
        .collect()
}

fn published(seen: &Seen) -> Value {
    let seen = seen.lock().unwrap();
    seen.iter()
        .rev()
        .find_map(|event| match event {
            Event::Published { data, .. } => Some(to_json(data)),
            _ => None,
        })
        .unwrap()
}

fn chats(dir: &Path, settings: &str) -> (Chats, Handle, Seen) {
    let store = dir.join("chats.json");
    // As a JSON string, so a Windows path's backslashes are escaped.
    let store = serde_json::to_string(&store.display().to_string()).unwrap();
    let settings = settings.replace("\"STORE\"", &store);
    let (host, seen) = host(&settings, dir);
    // Only the program the settings name: never one installed here.
    let mut chats = Chats::default();
    chats.isolated = true;
    chats.start(&host).unwrap();
    (chats, host, seen)
}

#[test]
fn only_starters_drive_chats() {
    let dir = tempfile::tempdir().unwrap();
    let (mut chats, host, seen) = chats(
        dir.path(),
        r#"{ "starters": ["chat"], "store": "STORE", "program": "/nowhere/herdr-acp" }"#,
    );
    chats.receive(
        &host,
        "other",
        message(r#"{ "start": "claude", "id": "a" }"#),
    );
    let answer = &replies(&seen)[0];
    assert_eq!(answer["ok"], false);
    assert_eq!(answer["reply"], "a");
    assert!(answer["error"].as_str().unwrap().contains("starters"));
    // A starter gets a chat, which fails without the program.
    chats.receive(
        &host,
        "chat/composer#main",
        message(r#"{ "start": "claude" }"#),
    );
    let answer = &replies(&seen)[1];
    assert_eq!(answer["ok"], true, "{answer}");
    let shared = published(&seen);
    assert_eq!(shared["available"], false);
    assert_eq!(shared["chats"][0]["owner"], "chat");
    assert_eq!(shared["chats"][0]["status"], "failed");
    // Another mod's chat is not its to close.
    let id = answer["chat"].as_str().unwrap().to_owned();
    chats.receive(
        &host,
        "other",
        message(&format!(r#"{{ "close": "{id}" }}"#)),
    );
    assert_eq!(published(&seen)["chats"].as_array().unwrap().len(), 1);
    chats.receive(&host, "chat", message(&format!(r#"{{ "close": "{id}" }}"#)));
    assert_eq!(published(&seen)["chats"].as_array().unwrap().len(), 0);
}

#[test]
fn a_bad_agent_name_or_folder_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut chats, host, seen) =
        chats(dir.path(), r#"{ "starters": ["chat"], "store": "STORE" }"#);
    chats.receive(&host, "chat", message(r#"{ "start": "../evil" }"#));
    chats.receive(
        &host,
        "chat",
        message(r#"{ "start": "claude", "cwd": "/no/such/folder" }"#),
    );
    let answers = replies(&seen);
    assert_eq!(answers[0]["ok"], false);
    assert_eq!(answers[1]["ok"], false);
}

#[test]
fn chats_are_saved_and_one_that_ended_comes_back_resumable() {
    let dir = tempfile::tempdir().unwrap();
    let settings = r#"{ "starters": ["chat"], "store": "STORE", "program": "/nowhere/herdr-acp" }"#;
    let (mut first, host, _) = chats(dir.path(), settings);
    first.receive(&host, "chat", message(r#"{ "start": "codex" }"#));
    {
        let mut state = lock(&first.state);
        state.chats[0].session = Some("session-1".to_owned());
        state.unsaved = true;
    }
    first.publish(&host);
    let (_second, _, seen) = chats(dir.path(), settings);
    let chat = &published(&seen)["chats"][0];
    assert_eq!(chat["agent"], "codex");
    assert_eq!(chat["status"], "ended");
    assert_eq!(chat["session"], "session-1");
}

#[test]
fn the_chat_in_view_shares_more_of_its_transcript() {
    let mut transcript = Transcript::new("claude");
    for at in 0..20 {
        let entry = Entry {
            id: EntryId(at),
            kind: EntryKind::Agent {
                text: "x".repeat(300 * 1024),
            },
        };
        transcript.apply(&Change::Push { entry }).unwrap();
    }
    let short = project::transcript(&transcript, project::SHORT_ENTRIES);
    assert_eq!(
        short["entries"].as_array().unwrap().len(),
        project::SHORT_ENTRIES
    );
    assert_eq!(short["hidden"], 20 - project::SHORT_ENTRIES);
    let text = short["entries"][0]["kind"]["text"].as_str().unwrap();
    assert!(text.len() <= 256 * 1024 + 3 && text.starts_with('…'));
    let full = project::transcript(&transcript, project::FULL_ENTRIES);
    assert_eq!(full["entries"].as_array().unwrap().len(), 20);
}

/// Against the installed herdr-acp and Claude: starts a chat, attaches,
/// toggles auto-accept (answered by herdr-acp itself, so no tokens are
/// spent) and closes it. `cargo test -p moonframe-chat -- --ignored`.
#[cfg(unix)]
#[test]
#[ignore = "needs herdr-acp and claude installed"]
fn a_real_chat_starts_attaches_and_takes_commands() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let store = serde_json::to_string(&dir.path().join("chats.json")).unwrap();
    let (host, seen) = host(
        &format!(r#"{{ "starters": ["chat"], "store": {store} }}"#),
        dir.path(),
    );
    let mut chats = Chats::default();
    chats.start(&host).unwrap();
    assert!(chats.program.is_some(), "herdr-acp not found");
    chats.receive(
        &host,
        "chat",
        message(&format!(
            r#"{{ "start": "claude", "cwd": "{}" }}"#,
            cwd.path().display()
        )),
    );
    let id = replies(&seen)[0]["chat"].as_str().unwrap().to_owned();
    // Sent while it starts: held, and sent as it attaches.
    chats.receive(
        &host,
        "chat",
        message(&format!(
            r#"{{ "auto_accept": true, "chat": "{id}", "id": "aa" }}"#
        )),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        chats.tick(&host);
        let chat = published(&seen)["chats"][0].clone();
        if chat["status"] == "live" && chat["view"]["status"] != "starting" {
            break;
        }
        assert_ne!(chat["status"], "failed", "{chat}");
        assert!(
            std::time::Instant::now() < deadline,
            "never attached: {chat}"
        );
        thread::sleep(std::time::Duration::from_millis(200));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        chats.tick(&host);
        if replies(&seen).iter().any(|reply| reply["reply"] == "aa") {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "no ack");
        thread::sleep(std::time::Duration::from_millis(50));
    }
    let ack = replies(&seen)
        .into_iter()
        .find(|reply| reply["reply"] == "aa")
        .unwrap();
    assert_eq!(ack["ok"], true, "{ack}");
    thread::sleep(std::time::Duration::from_millis(300));
    chats.tick(&host);
    assert_eq!(published(&seen)["chats"][0]["view"]["auto_accept"], true);
    eprintln!("model: {}", published(&seen)["chats"][0]["view"]["model"]);
    // A new chat lets the user switch to any other installed agent.
    let installed = published(&seen)["agents"].as_array().map_or(0, Vec::len);
    let offered = published(&seen)["chats"][0]["view"]["agents"].clone();
    eprintln!(
        "program: {}, agents: {offered}",
        published(&seen)["program"]
    );
    if installed > 1 {
        assert!(
            offered.as_array().is_some_and(|agents| agents.len() > 1),
            "{offered}"
        );
    }
    // Switching to Codex, when it is there, is acknowledged and recorded,
    // so a resume would start Codex.
    let codex = offered
        .as_array()
        .is_some_and(|agents| agents.iter().any(|agent| agent["id"] == "codex"));
    if codex {
        chats.receive(
            &host,
            "chat",
            message(&format!(
                r#"{{ "agent": "codex", "chat": "{id}", "id": "sw" }}"#
            )),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            chats.tick(&host);
            let switched = published(&seen)["chats"][0]["agent"] == "codex";
            let acked = replies(&seen).iter().any(|reply| reply["reply"] == "sw");
            if switched && acked {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "no switch: {:?}",
                replies(&seen)
            );
            thread::sleep(std::time::Duration::from_millis(100));
        }
        let ack = replies(&seen)
            .into_iter()
            .find(|reply| reply["reply"] == "sw")
            .unwrap();
        assert_eq!(ack["ok"], true, "{ack}");
        eprintln!(
            "switched: {}",
            published(&seen)["chats"][0]["view"]["agent"]
        );
    }
    chats.receive(&host, "chat", message(&format!(r#"{{ "close": "{id}" }}"#)));
}

#[test]
fn a_program_setting_naming_no_file_falls_back_to_the_search() {
    let dir = tempfile::tempdir().unwrap();
    let installed = dir.path().join("Sheprd.app/herdr-acp");
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::write(&installed, "").unwrap();
    let bundled = [dir.path().join("Herdr.app/herdr-acp"), installed.clone()];
    let missing = Path::new("/nowhere/herdr-acp");
    let found = program::find(Some(missing), std::ffi::OsStr::new(""), &bundled);
    assert_eq!(found.program.as_deref(), Some(installed.as_path()));
    assert_eq!(found.missing.as_deref(), Some(missing));
    // A setting naming a file wins, and nothing is missing.
    let found = program::find(Some(&installed), std::ffi::OsStr::new(""), &[]);
    assert_eq!(found, program::Found {
        program: Some(installed.clone()),
        missing: None
    });
    // An app's newest name for the program comes before its old one.
    let bundled = program::bundled(Some(Path::new("/Users/me")));
    let at = |name: &str| {
        bundled
            .iter()
            .position(|path| path == Path::new(name))
            .unwrap()
    };
    assert!(
        at("/Users/me/Applications/Sheprd.app/Contents/MacOS/sheprd-acp")
            < at("/Users/me/Applications/Sheprd.app/Contents/MacOS/herdr-acp")
    );
    // The provider says the setting was passed over.
    let (_chats, _host, seen) = chats(
        dir.path(),
        r#"{ "starters": ["chat"], "store": "STORE", "program": "/nowhere/herdr-acp" }"#,
    );
    let shared = published(&seen);
    assert!(
        shared["warning"]
            .as_str()
            .unwrap()
            .contains("/nowhere/herdr-acp")
    );
}

#[test]
fn a_prompt_carries_its_context_in_a_frame_the_view_hides() {
    let framed = command(
        "prompt",
        &json!({ "prompt": "Summarize" }),
        Some("Mail: Re: invoice"),
    )
    .unwrap();
    let Command::Prompt { text, .. } = framed else {
        panic!()
    };
    assert_eq!(
        moonframe_chat_view::unframe(&text),
        (Some("Mail: Re: invoice"), "Summarize")
    );
    // A start's context waits for the first prompt; a prompt's own wins.
    let dir = tempfile::tempdir().unwrap();
    let (mut chats, host, seen) = chats(
        dir.path(),
        r#"{ "starters": ["chat"], "store": "STORE", "program": "/nowhere/herdr-acp" }"#,
    );
    chats.receive(
        &host,
        "chat",
        message(r#"{ "start": "claude", "context": "You help with mail." }"#),
    );
    let id = replies(&seen)[0]["chat"].as_str().unwrap().to_owned();
    assert_eq!(
        chats.start_context("chat", &id).as_deref(),
        Some("You help with mail.")
    );
    assert_eq!(chats.start_context("other", &id), None, "not its chat");
    let too_long = "x".repeat(MAX_CONTEXT + 1);
    chats.receive(
        &host,
        "chat",
        message(&json!({ "start": "claude", "context": too_long, "id": "big" }).to_string()),
    );
    let refused = replies(&seen)
        .into_iter()
        .find(|reply| reply["reply"] == "big")
        .unwrap();
    assert_eq!(refused["ok"], false);
}

#[test]
fn chats_share_a_prepared_view_and_the_raw_transcript_only_on_request() {
    let dir = tempfile::tempdir().unwrap();
    for raw in [false, true] {
        let settings = format!(
            r#"{{ "starters": ["chat"], "store": "STORE", "program": "/nowhere/herdr-acp", "raw": {raw} }}"#
        );
        let (mut chats, host, seen) = chats(dir.path(), &settings);
        chats.receive(&host, "chat", message(r#"{ "start": "claude" }"#));
        {
            let mut state = lock(&chats.state);
            let mut transcript = Transcript::new("Claude");
            for (at, kind) in [
                EntryKind::User {
                    text: moonframe_chat_view::frame("hidden", "Hello"),
                    files: 0,
                    note: None,
                    guest: None,
                },
                EntryKind::Agent {
                    text: "Hi there".to_owned(),
                },
            ]
            .into_iter()
            .enumerate()
            {
                let entry = Entry {
                    id: EntryId(at as u64),
                    kind,
                };
                transcript.apply(&Change::Push { entry }).unwrap();
            }
            state.chats[0].transcript = Some(transcript);
            state.dirty = true;
        }
        chats.publish(&host);
        let chat = &published(&seen)["chats"][0];
        let rows = chat["view"]["rows"].as_array().unwrap();
        assert_eq!(rows[0]["type"], "user");
        assert_eq!(rows[0]["text"], "Hello");
        assert_eq!(rows[0]["context"], true);
        assert_eq!(rows[1]["text"], "Hi there");
        assert_eq!(chat.get("transcript").is_some(), raw);
        drop(chats);
        let _ = std::fs::remove_file(dir.path().join("chats.json"));
    }
}

#[cfg(unix)]
#[test]
fn commands_wait_for_a_starting_chat_and_answer_their_sender() {
    let dir = tempfile::tempdir().unwrap();
    let (chats, _host, _seen) = chats(
        dir.path(),
        r#"{ "starters": ["chat"], "store": "STORE", "program": "/nowhere/herdr-acp" }"#,
    );
    let starting = |id: &str, phase| Chat {
        id: id.to_owned(),
        owner: "chat".to_owned(),
        agent: "claude".to_owned(),
        cwd: "/".to_owned(),
        created: 0,
        pid: None,
        endpoint: None,
        session: None,
        phase,
        context: None,
        transcript: None,
        output_revisions: output::Revisions::default(),
        cursor: None,
        writer: None,
        held: Vec::new(),
        closing: false,
    };
    {
        let mut state = lock(&chats.state);
        state.chats.push(starting("new", Phase::Starting));
        state
            .chats
            .push(starting("old", Phase::Ended("gone".to_owned())));
    }
    let prompt = || Command::Prompt {
        text: "hi".to_owned(),
        attachments: Vec::new(),
        origin: None,
    };
    let from = "chat/agent-chat#main";
    for _ in 0..MAX_HELD {
        let sent = chats.command(from, "new", prompt(), json!("p"), "prompt");
        assert_eq!(sent, Ok(None), "answered as it attaches");
    }
    assert!(
        chats
            .command(from, "new", prompt(), json!("p"), "prompt")
            .is_err()
    );
    assert!(
        chats
            .command(from, "old", prompt(), json!("p"), "prompt")
            .is_err()
    );
    assert!(
        chats
            .command("other", "new", prompt(), json!("p"), "prompt")
            .is_err()
    );
    let state = lock(&chats.state);
    assert_eq!(state.chat("new").unwrap().held.len(), MAX_HELD);
    assert!(state.pending.values().all(|pending| pending.to == from));
}

#[cfg(unix)]
#[test]
fn the_start_context_goes_only_with_the_first_prompt_even_one_with_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let (mut chats, host, seen) = chats(
        dir.path(),
        r#"{ "starters": ["chat"], "store": "STORE", "program": "/nowhere/herdr-acp" }"#,
    );
    {
        let mut state = lock(&chats.state);
        state.chats.push(Chat {
            id: "new".to_owned(),
            owner: "chat".to_owned(),
            agent: "claude".to_owned(),
            cwd: "/".to_owned(),
            created: 0,
            pid: None,
            endpoint: None,
            session: None,
            phase: Phase::Starting,
            context: Some("You help with mail.".to_owned()),
            transcript: None,
            output_revisions: output::Revisions::default(),
            cursor: None,
            writer: None,
            held: Vec::new(),
            closing: false,
        });
    }
    chats.receive(
        &host,
        "chat",
        message(r#"{ "prompt": "hi", "chat": "new", "context": "Its own" }"#),
    );
    assert!(replies(&seen).is_empty(), "held, answered as it attaches");
    assert_eq!(chats.start_context("chat", "new"), None);
    let state = lock(&chats.state);
    let Command::Prompt { text, .. } = &state.chat("new").unwrap().held[0].1 else {
        panic!("not a prompt")
    };
    assert_eq!(moonframe_chat_view::unframe(text), (Some("Its own"), "hi"));
}

#[test]
fn tool_output_caches_pages_and_refreshes_on_replace() {
    use herdr_chat_protocol::{Tool, ToolContent, ToolKind, ToolStatus};

    let dir = tempfile::tempdir().unwrap();
    let (mut chats, host, seen) = chats(
        dir.path(),
        r#"{ "starters": ["chat"], "store": "STORE", "program": "/nowhere/herdr-acp" }"#,
    );
    chats.receive(&host, "chat", message(r#"{ "start": "claude" }"#));
    let id = replies(&seen)[0]["chat"].as_str().unwrap().to_owned();
    let first_page = "a".repeat(4096);
    let tool_entry = |text: String| Entry {
        id: EntryId(2),
        kind: EntryKind::Tool(Tool {
            call_id: "call-1".to_owned(),
            content: vec![ToolContent::Text { text }],
            kind: ToolKind::Execute,
            locations: Vec::new(),
            parent: None,
            started_ms: None,
            status: ToolStatus::Completed,
            title: "run".to_owned(),
        }),
    };
    {
        let mut state = lock(&chats.state);
        let mut transcript = Transcript::new("Claude");
        transcript
            .apply(&Change::Push {
                entry: tool_entry(format!("{first_page}tail")),
            })
            .unwrap();
        let chat = state.chat_mut(&id).unwrap();
        chat.transcript = Some(transcript);
        chat.output_revisions.on_snapshot();
    }

    // First page populates the cache; second page hits the same cached index.
    chats.receive(
        &host,
        "chat",
        message(&format!(
            r#"{{ "tool_output": "e2", "chat": "{id}", "page": 0, "id": "p0" }}"#
        )),
    );
    assert!(chats.output_cache.is_some());
    chats.receive(
        &host,
        "chat",
        message(&format!(
            r#"{{ "tool_output": "e2", "chat": "{id}", "page": 1, "id": "p1" }}"#
        )),
    );
    // An unrelated entry change leaves the cached tool output intact.
    {
        let mut state = lock(&chats.state);
        let chat = state.chat_mut(&id).unwrap();
        let push = Change::Push {
            entry: Entry {
                id: EntryId(3),
                kind: EntryKind::Agent {
                    text: "done".to_owned(),
                },
            },
        };
        chat.transcript.as_mut().unwrap().apply(&push).unwrap();
        chat.output_revisions.on_change(&push);
        assert!(matches!(
            output::lookup(chat, "e2", chats.output_cache.as_ref()),
            Ok(output::Lookup::Cached)
        ));
    }
    // Replacing e2 invalidates the cache and serves the updated text.
    {
        let mut state = lock(&chats.state);
        let chat = state.chat_mut(&id).unwrap();
        let replace = Change::Replace {
            entry: tool_entry("updated".to_owned()),
        };
        chat.transcript.as_mut().unwrap().apply(&replace).unwrap();
        chat.output_revisions.on_change(&replace);
    }
    chats.receive(
        &host,
        "chat",
        message(&format!(
            r#"{{ "tool_output": "e2", "chat": "{id}", "page": 0, "id": "p2" }}"#
        )),
    );

    let answers = replies(&seen);
    let reply = |tag: &str| answers.iter().find(|r| r["reply"] == tag).unwrap();
    assert_eq!(reply("p0")["text"], first_page);
    assert_eq!(reply("p0")["more"], true);
    assert_eq!(reply("p1")["text"], "tail");
    assert_eq!(reply("p1")["more"], false);
    assert_eq!(reply("p2")["text"], "updated");
    assert_eq!(reply("p2")["more"], false);

    // Closing the chat clears its cached output.
    chats.receive(&host, "chat", message(&format!(r#"{{ "close": "{id}" }}"#)));
    assert!(chats.output_cache.is_none());
}
