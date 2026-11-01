//! Keys an open overlay or popover takes for itself: `keys = { escape =
//! "close", down = "next", ["secondary-k"] = "palette" }`. While it is open
//! and on top, each of those keystrokes sends its message to the `update`
//! of the mod that drew it, before anything else sees the key (a focused
//! terminal included).
//!
//! A name is modifiers and a key joined by `-`, as GPUI writes keystrokes:
//! `escape`, `enter`, `up`, `shift-tab`, `cmd-k`, `ctrl-shift-p`. The
//! modifiers are `ctrl`, `alt`, `shift`, `cmd` (⌘ on macOS, the Super or
//! Windows key elsewhere) and `secondary` (⌘ on macOS, Ctrl elsewhere, for
//! shortcuts that should feel native on each).
//!
//! What a key types is never taken: a printable key (a letter, digit,
//! symbol or `space`) needs `ctrl`, `alt`, `cmd` or `secondary`, so an open
//! overlay cannot read what the user types into a terminal behind it. Text
//! entry goes through `ui.input`.

use {
    crate::{
        Data, Error,
        read::{Cx, Read},
    },
    mlua::Value,
};

/// Most keys one element can take.
const MAX_KEYS: usize = 64;

/// The named keys besides letters, digits and punctuation, as GPUI reports
/// them on every platform.
const NAMED: &[&str] = &[
    "escape",
    "enter",
    "tab",
    "space",
    "backspace",
    "delete",
    "insert",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageup",
    "pagedown",
];

/// Function keys go up to this: `f1` to `f24`.
const MAX_FUNCTION_KEY: u32 = 24;

/// One keystroke an element listens for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    /// GPUI's name for the key: `escape`, `k`, `/`.
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
    /// ⌘ on macOS, Ctrl elsewhere.
    pub secondary: bool,
}

/// A keystroke as the window reports it.
#[derive(Clone, Copy, Debug)]
pub struct Pressed<'a> {
    pub key: &'a str,
    /// The text it types, if any: `?` for shift-/ on a US layout.
    pub text: Option<&'a str>,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// ⌘ on macOS, Super or the Windows key elsewhere.
    pub cmd: bool,
}

impl Chord {
    /// Reads a key name such as `ctrl-shift-p`; the problem says why not.
    pub fn parse(name: &str) -> Result<Self, String> {
        let chord = Self::parse_typed(name)?;
        if chord.types_text() {
            return Err(format!(
                "`{name}` types text, which keys never take; add ctrl, alt, cmd or \
                 secondary, or use ui.input for typing"
            ));
        }
        Ok(chord)
    }

    /// Reads a key name as [`Chord::parse`] does, also taking a key that
    /// types text (`j`, `shift-c`): a shell menu's single-key shortcut,
    /// which the host offers only while no text field or terminal has
    /// focus, so it never takes what the user types.
    pub fn parse_typed(name: &str) -> Result<Self, String> {
        // A trailing `-` is the minus key itself: `cmd--`, or `-`.
        let (modifiers, key) = match name.strip_suffix("--") {
            Some(modifiers) => (modifiers, "-"),
            None if name == "-" => ("", "-"),
            None => match name.rsplit_once('-') {
                Some((modifiers, key)) => (modifiers, key),
                None => ("", name),
            },
        };
        let mut chord = Self {
            key: key.to_owned(),
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
            secondary: false,
        };
        for modifier in modifiers.split('-').filter(|_| !modifiers.is_empty()) {
            let flag = match modifier {
                "ctrl" => &mut chord.ctrl,
                "alt" => &mut chord.alt,
                "shift" => &mut chord.shift,
                "cmd" => &mut chord.cmd,
                "secondary" => &mut chord.secondary,
                other => {
                    return Err(format!(
                        "unknown modifier `{other}`; use ctrl, alt, shift, cmd or secondary"
                    ));
                },
            };
            if *flag {
                return Err(format!("`{modifier}` twice"));
            }
            *flag = true;
        }
        check_key(key)?;
        Ok(chord)
    }

    /// Whether pressing it types text: a letter, digit, symbol or `space`
    /// without ctrl, alt, cmd or secondary.
    pub fn types_text(&self) -> bool {
        let typed = self.key == "space" || self.key.chars().count() == 1;
        typed && !(self.ctrl || self.alt || self.cmd || self.secondary)
    }

