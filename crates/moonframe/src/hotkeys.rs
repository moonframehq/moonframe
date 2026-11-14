//! System-wide shortcuts mods claim (`app:hotkey`), kept app-wide: each
//! keys belongs to one mod, and a press is queued for the app, which hands
//! the claim's `msg` to the front main window's mod; see `app::menu_bar`.
//! macOS registers them with Carbon's hot keys, which need no permission;
//! elsewhere a claim stands as `"unsupported"`. Which are claimed shows as
//! `app.hotkeys`: `{ ["ctrl-shift-space"] = "claimed" | "taken" |
//! "unsupported" }`, `"taken"` when another app holds them.

#[cfg(all(feature = "hotkey", target_os = "macos"))]
mod macos;

use {
    futures::channel::mpsc,
    moonframe_core::{Chord, Data},
    std::{cell::RefCell, collections::BTreeMap, sync::OnceLock},
};

/// How a claim stands with the system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    #[cfg_attr(not(all(feature = "hotkey", target_os = "macos")), allow(dead_code))]
    Claimed,
    /// Another app holds them.
    #[cfg_attr(not(all(feature = "hotkey", target_os = "macos")), allow(dead_code))]
    Taken,
    /// Not on this platform.
    #[cfg_attr(all(feature = "hotkey", target_os = "macos"), allow(dead_code))]
    Unsupported,
}

impl State {
    fn name(self) -> &'static str {
        match self {
            Self::Claimed => "claimed",
            Self::Taken => "taken",
            Self::Unsupported => "unsupported",
        }
    }
}

/// A press, for the app to deliver: `msg` for the mod `to`.
pub(crate) struct Pressed {
    pub(crate) to: String,
    pub(crate) msg: Data,
}

struct Claim {
    owner: String,
    chord: Chord,
    to: String,
    msg: Data,
    state: State,
    /// The system's registration, which lasts until dropped.
    _registered: Option<Registered>,
}

#[cfg(all(feature = "hotkey", target_os = "macos"))]
type Registered = macos::Registered;
#[cfg(not(all(feature = "hotkey", target_os = "macos")))]
type Registered = ();

thread_local! {
    /// Every claim, by the keys as the mod wrote them.
    static CLAIMS: RefCell<BTreeMap<String, Claim>> = const { RefCell::new(BTreeMap::new()) };
}

static PRESSES: OnceLock<mpsc::UnboundedSender<Pressed>> = OnceLock::new();

/// Hears the presses of every claimed hotkey from now on.
pub(crate) fn listen() -> Option<mpsc::UnboundedReceiver<Pressed>> {
    let (sender, presses) = mpsc::unbounded();
    PRESSES.set(sender).ok()?;
    Some(presses)
}

/// The claim on `keys` was pressed.
#[cfg_attr(not(all(feature = "hotkey", target_os = "macos")), allow(dead_code))]
fn pressed(keys: &str) {
    let press = CLAIMS.with_borrow(|claims| {
        claims.get(keys).map(|claim| Pressed {
            to: claim.to.clone(),
            msg: claim.msg.clone(),
        })
    });
    if let (Some(press), Some(presses)) = (press, PRESSES.get()) {
        let _ = presses.unbounded_send(press);
    }
}

/// Claims `keys` for the mod `owner`: a press sends `msg` to `to`. Asking
/// again changes what it sends. Keys another mod here holds (under any
/// spelling) are refused; another app's stand as [`State::Taken`].
pub(crate) fn claim(
    owner: &str,
    keys: &str,
    chord: Chord,
    to: String,
    msg: Data,
) -> Result<(), String> {
    claim_with(owner, keys, chord, to, msg, register)
}

