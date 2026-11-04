//! `app:guard`: a mod holds back closing the main window and quitting, to
//! finish something first (a capture to stop, a file to write). Instead of
//! closing, the app sends the mod its `hold` message with `closing` added
//! (`"window"` or `"quit"`). The mod does what it must and then says
//! `{ release = true }`, which closes or quits as first asked.
//!
//! `timeout` keeps a stuck mod from trapping the user: once that long has
//! passed since the first attempt, the next one goes through.

use {
    super::APP,
    crate::{Data, Error},
    std::time::Duration,
};

/// How long a mod may hold on after the first attempt, unless it says.
const DEFAULT_TIMEOUT: f64 = 30.0;
const MAX_TIMEOUT: f64 = 600.0;

/// What `app:guard` asks.
#[derive(Clone, Debug, PartialEq)]
pub enum Guard {
    /// Holds closing back, sending `reply` to the mod that asked.
    Hold { reply: Data, timeout: Duration },
    /// Stops holding; an attempt waiting is forgotten.
    Stop,
    /// Stops holding and carries out the attempt that was waiting.
    Release,
}

/// `{ hold = <msg>, timeout = 30 }`, `{ hold = false }` or
/// `{ release = true }`.
pub(super) fn guard(msg: &Data) -> Result<Guard, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}guard`: {problem}"));
    let usage =
        "takes { hold = <msg>, timeout = <seconds> }, { hold = false } or { release = true }";
    let Data::Map(fields) = msg else {
        return Err(fail(usage));
    };
    if let Some(key) = fields
        .keys()
        .find(|key| !matches!(key.as_str(), "hold" | "timeout" | "release"))
    {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes hold, timeout and release"
        )));
    }
    match (fields.get("hold"), fields.get("release")) {
        (None, Some(Data::Bool(true))) => Ok(Guard::Release),
        (Some(Data::Bool(false)), None) => Ok(Guard::Stop),
        (Some(reply), None) if *reply != Data::Nil => {
            let timeout = match fields.get("timeout") {
                None => DEFAULT_TIMEOUT,
                Some(Data::Integer(seconds)) => *seconds as f64,
                Some(Data::Number(seconds)) => *seconds,
                Some(_) => return Err(fail("`timeout` is a number of seconds")),
            };
            if !(0.0..=MAX_TIMEOUT).contains(&timeout) {
                return Err(fail(&format!("`timeout` is 0 to {MAX_TIMEOUT} seconds")));
            }
            Ok(Guard::Hold {
                reply: reply.clone(),
                timeout: Duration::from_secs_f64(timeout),
            })
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
    fn it_reads_holding_stopping_and_releasing() {
        assert_eq!(
            guard(&map(&[
                ("hold", Data::from("closing")),
                ("timeout", Data::Integer(5))
            ]))
            .unwrap(),
            Guard::Hold {
                reply: Data::from("closing"),
                timeout: Duration::from_secs(5),
            }
        );
        assert_eq!(
            guard(&map(&[("hold", Data::Bool(false))])).unwrap(),
            Guard::Stop
        );
        assert_eq!(
            guard(&map(&[("release", Data::Bool(true))])).unwrap(),
            Guard::Release
        );
    }

    #[test]
    fn it_refuses_what_it_cannot_read() {
        for msg in [
            Data::from("closing"),
            map(&[("nope", Data::Bool(true))]),
            map(&[
                ("hold", Data::from("closing")),
                ("timeout", Data::Integer(9999)),
            ]),
            map(&[("release", Data::Bool(false))]),
        ] {
            assert!(guard(&msg).is_err(), "{msg:?}");
        }
    }
}
