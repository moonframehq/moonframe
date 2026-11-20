//! `app:window`: opens a mod in a window of its own, or raises it, as a
//! control's `window = "<mod>"` does, and closes it again. Any mod may ask
//! for any mod's window, as any control may: the window shows that mod's
//! own view, and what `send` carries reaches its `update` as any message
//! between mods would, so nothing new crosses. The shell is the main
//! window, never a mod's.

use {
    super::APP,
    crate::{
        Data, Display, Error, SHELL, WindowPlace,
        window::{MOD_WINDOW_HEIGHT, MOD_WINDOW_WIDTH},
    },
};

/// What `app:window` asks.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowRequest {
    /// Opens the mod `name`'s window, or raises it. `send`, delivered to
    /// its `update` before the window draws, lets it open on a page.
    /// `place`, when given, outranks what the mod's `window` key says.
    Open {
        name: String,
        send: Option<Data>,
        place: Option<WindowPlace>,
        /// `{ width, height }`: the size it opens at, over its `window`
        /// key's; an open window grows to it, never shrinks.
        size: Option<(f32, f32)>,
    },
    /// Closes the mod `name`'s window, if it is open.
    Close(String),
}

/// `"<mod>"`, `{ open = "<mod>", send = <msg>, display = "external",
/// fullscreen = true }` or `{ close = "<mod>" }`.
pub(super) fn window(msg: &Data) -> Result<WindowRequest, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}window`: {problem}"));
    let usage = "takes \"<mod>\", { open = \"<mod>\", send = <msg> } or { close = \"<mod>\" }";
    let name = |value: &Data| match value {
        Data::String(name) if name == SHELL => {
            Err(fail("the shell is the main window, not a mod's"))
        },
        Data::String(name) if crate::entries::module_name(name) => Ok(name.clone()),
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
                place: None,
                size: None,
            });
        },
        Data::Map(fields) => fields,
        _ => return Err(fail(usage)),
    };
    if let Some(key) = fields.keys().find(|key| {
        !matches!(
            key.as_str(),
            "open" | "send" | "close" | "display" | "fullscreen" | "size"
        )
    }) {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes open, send, display, fullscreen, size and close"
        )));
    }
    let size = match fields.get("size") {
        None | Some(Data::Nil) => None,
        Some(size) => Some(window_size(size).ok_or_else(|| {
            fail(&format!(
                "`size` is {{ width, height }}, width {}–{} and height {}–{}",
                MOD_WINDOW_WIDTH.start(),
                MOD_WINDOW_WIDTH.end(),
                MOD_WINDOW_HEIGHT.start(),
                MOD_WINDOW_HEIGHT.end()
            ))
        })?),
    };
    let display = match fields.get("display") {
        None | Some(Data::Nil) => None,
        Some(display) => Some(Display::from_data(display).ok_or_else(|| {
            fail("`display` is \"primary\", \"external\" or an id from `app.displays`")
        })?),
    };
    let fullscreen = match fields.get("fullscreen") {
        None | Some(Data::Nil) => None,
        Some(Data::Bool(fullscreen)) => Some(*fullscreen),
        Some(_) => return Err(fail("`fullscreen` is true or false")),
    };
    let place = (display.is_some() || fullscreen.is_some()).then_some(WindowPlace {
        display,
        fullscreen: fullscreen.unwrap_or(false),
    });
    match (fields.get("open"), fields.get("close")) {
        (Some(open), None) => Ok(WindowRequest::Open {
            name: name(open)?,
            send: fields
                .get("send")
                .cloned()
                .filter(|send| *send != Data::Nil),
            place,
            size,
        }),
        (None, Some(close))
            if !fields.contains_key("send") && place.is_none() && size.is_none() =>
        {
            Ok(WindowRequest::Close(name(close)?))
        },
        (None, Some(_)) if place.is_some() || size.is_some() => {
            Err(fail("`display`, `fullscreen` and `size` go with `open`"))
        },
        (None, Some(_)) => Err(fail("`send` goes with `open`")),
        (Some(_), Some(_)) => Err(fail("takes `open` or `close`, not both")),
        (None, None) => Err(fail(usage)),
    }
}

/// `{ width, height }` within what a mod's window may ask for.
fn window_size(value: &Data) -> Option<(f32, f32)> {
    let Data::Map(fields) = value else {
        return None;
    };
    if fields.keys().any(|key| key != "width" && key != "height") {
        return None;
    }
    let number = |key: &str| match fields.get(key)? {
        Data::Integer(value) => Some(*value as f32),
        Data::Number(value) => Some(*value as f32),
        _ => None,
    };
    let (width, height) = (number("width")?, number("height")?);
    (MOD_WINDOW_WIDTH.contains(&width) && MOD_WINDOW_HEIGHT.contains(&height))
        .then_some((width, height))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
