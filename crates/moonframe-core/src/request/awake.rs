//! `app:awake`: a mod keeps the machine from sleeping while something long
//! runs (a scan, a render). Each mod holds for itself, and the machine stays
//! awake while any does. Leaving the app ends every hold, even in a crash.

use {
    super::APP,
    crate::{Data, Error},
};

/// Longest the reason shown in `pmset -g assertions` can be.
const MAX_REASON: usize = 128;

/// What `app:awake` asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Awake {
    /// Holds for the mod that asked.
    Hold {
        /// Keeps the display on as well, so the screen never locks.
        display: bool,
        /// Why, for the system's list of what is keeping it awake.
        reason: Option<String>,
    },
    /// Lets go of that mod's hold.
    Release,
}

/// `{ hold = true, display = false, reason = "Scanning" }`, or
/// `{ hold = false }`. `display` starts on.
pub(super) fn awake(msg: &Data) -> Result<Awake, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}awake`: {problem}"));
    let usage = "takes { hold = true, display = <bool>, reason = \"...\" } or { hold = false }";
    let Data::Map(fields) = msg else {
        return Err(fail(usage));
    };
    if let Some(key) = fields
        .keys()
        .find(|key| !matches!(key.as_str(), "hold" | "display" | "reason"))
    {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes hold, display and reason"
        )));
    }
    match fields.get("hold") {
        Some(Data::Bool(false)) if fields.len() == 1 => Ok(Awake::Release),
        Some(Data::Bool(true)) => {
            let display = match fields.get("display") {
                None => true,
                Some(Data::Bool(display)) => *display,
                Some(_) => return Err(fail("`display` is true or false")),
            };
            let reason = match fields.get("reason") {
                None => None,
                Some(Data::String(reason)) if reason.len() <= MAX_REASON => Some(reason.clone()),
                Some(_) => {
                    return Err(fail(&format!(
                        "`reason` is text of at most {MAX_REASON} bytes"
                    )));
                },
            };
            Ok(Awake::Hold { display, reason })
        },
        _ => Err(fail(usage)),
    }
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
    fn it_reads_holding_and_letting_go() {
        assert_eq!(
            awake(&map(&[("hold", Data::Bool(true))])).unwrap(),
            Awake::Hold {
                display: true,
                reason: None
            }
        );
        assert_eq!(
            awake(&map(&[
                ("hold", Data::Bool(true)),
                ("display", Data::Bool(false)),
                ("reason", Data::from("Scanning")),
            ]))
            .unwrap(),
            Awake::Hold {
                display: false,
                reason: Some("Scanning".to_owned())
            }
        );
        assert_eq!(
            awake(&map(&[("hold", Data::Bool(false))])).unwrap(),
            Awake::Release
        );
    }

    #[test]
    fn it_refuses_what_it_cannot_read() {
        for msg in [
            Data::Bool(true),
            map(&[]),
            map(&[("nope", Data::Bool(true))]),
            map(&[("hold", Data::Bool(false)), ("display", Data::Bool(true))]),
            map(&[("hold", Data::Bool(true)), ("display", Data::from("yes"))]),
            map(&[
                ("hold", Data::Bool(true)),
                ("reason", Data::from("x".repeat(MAX_REASON + 1).as_str())),
            ]),
        ] {
            assert!(awake(&msg).is_err(), "{msg:?}");
        }
    }
}
