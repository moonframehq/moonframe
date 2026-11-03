//! Text fields, and the key chords they take over.

use super::*;

/// A text field the host edits: single-line, or several lines with
/// `multiline`. The host keeps what the user typed by the drawing mod and
/// `id` (or the field's place in the tree), so it survives re-renders and
/// reloads; `value` replaces it only when the mod passes a new one.
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub look: Look,
    /// Keeps the text across renders; without one, the field's path.
    pub id: Option<String>,
    /// The text to show, applied when it differs from the last value the
    /// mod passed, so a mod can set or clear it; otherwise the user's text
    /// stands.
    pub value: Option<String>,
    pub placeholder: Option<String>,
    /// The mod the messages go to; the drawing mod when unset.
    pub to: Option<String>,
    /// Sent on each edit, shaped by [`Input::message`].
    pub change: Option<Data>,
    /// Sent when the selection/caret moves, with the text and caret.
    pub select: Option<Data>,
    /// Sent on Enter (single-line), or ⌘/Ctrl-Enter (multiline).
    pub submit: Option<Data>,
    /// Sent with the text when the field loses keyboard focus: a title
    /// edited in place is saved as the user clicks away.
    pub blur: Option<Data>,
    /// Keys that send a message instead of editing while the field has focus.
    pub keys: Vec<(KeyChord, Data)>,
    pub multiline: bool,
    /// Visible lines of a multiline field; it scrolls past them.
    pub rows: u32,
    /// A multiline field grows from `rows` with its lines up to this many,
    /// then scrolls: a comment box, a title that wraps.
    pub max_rows: Option<u32>,
    pub disabled: bool,
    /// Take keyboard focus when it appears, or when this turns true.
    pub focus: bool,
    /// Use the monospace font.
    pub mono: bool,
    /// The `ui.browser` this field is the address bar of: Enter loads the
    /// typed address there (see [`typed_address`]), focus selects the
    /// text, and Escape puts back the page's address.
    pub browser: Option<String>,
    pub width: Option<f32>,
    pub grow: bool,
    /// A writing field: markdown is styled as it is typed (headings bold,
    /// emphasis slanted, markup faded) and lines wrap at the field's width.
    pub prose: bool,
    /// Focus on the sentence or paragraph being written: the rest fades.
    pub dim: Option<Dim>,
    /// Keep the line being written in the middle of the field, as a
    /// typewriter keeps its line, scrolling the text instead of the caret.
    pub typewriter: bool,
}

/// What a writing field keeps in full while the rest of its text fades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dim {
    /// The sentence the caret is in.
    Sentence,
    /// The paragraph the caret is in: lines up to a blank one.
    Paragraph,
}

impl Input {
    /// Lines a multiline field shows without `rows`.
    pub const DEFAULT_ROWS: u32 = 3;
    /// Most `rows` a field can ask for.
    pub const MAX_ROWS: u32 = 200;

    /// The lines a multiline field shows holding `text`: `rows`, or with
    /// `max_rows` as many as its lines up to that.
    pub fn visible_rows(&self, text: &str) -> u32 {
        let Some(most) = self.max_rows else {
            return self.rows;
        };
        let lines = u32::try_from(text.split('\n').count()).unwrap_or(u32::MAX);
        lines.clamp(self.rows, most.max(self.rows))
    }

    /// What the field sends for `msg` when its text is `value` with the
    /// caret `caret` bytes in: the message [`Input::message`] makes, with
    /// `caret` added, so a mod can tell what was typed just before it (an
    /// `@name` to complete).
    pub fn message_at(msg: &Data, value: &str, caret: usize) -> Data {
        let caret = Data::Integer(i64::try_from(caret).unwrap_or(i64::MAX));
        match Self::message(msg, value) {
            Data::Map(mut entries) => {
                entries.insert("caret".to_owned(), caret);
                Data::Map(entries)
            },
            other => other,
        }
    }

    /// Whether the host edits it as prose rather than plain text: any of
    /// `prose`, `dim` or `typewriter`.
    pub fn writing(&self) -> bool {
        self.prose || self.dim.is_some() || self.typewriter
    }

    /// What the field sends for `msg` when its text is `value`: a table
    /// `msg` is copied with `value` set; anything else becomes
    /// `{ msg = msg, value = value }`.
    pub fn message(msg: &Data, value: &str) -> Data {
        let value = Data::String(value.to_owned());
        match msg {
            Data::Map(entries) => {
                let mut entries = entries.clone();
                entries.insert("value".to_owned(), value);
                Data::Map(entries)
            },
            other => Data::Map(
                [
                    ("msg".to_owned(), other.clone()),
                    ("value".to_owned(), value),
                ]
                .into(),
            ),
        }
    }
}

/// A key and the modifiers held with it, named as GPUI names keystrokes:
/// `up`, `escape`, `shift-tab`, `ctrl-n`, `secondary-enter`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyChord {
    /// GPUI's name for the key: a lowercase letter, digit or punctuation
    /// mark, or one of [`KEY_NAMES`].
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// ⌘ on macOS, the Windows or Super key elsewhere.
    pub cmd: bool,
    /// ⌘ on macOS, Ctrl elsewhere: the one to use for shortcuts that work
    /// on every platform.
    pub secondary: bool,
}

/// The modifiers a [`KeyChord`] can name.
pub const KEY_MODIFIERS: &[&str] = &["ctrl", "alt", "shift", "cmd", "secondary"];

/// Keys with a name rather than a character.
pub const KEY_NAMES: &[&str] = &[
    "up",
    "down",
    "left",
    "right",
    "escape",
    "tab",
    "enter",
    "backspace",
    "delete",
    "insert",
    "home",
    "end",
    "pageup",
    "pagedown",
    "space",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

impl KeyChord {
    /// Reads `ctrl-shift-n`: modifiers, each at most once, then the key.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut chord = Self {
            key: String::new(),
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
            secondary: false,
        };
        // A trailing `-` after a separator is the minus key (`ctrl--`).
        let (modifiers, key) = match text.strip_suffix("--") {
            Some(rest) => (Some(rest), "-"),
            None => match text.rsplit_once('-') {
                Some((modifiers, key)) if !modifiers.is_empty() => (Some(modifiers), key),
                _ => (None, text),
            },
        };
        for modifier in modifiers
            .into_iter()
            .flat_map(|modifiers| modifiers.split('-'))
        {
            let held = match modifier {
                "ctrl" => &mut chord.ctrl,
                "alt" => &mut chord.alt,
                "shift" => &mut chord.shift,
                "cmd" => &mut chord.cmd,
                "secondary" => &mut chord.secondary,
                _ => {
                    return Err(format!(
                        "unknown modifier `{modifier}`; use {}",
                        KEY_MODIFIERS.join(", ")
                    ));
                },
            };
            if *held {
                return Err(format!("`{modifier}` twice"));
            }
            *held = true;
        }
        let mut chars = key.chars();
        let character = match (chars.next(), chars.next()) {
            (Some(c), None) => c.is_ascii_graphic() && !c.is_ascii_uppercase(),
            _ => false,
        };
        if !character && !KEY_NAMES.contains(&key) {
            return Err(format!(
                "unknown key `{key}`; use a lowercase letter, digit or mark, or one of {}",
                KEY_NAMES.join(", ")
            ));
        }
        chord.key = key.to_owned();
        Ok(chord)
    }
}
