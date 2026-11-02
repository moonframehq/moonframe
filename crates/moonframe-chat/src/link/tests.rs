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
