//! A shell's [`Signal`]s as the messages a mod reads.

use {
    moonframe_core::{Data, Markdown},
    moonframe_terminal::Signal,
};

/// `msg`, the element's `shell`, with `event` and what happened added.
pub(super) fn message(msg: &Data, signal: Signal) -> Data {
    let text = |value: Option<String>| value.map_or(Data::Nil, Data::String);
    match signal {
        Signal::Prompt => Markdown::note(msg, [("event", Data::from("prompt"))]),
        Signal::Started { command } => Markdown::note(msg, [
            ("event", Data::from("started")),
            ("command", text(command)),
        ]),
        Signal::Finished {
            command,
            status,
            took,
        } => Markdown::note(msg, [
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
        Signal::Folder { path, host } => Markdown::note(msg, [
            ("event", Data::from("folder")),
            ("path", Data::String(path)),
            ("host", text(host)),
        ]),
        Signal::Title(title) => Markdown::note(msg, [
            ("event", Data::from("title")),
            ("title", Data::String(title)),
        ]),
        Signal::Notice { title, body } => Markdown::note(msg, [
            ("event", Data::from("notice")),
            ("title", text(title)),
            ("body", Data::String(body)),
        ]),
    }
}
