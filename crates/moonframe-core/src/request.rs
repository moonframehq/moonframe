//! What a module asks of the app itself, from `update` (or a control's
//! `msg`), by sending to `app:<what>`: `{ to = "app:command", msg =
//! "minimize" }`. The workspace checks each request and keeps it for the
//! app to carry out; a request it cannot read is the sender's error.
//! Previews (the Components window) ask nothing: their sends to the app go
//! nowhere, as their commands do.

use crate::{Command, Data, Error};

mod awake;
mod dock;
mod edit;
mod guard;
mod pick;
mod window;

pub use {
    awake::Awake,
    dock::Dock,
    edit::{Edit, EditChange, Splice},
    guard::Guard,
    pick::{Pick, PickKind},
    window::WindowRequest,
};

/// How a message names the app rather than a mod: `to = "app:command"`.
pub const APP: &str = "app:";

/// One thing a module asked the app to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// `app:command`: runs an app command, as a control's `command` does,
    /// with the same commands allowed.
    Command(Command),
    /// `app:appearance`: light, dark, or following the system; kept
    /// across launches.
    Appearance(Appearance),
    /// `app:theme`: the theme light windows show, the one dark windows
    /// show, or both, by name; kept across launches. The app checks the
    /// names.
    Theme {
        light: Option<String>,
        dark: Option<String>,
    },
    /// `app:sound`: plays a short sound file from the app's folders, or
    /// the app's own chime.
    Sound(Sound),
    /// `app:notify`: posts a system notification (a toast where there is
    /// none); a click on it brings the window forward and tells the mod
    /// that posted it, on `app/notification`, with its `id`.
    Notify(Notification),
    /// `app:pick`: the system's panel to open files, pick folders or
    /// name a file to save; the answer comes back to the mod that asked.
    Pick(Pick),
    /// `app:edit`: changes the text of one of the sending mod's fields
    /// around its selection, as a formatting toolbar or a completion does.
    Edit(Edit),
    /// `app:window`: opens a mod in a window of its own, or raises it,
    /// first delivering what it is sent; or closes it.
    Window(WindowRequest),
    /// `app:guard`: holds back closing the main window and quitting, so
    /// a mod can finish something first.
    Guard(Guard),
    /// `app:awake`: keeps the machine, and by default the display, from
    /// sleeping until this mod lets go; `app.awake` says how that stands.
    Awake(Awake),
    /// `app:dock`: a badge or a progress bar on the app's Dock icon.
    Dock(Dock),
    /// `app:restart`: starts a provider again, by name.
    Restart(String),
    /// `app:open`: opens a web address (`https://` or `http://`) in the
    /// system's browser: a pull request, a sign-in page.
    Open(String),
}

/// What `app:sound` plays.
#[derive(Clone, Debug, PartialEq)]
pub enum Sound {
    /// `"default"`: a chime the app makes itself.
    Chime,
    /// A file, as `ui.image` names one: relative to the sending mod's
    /// folder, `~/...`, or absolute, and inside the app's folders.
    File(String),
}

/// What `app:notify` posts.
#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    pub title: String,
    pub body: String,
    /// What the mod hears back when it is clicked.
    pub id: Option<String>,
}

/// Longest a notification's title, its body and its id can be, in bytes.
pub const MAX_TITLE: usize = 256;
pub const MAX_BODY: usize = 2048;
pub const MAX_ID: usize = 128;
/// Longest a sound file's name can be.
const MAX_PATH: usize = 1024;
/// The sound files `app:sound` plays, by extension.
pub const SOUND_TYPES: &[&str] = &["wav", "aiff", "aif", "caf", "mp3", "m4a"];

tokens! {
    /// Whether the window is light or dark: as the system is (and as it
    /// changes), or always one of them.
    Appearance {
        System = "system",
        Light = "light",
        Dark = "dark",
    }
}

impl Appearance {
    /// The appearance called `name`, as saved.
    pub fn named(name: &str) -> Option<Self> {
        Self::parse(name)
    }

