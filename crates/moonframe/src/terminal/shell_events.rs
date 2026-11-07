//! A shell's [`Signal`]s as the messages a mod reads.

use {crate::browser::with, moonframe_core::Data, moonframe_terminal::Signal};

/// `msg`, the element's `shell`, with `event` and what happened added.
pub(super) fn message(msg: &Data, signal: Signal) -> Data {
    let text = |value: Option<String>| value.map_or(Data::Nil, Data::String);
    match signal {
        Signal::Prompt => with(msg, [("event", Data::from("prompt"))]),
        Signal::Started { command } => with(msg, [
            ("event", Data::from("started")),
            ("command", text(command)),
        ]),
        Signal::Finished {
            command,
            status,
            took,
        } => with(msg, [
            ("event", Data::from("finished")),
            ("command", text(command)),
            (
                "status",
                status.map_or(Data::Nil, |code| Data::Integer(code.into())),
            ),
            (
                "took_ms",
                Data::Integer(i64::try_from(took.as_millis()).unwrap_or(i64::MAX)),
            ),
        ]),
        Signal::Folder { path, host } => with(msg, [
            ("event", Data::from("folder")),
            ("path", Data::String(path)),
            ("host", text(host)),
        ]),
        Signal::Title(title) => with(msg, [
            ("event", Data::from("title")),
            ("title", Data::String(title)),
        ]),
        Signal::Notice { title, body } => with(msg, [
            ("event", Data::from("notice")),
            ("title", text(title)),
            ("body", Data::String(body)),
        ]),
    }
}
