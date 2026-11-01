//! What a module asks of the app itself, from `update` (or a control's
//! `msg`), by sending to `app:<what>`: `{ to = "app:command", msg =
//! "minimize" }`. The workspace checks each request and keeps it for the
//! app to carry out; a request it cannot read is the sender's error.
//! Previews (the Components window) ask nothing: their sends to the app go
//! nowhere, as their commands do.

use crate::{Command, Data, Error};

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
}

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
pub const NAMES: &[&str] = &["command", "appearance"];

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