    /// Whether `pressed` is this chord, on macOS (`mac`) or elsewhere,
    /// where `secondary` means Ctrl rather than ⌘.
    pub fn matches(&self, pressed: &Pressed, mac: bool) -> bool {
        let ctrl = self.ctrl || (self.secondary && !mac);
        let cmd = self.cmd || (self.secondary && mac);
        if (ctrl, self.alt, cmd) != (pressed.ctrl, pressed.alt, pressed.cmd) {
            return false;
        }
        if self.shift == pressed.shift && self.key == pressed.key {
            return true;
        }
        // A symbol typed with shift, such as `?`, matches by its text.
        !self.shift && is_symbol(&self.key) && pressed.text == Some(self.key.as_str())
    }

    /// Plain Escape, with no modifiers.
    fn is_escape(&self) -> bool {
        *self == Self::plain("escape")
    }

    fn plain(key: &str) -> Self {
        Self {
            key: key.to_owned(),
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
            secondary: false,
        }
    }
}

/// Whether `key` is a key GPUI names; the problem says what is.
fn check_key(key: &str) -> Result<(), String> {
    let mut chars = key.chars();
    let single = match (chars.next(), chars.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    };
    let function = key
        .strip_prefix('f')
        .and_then(|number| number.parse::<u32>().ok())
        .is_some_and(|number| (1..=MAX_FUNCTION_KEY).contains(&number));
    match single {
        Some(letter) if letter.is_ascii_uppercase() => Err(format!(
            "write `shift-{}` for `{key}`; key names are lowercase",
            letter.to_ascii_lowercase()
        )),
        Some(only) if only.is_ascii_graphic() => Ok(()),
        _ if function || NAMED.contains(&key) => Ok(()),
        _ if key.is_empty() => Err("missing the key after the modifiers".to_owned()),
        _ => Err(format!(
            "unknown key `{key}`; use a-z, 0-9, a symbol such as `/`, f1 to f{MAX_FUNCTION_KEY}, or {}",
            NAMED.join(", ")
        )),
    }
}

/// A single printable character that is not a letter or digit.
fn is_symbol(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(only), None) if only.is_ascii_punctuation()
    )
}

/// What an element's `keys` send, by keystroke.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keys(pub Vec<(Chord, Data)>);

impl Keys {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What `pressed` sends to the mod: its own key's message, else for
    /// plain Escape the `dismiss` message when no key takes Escape.
    pub fn message<'a>(
        &'a self,
        dismiss: Option<&'a Data>,
        pressed: &Pressed,
        mac: bool,
    ) -> Option<&'a Data> {
        if let Some((_, msg)) = self.0.iter().find(|(chord, _)| chord.matches(pressed, mac)) {
            return Some(msg);
        }
        let escape = Chord::plain("escape");
        let takes_escape = self.0.iter().any(|(chord, _)| chord.is_escape());
        if !takes_escape && escape.matches(pressed, mac) {
            dismiss
        } else {
            None
        }
    }
}

impl Read for Keys {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let Value::Table(table) = value else {
            return Err(cx.fail("expected a table of key names to messages"));
        };
        let mut keys: Vec<(String, Chord, Data)> = Vec::new();
        for pair in table.pairs::<Value, Value>() {
            let (name, msg) = pair?;
            let Value::String(name) = name else {
                return Err(cx.fail("keys are key names such as `escape` or `cmd-k`"));
            };
            let name = name.to_string_lossy();
            let at = cx.quoted(&name);
            let chord = Chord::parse(&name).map_err(|problem| at.fail(problem))?;
            if let Some((other, ..)) = keys.iter().find(|(_, other, _)| *other == chord) {
                return Err(at.fail(format!("the same key as `{other}`")));
            }
            keys.push((name, chord, <Data as Read>::read(&msg, &at)?));
            if keys.len() > MAX_KEYS {
                return Err(cx.fail(format!("more than {MAX_KEYS} keys")));
            }
        }
        // Tables have no order; a stable one keeps views comparable.
        keys.sort_by(|(a, ..), (b, ..)| a.cmp(b));
        Ok(Self(
            keys.into_iter()
                .map(|(_, chord, msg)| (chord, msg))
                .collect(),
        ))
    }
}
