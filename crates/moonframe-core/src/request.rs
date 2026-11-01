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
}

impl Request {
    /// The request a message to `app:<what>` makes.
    pub fn parse(what: &str, msg: &Data) -> Result<Self, Error> {
        match what {
            "command" => {
                let names = Command::NAMES.join(", ");
                let Data::String(name) = msg else {
                    return Err(Error::new(format!(
                        "`app:command` takes a command's name: one of {names}"
                    )));
                };
                Command::parse(name).map(Self::Command).ok_or_else(|| {
                    Error::new(format!(
                        "`app:command`: unknown `{name}`; expected one of {names}"
                    ))
                })
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
pub const NAMES: &[&str] = &["command"];

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
