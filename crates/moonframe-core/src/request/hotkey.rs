//! `app:hotkey`: a system-wide shortcut for a mod, heard whichever app is
//! in front: `{ keys = "ctrl-shift-space", msg = "capture" }` sends the
//! mod (or `to`) its `msg` at each press, until it gives the keys back
//! with `{ keys = ..., release = true }`. Whether each was claimed, or is
//! another app's, shows in `app.hotkeys`. Only macOS has them yet.

use {
    super::APP,
    crate::{Chord, Data, Error, SHELL},
};

/// What `app:hotkey` asks.
#[derive(Clone, Debug, PartialEq)]
pub enum Hotkey {
    /// Claims `keys` for the mod: each press sends `msg` to `to`, or to
    /// the mod.
    Claim {
        /// As the mod wrote them: their name in `app.hotkeys`.
        keys: String,
        chord: Chord,
        msg: Data,
        to: Option<String>,
    },
    /// Gives `keys` back.
    Release(String),
}

/// `{ keys, msg, to }` or `{ keys, release = true }`.
pub(super) fn hotkey(msg: &Data) -> Result<Hotkey, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}hotkey`: {problem}"));
    let usage = "takes { keys = \"ctrl-shift-space\", msg = <msg>, to = \"<mod>\" } or \
                 { keys = \"...\", release = true }";
    let Data::Map(fields) = msg else {
        return Err(fail(usage));
    };
    if let Some(key) = fields
        .keys()
        .find(|key| !matches!(key.as_str(), "keys" | "msg" | "to" | "release"))
    {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes keys, msg, to and release"
        )));
    }
    let Some(Data::String(keys)) = fields.get("keys") else {
        return Err(fail(usage));
    };
    let chord = Chord::parse(keys).map_err(|problem| fail(&problem))?;
    let function = chord
        .key
        .strip_prefix('f')
        .is_some_and(|number| number.parse::<u32>().is_ok());
    if !(chord.ctrl || chord.alt || chord.cmd || chord.secondary || function) {
        return Err(fail(&format!(
            "`{keys}` would take a key from every app; add ctrl, alt, cmd or secondary"
        )));
    }
    match fields.get("release") {
        None => {},
        Some(Data::Bool(true)) if fields.len() == 2 => return Ok(Hotkey::Release(keys.clone())),
        Some(_) => return Err(fail("`release = true` comes with `keys` alone")),
    }
    let msg = match fields.get("msg") {
        None | Some(Data::Nil) => return Err(fail("needs the `msg` a press sends")),
        Some(msg) => msg.clone(),
    };
    let to = match fields.get("to") {
        None => None,
        Some(Data::String(to)) if to == SHELL || crate::entries::module_name(to) => {
            Some(to.clone())
        },
        Some(_) => return Err(fail("`to` is a mod's name, or \"shell\"")),
    };
    Ok(Hotkey::Claim {
        keys: keys.clone(),
        chord,
        msg,
        to,
    })
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
    fn a_hotkey_is_claimed_and_given_back() {
        let claim = hotkey(&map(&[
            ("keys", Data::from("ctrl-shift-space")),
            ("msg", Data::from("capture")),
            ("to", Data::from("capture")),
        ]))
        .unwrap();
        let Hotkey::Claim {
            keys,
            chord,
            msg,
            to,
        } = claim
        else {
            panic!("{claim:?}");
        };
        assert_eq!(keys, "ctrl-shift-space");
        assert!(chord.ctrl && chord.shift && chord.key == "space");
        assert_eq!(msg, Data::from("capture"));
        assert_eq!(to.as_deref(), Some("capture"));
        assert_eq!(
            hotkey(&map(&[
                ("keys", Data::from("f5")),
                ("release", Data::Bool(true))
            ]))
            .unwrap(),
            Hotkey::Release("f5".to_owned())
        );
    }

    #[test]
    fn keys_every_app_needs_are_refused() {
        for (bad, says) in [
            (
                map(&[("keys", Data::from("k")), ("msg", Data::from("x"))]),
                "types text",
            ),
            (
                map(&[("keys", Data::from("escape")), ("msg", Data::from("x"))]),
                "every app",
            ),
            (
                map(&[
                    ("keys", Data::from("shift-enter")),
                    ("msg", Data::from("x")),
                ]),
                "every app",
            ),
            (map(&[("keys", Data::from("ctrl-k"))]), "needs the `msg`"),
            (
                map(&[
                    ("keys", Data::from("ctrl-k")),
                    ("msg", Data::from("x")),
                    ("release", Data::Bool(true)),
                ]),
                "alone",
            ),
            (
                map(&[
                    ("keys", Data::from("ctrl-k")),
                    ("msg", Data::from("x")),
                    ("to", Data::from("not a mod")),
                ]),
                "`to`",
            ),
            (Data::from("ctrl-k"), "takes"),
        ] {
            let problem = hotkey(&bad).unwrap_err().to_string();
            assert!(problem.contains(says), "{bad:?}: {problem}");
        }
    }
}