    /// Whether the window is dark in this appearance, when the system's is
    /// `system_dark`.
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

impl Request {
    /// The request a message to `app:<what>` makes.
    pub fn parse(what: &str, msg: &Data) -> Result<Self, Error> {
        match what {
            "command" => token(what, msg, Command::parse, Command::NAMES).map(Self::Command),
            "appearance" => {
                token(what, msg, Appearance::parse, Appearance::NAMES).map(Self::Appearance)
            },
            "theme" => theme(msg),
            "sound" => sound(msg).map(Self::Sound),
            "notify" => notification(msg).map(Self::Notify),
            "pick" => pick::pick(msg).map(Self::Pick),
            "edit" => edit::edit(msg).map(Self::Edit),
            "window" => window::window(msg).map(Self::Window),
            "guard" => guard::guard(msg).map(Self::Guard),
            "awake" => awake::awake(msg).map(Self::Awake),
            "dock" => dock::dock(msg).map(Self::Dock),
            "restart" => restart(msg).map(Self::Restart),
            "open" => open(msg).map(Self::Open),
            _ => Err(Error::new(format!(
                "no `{APP}{what}`; the app takes {}",
                NAMES
                    .iter()
                    .map(|name| format!("`{APP}{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }
}

/// What a module can send to, after `app:`.
pub const NAMES: &[&str] = &[
    "command",
    "appearance",
    "theme",
    "sound",
    "notify",
    "pick",
    "edit",
    "window",
    "guard",
    "awake",
    "dock",
    "restart",
    "open",
];

/// Longest address `app:open` opens.
const MAX_URL: usize = 2048;

/// A provider's name.
fn restart(msg: &Data) -> Result<String, Error> {
    match msg {
        Data::String(name) if !name.is_empty() => Ok(name.clone()),
        _ => Err(Error::new(format!(
            "`{APP}restart` takes a provider's name"
        ))),
    }
}

/// A web address, never a file or another program's scheme.
fn open(msg: &Data) -> Result<String, Error> {
    match msg {
        Data::String(url) if url.len() <= MAX_URL && crate::is_remote_url(url) => Ok(url.clone()),
        _ => Err(Error::new(format!(
            "`{APP}open` takes a web address (https:// or http://, at most {MAX_URL} bytes)"
        ))),
    }
}

/// `"default"`, or a sound file's path: never an address.
fn sound(msg: &Data) -> Result<Sound, Error> {
    let types = SOUND_TYPES.join(", ");
    let Data::String(name) = msg else {
        return Err(Error::new(format!(
            "`{APP}sound` takes \"default\" or a sound file's path ({types})"
        )));
    };
    if name == "default" {
        return Ok(Sound::Chime);
    }
    if name.contains("://") {
        return Err(Error::new(format!(
            "`{APP}sound`: `{name}` is an address; it plays files in the app's folders"
        )));
    }
    let known = std::path::Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| SOUND_TYPES.contains(&extension.to_lowercase().as_str()));
    if name.is_empty() || name.len() > MAX_PATH || !known {
        return Err(Error::new(format!(
            "`{APP}sound`: `{name}` is not a sound file ({types})"
        )));
    }
    Ok(Sound::File(name.clone()))
}

/// `{ title = "...", body = "...", id = "..." }`, `body` and `id` optional.
fn notification(msg: &Data) -> Result<Notification, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}notify`: {problem}"));
    let Data::Map(fields) = msg else {
        return Err(fail(
            "takes { title = \"...\", body = \"...\", id = \"...\" }",
        ));
    };
    if let Some(key) = fields
        .keys()
        .find(|key| !matches!(key.as_str(), "title" | "body" | "id"))
    {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes title, body and id"
        )));
    }
    let text = |key: &str, max: usize| match fields.get(key) {
        None => Ok(None),
        Some(Data::String(text)) if text.len() <= max => Ok(Some(text.clone())),
        Some(Data::String(_)) => Err(fail(&format!("`{key}` is longer than {max} bytes"))),
        Some(_) => Err(fail(&format!("`{key}` is not a string"))),
    };
    let title = text("title", MAX_TITLE)?.filter(|title| !title.trim().is_empty());
    let id = text("id", MAX_ID)?;
    // It travels with the notification, and comes back as it was.
    if id.as_ref().is_some_and(|id| {
        id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/'))
    }) {
        return Err(fail(&format!(
            "`id` is 1 to {MAX_ID} of a-z, A-Z, 0-9, - _ . : /"
        )));
    }
    Ok(Notification {
        title: title.ok_or_else(|| fail("needs a `title`"))?,
        body: text("body", MAX_BODY)?.unwrap_or_default(),
        id,
    })
}

/// `{ light = "<theme>", dark = "<theme>" }`, either or both.
fn theme(msg: &Data) -> Result<Request, Error> {
    let fail = || {
        Error::new(format!(
            "`{APP}theme` takes {{ light = \"<theme>\", dark = \"<theme>\" }}, either or both"
        ))
    };
    let Data::Map(fields) = msg else {
        return Err(fail());
    };
    if fields.is_empty() || fields.keys().any(|key| key != "light" && key != "dark") {
        return Err(fail());
    }
    let name = |key: &str| match fields.get(key) {
        None => Ok(None),
        Some(Data::String(name)) => Ok(Some(name.clone())),
        Some(_) => Err(fail()),
    };
    Ok(Request::Theme {
        light: name("light")?,
        dark: name("dark")?,
    })
}

/// `msg` read as one of `names`, for the request `app:<what>`.
fn token<T>(
    what: &str,
    msg: &Data,
    parse: fn(&str) -> Option<T>,
    names: &[&str],
) -> Result<T, Error> {
    let names = names.join(", ");
    match msg {
        Data::String(name) => parse(name).ok_or_else(|| {
            Error::new(format!(
                "`{APP}{what}`: unknown `{name}`; expected one of {names}"
            ))
        }),
        _ => Err(Error::new(format!("`{APP}{what}` takes one of {names}"))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
