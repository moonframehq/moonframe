//! `app:devtools`: what the Developer tools mod asks of the app: where the
//! tools show (docked beside the window, in a window of their own, or
//! not at all), a mod's source to read or save, and a mod's state. The
//! answers come back to the mod that asked, as its `reply` with fields
//! added. The app checks that a path is a Luau file of the app's own
//! folders before reading or writing it.

use {
    super::APP,
    crate::{Data, Error},
};

/// Longest source `save` writes: a mod's file, not a dump.
pub const MAX_SOURCE: usize = 1024 * 1024;
/// Longest path or mod name it takes.
const MAX_NAME: usize = 1024;

/// What `app:devtools` asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Devtools {
    /// `{ place = "dock" | "window" | "closed" }`: where the tools show.
    Place(DevtoolsPlace),
    /// `{ read = path, reply }`: the file's text, as `text`.
    Read { path: String, reply: Data },
    /// `{ save = path, text, reply }`: writes it, and the window reloads.
    Save {
        path: String,
        text: String,
        reply: Data,
    },
    /// `{ state = mod, reply }`: the mod's state, as `state`.
    State { module: String, reply: Data },
    /// `{ select = path }`: selects instead the element drawn at that
    /// path, one around or inside the selection, as its trail names them.
    Select(String),
}

tokens! {
    /// Where the Developer tools show.
    DevtoolsPlace {
        /// Beside the main window's content, on its right.
        Dock = "dock",
        /// In a window of their own.
        Window = "window",
        Closed = "closed",
    }
}

const USAGE: &str = "takes { place = \"dock\" | \"window\" | \"closed\" }, { read = path, reply }, \
                     { save = path, text, reply }, { state = mod, reply } or { select = path }";

pub(super) fn devtools(msg: &Data) -> Result<Devtools, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}devtools`: {problem}"));
    let Data::Map(fields) = msg else {
        return Err(fail(USAGE));
    };
    let name = |key: &str| match fields.get(key) {
        Some(Data::String(name)) if !name.is_empty() && name.len() <= MAX_NAME => Ok(name.clone()),
        _ => Err(fail(&format!(
            "`{key}` is a name of at most {MAX_NAME} bytes"
        ))),
    };
    let reply = || fields.get("reply").cloned().unwrap_or(Data::Nil);
    let only = |allowed: &[&str]| match fields.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(fail(&format!("unknown key `{key}`; {USAGE}"))),
        None => Ok(()),
    };
    if let Some(place) = fields.get("place") {
        only(&["place"])?;
        return match place {
            Data::String(place) => DevtoolsPlace::parse(place)
                .map(Devtools::Place)
                .ok_or_else(|| fail("`place` is \"dock\", \"window\" or \"closed\"")),
            _ => Err(fail("`place` is \"dock\", \"window\" or \"closed\"")),
        };
    }
    if fields.contains_key("read") {
        only(&["read", "reply"])?;
        return Ok(Devtools::Read {
            path: name("read")?,
            reply: reply(),
        });
    }
    if fields.contains_key("save") {
        only(&["save", "text", "reply"])?;
        let text = match fields.get("text") {
            Some(Data::String(text)) if text.len() <= MAX_SOURCE => text.clone(),
            _ => {
                return Err(fail(&format!(
                    "`save` goes with `text`, the source, at most {MAX_SOURCE} bytes"
                )));
            },
        };
        return Ok(Devtools::Save {
            path: name("save")?,
            text,
            reply: reply(),
        });
    }
    if fields.contains_key("select") {
        only(&["select"])?;
        return Ok(Devtools::Select(name("select")?));
    }
    if fields.contains_key("state") {
        only(&["state", "reply"])?;
        return Ok(Devtools::State {
            module: name("state")?,
            reply: reply(),
        });
    }
    Err(fail(USAGE))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, std::collections::BTreeMap};

    fn map(fields: &[(&str, Data)]) -> Data {
        Data::Map(
            fields
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    #[test]
    fn it_reads_each_kind_of_request() {
        assert_eq!(
            devtools(&map(&[("place", Data::from("window"))])).unwrap(),
            Devtools::Place(DevtoolsPlace::Window)
        );
        assert_eq!(
            devtools(&map(&[
                ("read", Data::from("mods/a/main.luau")),
                ("reply", Data::from("got"))
            ]))
            .unwrap(),
            Devtools::Read {
                path: "mods/a/main.luau".to_owned(),
                reply: Data::from("got"),
            }
        );
        assert_eq!(
            devtools(&map(&[
                ("save", Data::from("a.luau")),
                ("text", Data::from("return 1"))
            ]))
            .unwrap(),
            Devtools::Save {
                path: "a.luau".to_owned(),
                text: "return 1".to_owned(),
                reply: Data::Nil,
            }
        );
        assert_eq!(
            devtools(&map(&[("state", Data::from("status"))])).unwrap(),
            Devtools::State {
                module: "status".to_owned(),
                reply: Data::Nil,
            }
        );
    }

    #[test]
    fn it_refuses_what_it_cannot_do() {
        assert!(devtools(&Data::from("dock")).is_err());
        assert!(devtools(&map(&[("place", Data::from("left"))])).is_err());
        assert!(devtools(&map(&[("save", Data::from("a.luau"))])).is_err());
        assert!(devtools(&map(&[("read", Data::from(""))])).is_err());
        assert!(
            devtools(&map(&[
                ("read", Data::from("a")),
                ("text", Data::from("b"))
            ]))
            .is_err()
        );
    }
}
