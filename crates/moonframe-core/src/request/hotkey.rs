//! `app:hotkey`: a system-wide shortcut for a mod, heard whichever app is
//! in front: `{ keys = "ctrl-shift-space", msg = "capture" }` sends the
//! mod (or `to`) its `msg` at each press, until it gives the keys back
//! with `{ keys = ..., release = true }`. Whether each was claimed, or is
//! another app's, shows in `app.hotkeys`. Only macOS has them yet.
//!
//! Keys need two modifiers (ctrl, alt, shift, cmd or secondary, at least
//! one of them not shift), or a function key: one modifier alone takes
//! what every app uses, ⌘C, Ctrl-A in a text field, ⌥E for an accent.

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
    /// Gives `keys` back, under any spelling of them.
    Release { keys: String, chord: Chord },
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
    match fields.get("release") {
        None => {},
        Some(Data::Bool(true)) if fields.len() == 2 => {
            return Ok(Hotkey::Release {
                keys: keys.clone(),
                chord,
            });
        },
        Some(_) => return Err(fail("`release = true` comes with `keys` alone")),
    }
    if !for_every_app(&chord) {
        return Err(fail(&format!(
            "`{keys}` would take a key or shortcut every app uses; use two modifiers, \
             such as ctrl-alt or ctrl-shift, or a function key"
        )));
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

/// Whether `chord` may be taken from every app: a function key, or two
/// modifiers, at least one of them not shift.
fn for_every_app(chord: &Chord) -> bool {
    let function = chord
        .key
        .strip_prefix('f')
        .is_some_and(|number| number.parse::<u32>().is_ok());
    let held = [
        chord.ctrl,
        chord.alt,
        chord.shift,
        chord.cmd || chord.secondary,
    ];
    let count = held.into_iter().filter(|on| *on).count();
    function || (count >= 2 && (chord.ctrl || chord.alt || chord.cmd || chord.secondary))
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
        let Hotkey::Release { keys, chord } = hotkey(&map(&[
            ("keys", Data::from("f5")),
            ("release", Data::Bool(true)),
        ]))
        .unwrap() else {
            panic!("not a release");
        };
        assert_eq!((keys.as_str(), chord.key.as_str()), ("f5", "f5"));
        for keys in ["ctrl-alt-l", "cmd-shift-k", "secondary-alt-k", "shift-f5"] {
            let claim = map(&[("keys", Data::from(keys)), ("msg", Data::from("x"))]);
            assert!(hotkey(&claim).is_ok(), "{keys}");
        }
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
            (
                map(&[("keys", Data::from("cmd-c")), ("msg", Data::from("x"))]),
                "every app",
            ),
            (
                map(&[("keys", Data::from("ctrl-a")), ("msg", Data::from("x"))]),
                "every app",
            ),
            (
                map(&[("keys", Data::from("alt-e")), ("msg", Data::from("x"))]),
                "every app",
            ),
            (
                map(&[("keys", Data::from("ctrl-alt-k"))]),
                "needs the `msg`",
            ),
            (
                map(&[
                    ("keys", Data::from("ctrl-alt-k")),
                    ("msg", Data::from("x")),
                    ("release", Data::Bool(true)),
                ]),
                "alone",
            ),
            (
                map(&[
                    ("keys", Data::from("ctrl-alt-k")),
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
