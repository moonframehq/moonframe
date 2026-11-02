//! The connection to one chat program: a thread that greets it, keeps the
//! transcript in step with its changes, and comes back after a drop with
//! the cursor it reached, so only what it missed is sent again.

use {
    crate::{Phase, Shared, lock, send},
    herdr_chat_protocol::{
        ClientKind, ClientMessage, Command, CommandId, EndpointId, REVISION, ServerMessage,
        VERSION, read_frame, write_frame,
    },
    moonframe_providers::Handle,
    serde_json::json,
    std::{os::unix::net::UnixStream, thread, time::Duration},
};

/// How long a chat that dropped is tried again before it counts as ended.
const RETRIES: u32 = 50;
const RETRY: Duration = Duration::from_millis(200);

/// Attaches to the chat `id` at `endpoint` on a thread of its own.
pub(crate) fn attach(shared: Shared, host: Handle, id: String, endpoint: EndpointId) {
    let spawned = thread::Builder::new()
        .name(format!("chat-{id}"))
        .spawn(move || run(&shared, &host, &id, &endpoint));
    if let Err(error) = spawned {
        tracing::warn!("chat: cannot attach: {error}");
    }
}

fn run(shared: &Shared, host: &Handle, id: &str, endpoint: &EndpointId) {
    let Some(address) = herdr_chat_protocol::endpoint_address(endpoint) else {
        return;
    };
    let mut failures = 0;
    loop {
        if closing(shared, id) {
            return;
        }
        let ended = match UnixStream::connect(&address) {
            Ok(stream) => {
                failures = 0;
                session(shared, host, id, stream)
            },
            Err(_) => {
                failures += 1;
                false
            },
        };
        let gone = ended || !address.exists() || failures > RETRIES;
        let mut state = lock(shared);
        if state.detached {
            return;
        }
        state.settle(host, id, "the chat disconnected before answering");
        let Some(chat) = state.chat_mut(id) else {
            return;
        };
        chat.writer = None;
        if chat.closing {
            return;
        }
        if gone {
            chat.phase = Phase::Ended("the chat program ended".to_owned());
            state.dirty = true;
            state.unsaved = true;
            return;
        }
        chat.phase = Phase::Reconnecting;
        state.dirty = true;
        drop(state);
        thread::sleep(RETRY);
    }
}

fn closing(shared: &Shared, id: &str) -> bool {
    let state = lock(shared);
    state.detached || state.chat(id).is_none_or(|chat| chat.closing)
}

/// One connection, until it drops; true when the program said it exits.
fn session(shared: &Shared, host: &Handle, id: &str, mut stream: UnixStream) -> bool {
    let since = lock(shared).chat(id).and_then(|chat| chat.cursor);
    let hello = ClientMessage::Hello {
        client: ClientKind::Gui,
        revision: REVISION,
        since,
        version: VERSION,
    };
    if write_frame(&mut stream, &hello).is_err() {
        return false;
    }
    match stream.try_clone() {
        Ok(writer) => {
            let mut state = lock(shared);
            if let Some(chat) = state.chat_mut(id) {
                chat.writer = Some(writer);
            }
        },
        Err(_) => return false,
    }
    loop {
        let message = match read_frame::<_, ServerMessage>(&mut stream) {
            Ok(Some(message)) => message,
            Ok(None) | Err(_) => return false,
        };
        let mut state = lock(shared);
        let Some(chat) = state.chat_mut(id) else {
            return true;
        };
        match message {
            ServerMessage::Welcome { .. } => {
                // A valid cursor is resumed with changes, not a snapshot.
                if chat.cursor.is_some() && chat.transcript.is_some() {
                    chat.phase = Phase::Live;
                }
            },
            ServerMessage::Snapshot { cursor, transcript } => {
                chat.cursor = Some(cursor);
                chat.transcript = Some(*transcript);
                chat.phase = Phase::Live;
            },
            ServerMessage::Change { change, cursor } => {
                let applied = chat
                    .transcript
                    .as_mut()
                    .map(|transcript| transcript.apply(&change));
                match applied {
                    Some(Ok(())) => chat.cursor = Some(cursor),
                    // Out of step: start over from a snapshot.
                    Some(Err(_)) | None => {
                        chat.cursor = None;
                        return false;
                    },
                }
            },
            ServerMessage::Ack {
                id: command,
                rejected,
            } => {
                if let Some(pending) = state.pending.remove(&command) {
                    let answer = match rejected {
                        Some(why) => json!({
                            "reply": pending.reply, "action": pending.action,
                            "ok": false, "error": why.to_string(),
                        }),
                        None => json!({
                            "reply": pending.reply, "action": pending.action, "ok": true,
                        }),
                    };
                    send(host, &pending.owner, &answer);
                }
            },
            ServerMessage::Bye { .. } => return true,
        }
        if let Some(session) = state
            .chat(id)
            .and_then(|chat| chat.transcript.as_ref())
            .and_then(|transcript| transcript.session.as_ref())
            .map(|session| session.id.clone())
            && state
                .chat(id)
                .is_some_and(|chat| chat.session.as_deref() != Some(&session))
        {
            if let Some(chat) = state.chat_mut(id) {
                chat.session = Some(session);
            }
            state.unsaved = true;
        }
        state.dirty = true;
    }
}

/// Sends `command` to the chat's program; false when it is not attached.
pub(crate) fn command(writer: &mut UnixStream, id: CommandId, command: Command) -> bool {
    write_frame(writer, &ClientMessage::Command { command, id }).is_ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
