use {
    super::*,
    crate::{Chat, State},
    herdr_chat_protocol::{Change, Cursor, Entry, EntryId, EntryKind, Transcript},
    moonframe_core::Data,
    std::sync::{Arc, Mutex},
};

#[test]
fn cursor_reconnect_becomes_live_without_a_snapshot() {
    let cursor = Cursor::new_epoch();
    let shared = Arc::new(Mutex::new(State::default()));
    lock(&shared).chats.push(Chat {
        id: "test".into(),
        owner: "chat".into(),
        agent: "test".into(),
        cwd: "/".into(),
        created: 0,
        pid: None,
        endpoint: None,
        session: None,
        phase: Phase::Reconnecting,
        transcript: Some(Transcript::new("test")),
        cursor: Some(cursor),
        writer: None,
        closing: false,
    });
    let host = Handle::new(
        "chat",
        std::path::Path::new("."),
        Data::Nil,
        Arc::new(|_| {}),
    );
    let (reader, mut server) = UnixStream::pair().unwrap();
    let client_state = Arc::clone(&shared);
    let client = thread::spawn(move || session(&client_state, &host, "test", reader));
    assert!(
        matches!(read_frame::<_, ClientMessage>(&mut server).unwrap(), Some(ClientMessage::Hello { since: Some(since), .. }) if since == cursor)
    );
    write_frame(&mut server, &ServerMessage::Welcome {
        agent: "test".into(),
        pane: None,
        version: VERSION,
    })
    .unwrap();
    write_frame(&mut server, &ServerMessage::Change {
        cursor: Cursor { seq: 1, ..cursor },
        change: Change::Push {
            entry: Entry {
                id: EntryId(1),
                kind: EntryKind::Agent {
                    text: "replayed".into(),
                },
            },
        },
    })
    .unwrap();
    server.shutdown(std::net::Shutdown::Write).unwrap();
    assert!(!client.join().unwrap());
    let state = lock(&shared);
    let chat = state.chat("test").unwrap();
    assert_eq!(chat.phase, Phase::Live);
    assert_eq!(chat.cursor.unwrap().seq, 1);
    assert_eq!(
        chat.transcript.as_ref().unwrap().entries[0].kind.text(),
        Some("replayed")
    );
}

fn chat(id: &str, writer: Option<UnixStream>) -> Chat {
    Chat {
        id: id.into(),
        owner: "chat".into(),
        agent: "test".into(),
        cwd: "/".into(),
        created: 0,
        pid: None,
        endpoint: None,
        session: None,
        phase: Phase::Live,
        transcript: Some(Transcript::new("test")),
        cursor: None,
        writer,
        closing: false,
    }
}

#[test]
fn commands_a_chat_never_answered_are_answered_when_it_goes() {
    let seen: Arc<Mutex<Vec<moonframe_providers::Event>>> = Arc::default();
    let events: moonframe_providers::Events = {
        let seen = Arc::clone(&seen);
        Arc::new(move |event| seen.lock().unwrap().push(event))
    };
    let host = Handle::new("chat", std::path::Path::new("."), Data::Nil, events);
    let mut state = State::default();
    for (chat, reply) in [("a", "first"), ("b", "other")] {
        state.pending.insert(CommandId::random(), crate::Pending {
            chat: chat.into(),
            owner: "chat".into(),
            reply: json!(reply),
            action: "prompt",
        });
    }
    state.settle(&host, "a", "the chat disconnected before answering");
    assert_eq!(
        state.pending.len(),
        1,
        "only the gone chat's commands settle"
    );
    let answers: Vec<serde_json::Value> = seen
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            moonframe_providers::Event::Sent { msg, .. } => Some(moonframe_providers::to_json(msg)),
            _ => None,
        })
        .collect();
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0]["reply"], "first");
    assert_eq!(answers[0]["ok"], false);
}

#[test]
fn dropping_the_provider_ends_its_connections_not_its_chats() {
    let (writer, mut program) = UnixStream::pair().unwrap();
    let chats = crate::Chats::default();
    let shared = Arc::clone(&chats.state);
    lock(&shared).chats.push(chat("kept", Some(writer)));
    drop(chats);
    {
        let state = lock(&shared);
        assert!(state.detached);
        assert!(
            state.chat("kept").is_some(),
            "the chat itself is not closed"
        );
    }
    // Its connection thread sees it and ends.
    assert!(closing(&shared, "kept"));
    // The connection is shut: the program's side reads its end.
    let mut byte = [0; 1];
    assert_eq!(std::io::Read::read(&mut program, &mut byte).unwrap(), 0);
}
