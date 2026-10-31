//! What a key sends to the shell: the xterm sequences most programs expect.
//! Shared by both emulators, so a key means the same in either.

/// A key press as the window reports it: the key's name (`"enter"`, `"up"`,
/// `"a"`) and the text it types, if any.
#[derive(Clone, Copy, Debug, Default)]
pub struct Key<'a> {
    pub name: &'a str,
    pub text: Option<&'a str>,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// The bytes for `key`, or `None` for a key the terminal leaves to the app
/// (such as one with ⌘). `app_cursor` is the DECCKM mode: arrows then send
/// `ESC O` instead of `ESC [`.
pub fn encode(key: Key, app_cursor: bool) -> Option<Vec<u8>> {
    let arrow = |letter: u8| {
        let prefix: &[u8] = if app_cursor {
            b"\x1bO"
        } else {
            b"\x1b["
        };
        [prefix, &[letter]].concat()
    };
    let named: Option<Vec<u8>> = match key.name {
        "enter" => Some(b"\r".to_vec()),
        "tab" if key.shift => Some(b"\x1b[Z".to_vec()),
        "tab" => Some(b"\t".to_vec()),
        "backspace" if key.alt => Some(b"\x1b\x7f".to_vec()),
        "backspace" => Some(b"\x7f".to_vec()),
        "escape" => Some(b"\x1b".to_vec()),
        "up" => Some(arrow(b'A')),
        "down" => Some(arrow(b'B')),
        "right" => Some(arrow(b'C')),
        "left" => Some(arrow(b'D')),
        "home" => Some(arrow(b'H')),
        "end" => Some(arrow(b'F')),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        "delete" => Some(b"\x1b[3~".to_vec()),
        "insert" => Some(b"\x1b[2~".to_vec()),
        "f1" => Some(b"\x1bOP".to_vec()),
        "f2" => Some(b"\x1bOQ".to_vec()),
        "f3" => Some(b"\x1bOR".to_vec()),
        "f4" => Some(b"\x1bOS".to_vec()),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    if key.ctrl {
        return control(key.name).map(|byte| alt(key.alt, vec![byte]));
    }
    let text = match key.text {
        Some(text) if !text.is_empty() => text,
        _ if key.name == "space" => " ",
        _ => return None,
    };
    Some(alt(key.alt, text.as_bytes().to_vec()))
}

/// Text pasted into the terminal, wrapped when the shell asked for
/// bracketed paste so it is not run as typed. Control characters other than
/// tab and newlines are dropped: with ESC gone no escape sequence can be
/// rebuilt from the pieces, such as one that ends the bracket early.
pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let text: String = text
        .chars()
        .filter(|char| matches!(char, '\t' | '\n' | '\r') || !char.is_control())
        .collect();
    if bracketed {
        [b"\x1b[200~", text.as_bytes(), b"\x1b[201~"].concat()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

/// Ctrl with a letter or one of `@[\]^_ ` sends its control byte.
fn control(name: &str) -> Option<u8> {
    let [byte] = name.as_bytes() else {
        return (name == "space").then_some(0);
    };
    match byte.to_ascii_lowercase() {
        letter @ b'a'..=b'z' => Some(letter - b'a' + 1),
        b'@' | b'2' => Some(0),
        b'[' => Some(0x1b),
        b'\\' => Some(0x1c),
        b']' => Some(0x1d),
        b'^' | b'6' => Some(0x1e),
        b'_' | b'-' => Some(0x1f),
        _ => None,
    }
}

/// Alt (Option) sends ESC first, as xterm's "meta sends escape".
fn alt(alt: bool, bytes: Vec<u8>) -> Vec<u8> {
    if alt {
        [b"\x1b".as_slice(), &bytes].concat()
    } else {
        bytes
    }
}
