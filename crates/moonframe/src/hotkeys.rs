//! System-wide shortcuts mods claim (`app:hotkey`), kept app-wide: each
//! keys belongs to one mod, whatever their spelling (`secondary` is ⌘ on
//! macOS), and a press is queued for the app, which hands the claim's
//! `msg` to the front main window's mod; see `app::menu_bar`. macOS
//! registers them with Carbon's hot keys, which need no permission;
//! elsewhere a claim stands as `"unsupported"`. Which are claimed shows as
//! `app.hotkeys`: `{ ["ctrl-shift-space"] = "claimed" | "taken" |
//! "unsupported" }`, `"taken"` when the system refused them because
//! another app holds them for itself alone. An app that shares them, and
//! the system's own shortcuts, go unreported.

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
    /// Another app holds them for itself alone: asking again tries again.
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
    /// As the system takes it: [`canonical`].
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
/// again changes what it sends, and tries the system again when they
/// were [`State::Taken`]. Keys held here under another spelling, by
/// another mod or this one, are refused; another app's stand as
/// [`State::Taken`].
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
    let chord = canonical(&chord);
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
            if claim.state == State::Taken {
                (claim.state, claim._registered) = register(keys, &claim.chord)?;
            }
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

/// Gives `chord` back, as the mod `owner` spelt it in `keys` or not, if
/// that mod holds it.
pub(crate) fn release(owner: &str, keys: &str, chord: &Chord) -> Result<(), String> {
    let chord = canonical(chord);
    CLAIMS.with_borrow_mut(|claims| {
        let Some((held, holder)) = claims
            .iter()
            .find(|(_, claim)| claim.chord == chord)
            .map(|(held, claim)| (held.clone(), claim.owner.clone()))
        else {
            return Ok(());
        };
        if holder != owner {
            return Err(format!("`{keys}` is held by {holder}"));
        }
        claims.remove(&held);
        Ok(())
    })
}

/// Gives back each claim `stale` names, by its mod and its keys as
/// written: whether there was any.
pub(crate) fn forget(stale: impl Fn(&str, &str) -> bool) -> bool {
    CLAIMS.with_borrow_mut(|claims| {
        let before = claims.len();
        claims.retain(|keys, claim| !stale(&claim.owner, keys));
        claims.len() != before
    })
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

/// `chord` as the system takes it: `secondary` is ⌘ on macOS, Ctrl
/// elsewhere, so `cmd-alt-k` and `secondary-alt-k` are the same keys.
fn canonical(chord: &Chord) -> Chord {
    let mut chord = chord.clone();
    if std::mem::take(&mut chord.secondary) {
        if cfg!(target_os = "macos") {
            chord.cmd = true;
        } else {
            chord.ctrl = true;
        }
    }
    chord
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

    fn claim(owner: &str, keys: &str, state: State) -> Result<(), String> {
        claim_with(
            owner,
            keys,
            chord(keys),
            owner.to_owned(),
            Data::from("go"),
            fake(state),
        )
    }

    fn state(keys: &str) -> State {
        CLAIMS.with_borrow(|claims| claims[keys].state)
    }

    #[test]
    fn each_keys_belong_to_one_mod() {
        assert!(claim("capture", "ctrl-shift-space", State::Claimed).is_ok());
        assert!(claim("other", "f5", State::Taken).is_ok());
        assert_eq!(
            data(),
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
        assert_eq!(state("ctrl-shift-space"), State::Claimed);
        assert!(release("other", "ctrl-shift-space", &chord("ctrl-shift-space")).is_err());
        // Given back under another spelling.
        assert!(release("capture", "shift-ctrl-space", &chord("shift-ctrl-space")).is_ok());
        assert!(forget(|owner, _| owner == "other"));
        assert!(!forget(|owner, _| owner == "other"));
        assert_eq!(data(), Data::Map(BTreeMap::new()));
        assert!(owners().is_empty());
    }

    #[test]
    fn secondary_is_the_key_it_stands_for() {
        let same = if cfg!(target_os = "macos") {
            "cmd-alt-k"
        } else {
            "ctrl-alt-k"
        };
        assert!(claim("capture", "secondary-alt-k", State::Claimed).is_ok());
        let problem = claim("other", same, State::Claimed).unwrap_err();
        assert!(
            problem.contains("as `secondary-alt-k` by capture"),
            "{problem}"
        );
        assert!(release("capture", same, &chord(same)).is_ok());
        assert!(owners().is_empty());
    }

    #[test]
    fn keys_another_app_held_are_tried_again() {
        assert!(claim("capture", "ctrl-alt-k", State::Taken).is_ok());
        assert_eq!(state("ctrl-alt-k"), State::Taken);
        assert!(claim("capture", "ctrl-alt-k", State::Claimed).is_ok());
        assert_eq!(state("ctrl-alt-k"), State::Claimed);
    }

    #[test]
    fn only_the_claims_named_are_forgotten() {
        assert!(claim("capture", "ctrl-alt-k", State::Claimed).is_ok());
        assert!(claim("capture", "ctrl-alt-j", State::Claimed).is_ok());
        assert!(claim("other", "ctrl-alt-l", State::Claimed).is_ok());
        // `capture` loaded again asking for ctrl-alt-j alone.
        assert!(forget(
            |owner, keys| owner == "capture" && keys != "ctrl-alt-j"
        ));
        assert_eq!(
            data(),
            Data::Map(BTreeMap::from([
                ("ctrl-alt-j".to_owned(), Data::from("claimed")),
                ("ctrl-alt-l".to_owned(), Data::from("claimed")),
            ]))
        );
    }
}