fn claim_with(
    owner: &str,
    keys: &str,
    chord: Chord,
    to: String,
    msg: Data,
    register: impl FnOnce(&str, &Chord) -> Result<(State, Option<Registered>), String>,
) -> Result<(), String> {
    CLAIMS.with_borrow_mut(|claims| {
        if let Some((held, other)) = claims
            .iter()
            .find(|(held, claim)| claim.chord == chord && (claim.owner != owner || *held != keys))
        {
            return Err(format!(
                "`{keys}` is held already, as `{held}` by {}",
                other.owner
            ));
        }
        if let Some(claim) = claims.get_mut(keys) {
            claim.to = to;
            claim.msg = msg;
            return Ok(());
        }
        let (state, registered) = register(keys, &chord)?;
        claims.insert(keys.to_owned(), Claim {
            owner: owner.to_owned(),
            chord,
            to,
            msg,
            state,
            _registered: registered,
        });
        Ok(())
    })
}

/// Gives `keys` back, if the mod `owner` holds them.
pub(crate) fn release(owner: &str, keys: &str) -> Result<(), String> {
    CLAIMS.with_borrow_mut(|claims| match claims.get(keys) {
        Some(claim) if claim.owner == owner => {
            claims.remove(keys);
            Ok(())
        },
        Some(claim) => Err(format!("`{keys}` is held by {}", claim.owner)),
        None => Ok(()),
    })
}

/// Gives back what the mods `gone` held.
pub(crate) fn forget(gone: impl Fn(&str) -> bool) {
    CLAIMS.with_borrow_mut(|claims| claims.retain(|_, claim| !gone(&claim.owner)));
}

/// The mods holding any.
pub(crate) fn owners() -> Vec<String> {
    CLAIMS.with_borrow(|claims| claims.values().map(|claim| claim.owner.clone()).collect())
}

/// `app.hotkeys`.
pub(crate) fn data() -> Data {
    CLAIMS.with_borrow(|claims| {
        Data::Map(
            claims
                .iter()
                .map(|(keys, claim)| (keys.clone(), Data::from(claim.state.name())))
                .collect(),
        )
    })
}

#[cfg(all(feature = "hotkey", target_os = "macos"))]
fn register(keys: &str, chord: &Chord) -> Result<(State, Option<Registered>), String> {
    Ok(match macos::register(keys, chord)? {
        Some(registered) => (State::Claimed, Some(registered)),
        None => (State::Taken, None),
    })
}

#[cfg(not(all(feature = "hotkey", target_os = "macos")))]
fn register(_: &str, _: &Chord) -> Result<(State, Option<Registered>), String> {
    Ok((State::Unsupported, None))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn chord(name: &str) -> Chord {
        Chord::parse(name).unwrap_or_else(|problem| panic!("{problem}"))
    }

    fn fake(
        state: State,
    ) -> impl FnOnce(&str, &Chord) -> Result<(State, Option<Registered>), String> {
        move |_, _| Ok((state, None))
    }

    fn states() -> Data {
        data()
    }

    #[test]
    fn each_keys_belong_to_one_mod() {
        let claim = |owner: &str, keys: &str, state| {
            claim_with(
                owner,
                keys,
                chord(keys),
                owner.to_owned(),
                Data::from("go"),
                fake(state),
            )
        };
        assert!(claim("capture", "ctrl-shift-space", State::Claimed).is_ok());
        assert!(claim("other", "f5", State::Taken).is_ok());
        assert_eq!(
            states(),
            Data::Map(BTreeMap::from([
                ("ctrl-shift-space".to_owned(), Data::from("claimed")),
                ("f5".to_owned(), Data::from("taken")),
            ]))
        );
        // The same keys, another mod or another spelling: refused.
        let problem = claim("other", "ctrl-shift-space", State::Claimed).unwrap_err();
        assert!(problem.contains("held already"), "{problem}");
        let problem = claim("capture", "shift-ctrl-space", State::Claimed).unwrap_err();
        assert!(problem.contains("as `ctrl-shift-space`"), "{problem}");
        // Asked again, it only changes what a press sends.
        assert!(claim("capture", "ctrl-shift-space", State::Taken).is_ok());
        assert_eq!(
            CLAIMS.with_borrow(|claims| claims["ctrl-shift-space"].state),
            State::Claimed
        );
        assert!(release("other", "ctrl-shift-space").is_err());
        assert!(release("capture", "ctrl-shift-space").is_ok());
        forget(|owner| owner == "other");
        assert_eq!(states(), Data::Map(BTreeMap::new()));
        assert!(owners().is_empty());
    }
}
