//! `app:window`: opens a mod in a window of its own, or raises it, as a
//! control's `window = "<mod>"` does, and closes it again. Any mod may ask
//! for any mod's window, as any control may: the window shows that mod's
//! own view, and what `send` carries reaches its `update` as any message
//! between mods would, so nothing new crosses. The shell is the main
//! window, never a mod's.

use {
    super::APP,
    crate::{Data, Error, SHELL},
};

/// What `app:window` asks.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowRequest {
    /// Opens the mod `name`'s window, or raises it. `send`, delivered to
    /// its `update` before the window draws, lets it open on a page.
    Open { name: String, send: Option<Data> },
    /// Closes the mod `name`'s window, if it is open.
    Close(String),
}

/// `"<mod>"`, `{ open = "<mod>", send = <msg> }` or `{ close = "<mod>" }`.
pub(super) fn window(msg: &Data) -> Result<WindowRequest, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}window`: {problem}"));
    let usage = "takes \"<mod>\", { open = \"<mod>\", send = <msg> } or { close = \"<mod>\" }";
    let name = |value: &Data| match value {
        Data::String(name) if name == SHELL => {
            Err(fail("the shell is the main window, not a mod's"))
        },
        Data::String(name) if crate::style::valid_name(name) => Ok(name.clone()),
        Data::String(name) => Err(fail(&format!(
            "`{name}` is not a mod's name (a-z, 0-9, - and _)"
        ))),
        _ => Err(fail(usage)),
    };
    let fields = match msg {
        Data::String(_) => {
            return Ok(WindowRequest::Open {
                name: name(msg)?,
                send: None,
            });
        },
        Data::Map(fields) => fields,
        _ => return Err(fail(usage)),
    };
    if let Some(key) = fields
        .keys()
        .find(|key| !matches!(key.as_str(), "open" | "send" | "close"))
    {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes open, send and close"
        )));
    }
    match (fields.get("open"), fields.get("close")) {
        (Some(open), None) => Ok(WindowRequest::Open {
            name: name(open)?,
            send: fields
                .get("send")
                .cloned()
                .filter(|send| *send != Data::Nil),
        }),
        (None, Some(close)) if !fields.contains_key("send") => {
            Ok(WindowRequest::Close(name(close)?))
        },
        (None, Some(_)) => Err(fail("`send` goes with `open`")),
        (Some(_), Some(_)) => Err(fail("takes `open` or `close`, not both")),
        (None, None) => Err(fail(usage)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
