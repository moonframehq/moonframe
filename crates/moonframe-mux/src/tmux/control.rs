//! Reading a tmux control-mode client: `%output` lines for one pane become
//! the terminal's bytes, and the replies to our own commands (a `%begin` …
//! `%end` block each, flagged 1) seed a terminal that attaches to a session
//! already running with its scrollback, screen and cursor.

use std::{
    collections::VecDeque,
    io::BufRead,
    sync::{Arc, Mutex},
};

/// What a command's reply is for, in the order the commands were sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Expect {
    Nothing,
    /// `capture-pane`: the scrollback and screen, a line each.
    Capture,
    /// `display-message`: `cursor_x cursor_y keypad_cursor_flag alternate_on`.
    Cursor,
}

/// The pane's output, as an iterator of chunks for the session.
pub(super) struct Reader<R> {
    lines: R,
    pane: String,
    expected: Arc<Mutex<VecDeque<Expect>>>,
    /// The reply being read, if it is one of ours.
    block: Option<(Expect, Vec<Vec<u8>>)>,
    /// The copied scrollback and screen, until the cursor arrives.
    captured: Option<Vec<Vec<u8>>>,
    /// Output before the copy is already in it, so it waits for the seed.
    seeded: bool,
    done: bool,
}

impl<R: BufRead> Reader<R> {
    pub(super) fn new(lines: R, pane: String, expected: Arc<Mutex<VecDeque<Expect>>>) -> Self {
        Self {
            lines,
            pane,
            expected,
            block: None,
            captured: None,
            seeded: false,
            done: false,
        }
    }

    /// Handles one line; returns bytes for the emulator, if it carried any.
    fn line(&mut self, line: &[u8]) -> Option<Vec<u8>> {
        if let Some((expect, lines)) = &mut self.block {
            let failed = line.starts_with(b"%error ");
            if failed || line.starts_with(b"%end ") {
                let expect = *expect;
                let lines = self
                    .block
                    .take()
                    .map(|(_, lines)| lines)
                    .unwrap_or_default();
                // A failed command's lines are its error, not its answer.
                return self.reply(
                    expect,
                    if failed {
                        Vec::new()
                    } else {
                        lines
                    },
                );
            }
            lines.push(line.to_vec());
            return None;
        }
        if let Some(rest) = line.strip_prefix(b"%output ") {
            let (pane, data) = split_word(rest);
            return (self.seeded && pane == self.pane.as_bytes()).then(|| unescape(data));
        }
        if let Some(rest) = line.strip_prefix(b"%begin ") {
            // `%begin <time> <number> <flags>`: flag 1 marks our commands.
            let ours = rest.rsplit(|byte| *byte == b' ').next() == Some(b"1");
            if ours {
                let expect = self
                    .expected
                    .lock()
                    .ok()
                    .and_then(|mut expected| expected.pop_front())
                    .unwrap_or(Expect::Nothing);
                self.block = Some((expect, Vec::new()));
            } else {
                // Someone else's command: its lines are read and dropped.
                self.block = Some((Expect::Nothing, Vec::new()));
            }
            return None;
        }
        if line.starts_with(b"%exit") {
            self.done = true;
        }
        None
    }

    fn reply(&mut self, expect: Expect, lines: Vec<Vec<u8>>) -> Option<Vec<u8>> {
        match expect {
            Expect::Nothing => None,
            Expect::Capture => {
                self.captured = Some(lines);
                None
            },
            Expect::Cursor => {
                self.seeded = true;
                let captured = self.captured.take().unwrap_or_default();
                let facts = lines
                    .first()
                    .map(|line| String::from_utf8_lossy(line).into_owned());
                Some(seed(&captured, facts.as_deref().unwrap_or("")))
            },
        }
    }
}

impl<R: BufRead> Iterator for Reader<R> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Vec<u8>> {
        let mut line = Vec::new();
        while !self.done {
            line.clear();
            match self.lines.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => self.done = true,
                Ok(_) => {
                    let trimmed = line.strip_suffix(b"\n").unwrap_or(&line);
                    let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
                    if let Some(bytes) = self.line(trimmed)
                        && !bytes.is_empty()
                    {
                        return Some(bytes);
                    }
                },
            }
        }
        None
    }
}

/// What paints a fresh emulator like the pane: its scrollback and screen
/// line by line (the last lines fill the screen, the rest scroll into the
/// history), then its cursor and cursor-key mode.
fn seed(captured: &[Vec<u8>], facts: &str) -> Vec<u8> {
    let mut bytes = captured.join(&b"\r\n"[..]);
    let numbers: Vec<u32> = facts
        .split_whitespace()
        .filter_map(|word| word.parse().ok())
        .collect();
    if let [x, y, keypad, ..] = numbers[..] {
        bytes.extend_from_slice(format!("\x1b[0m\x1b[{};{}H", y + 1, x + 1).as_bytes());
        if keypad == 1 {
            bytes.extend_from_slice(b"\x1b[?1h");
        }
    }
    bytes
}

fn split_word(text: &[u8]) -> (&[u8], &[u8]) {
    match text.iter().position(|byte| *byte == b' ') {
        Some(at) => (&text[..at], &text[at + 1..]),
        None => (text, &[]),
    }
}

/// tmux writes bytes below space, and `\`, as a backslash and three octal
/// digits; everything else as it is.
pub(super) fn unescape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut at = 0;
    while at < data.len() {
        let byte = data[at];
        if byte == b'\\'
            && let Some(digits) = data.get(at + 1..at + 4)
            && digits.iter().all(|digit| (b'0'..=b'7').contains(digit))
        {
            let value = digits
                .iter()
                .fold(0u32, |value, digit| value * 8 + u32::from(digit - b'0'));
            out.push(u8::try_from(value).unwrap_or(b'?'));
            at += 4;
        } else {
            out.push(byte);
            at += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use {super::*, std::io::Cursor};

    fn read(input: &str, expected: &[Expect]) -> Vec<u8> {
        let expected = Arc::new(Mutex::new(expected.iter().copied().collect()));
        Reader::new(
            Cursor::new(input.as_bytes().to_vec()),
            "%2".into(),
            expected,
        )
        .flatten()
        .collect()
    }

    #[test]
    fn octal_escapes_become_bytes() {
        assert_eq!(unescape(br"a\033[1mb\015"), b"a\x1b[1mb\r");
        assert_eq!(unescape(br"\134"), b"\\");
        assert_eq!(unescape("❯ é".as_bytes()), "❯ é".as_bytes());
    }

    #[test]
    fn a_running_session_is_seeded_then_followed() {
        let input = "%begin 1 1 0\n%end 1 1 0\n\
            %output %2 before\n\
            %begin 1 2 1\n%end 1 2 1\n\
            %begin 1 3 1\nold\nprompt $\n%end 1 3 1\n\
            %begin 1 4 1\n9 1 1 0\n%end 1 4 1\n\
            %output %2 \\033[1mafter\n\
            %output %3 another pane\n\
            %exit\n%output %2 gone\n";
        let bytes = read(input, &[Expect::Nothing, Expect::Capture, Expect::Cursor]);
        assert_eq!(
            String::from_utf8_lossy(&bytes),
            "old\r\nprompt $\x1b[0m\x1b[2;10H\x1b[?1h\x1b[1mafter"
        );
    }

    #[test]
    fn replies_to_keys_are_not_output() {
        let input = "%begin 1 2 1\n%end 1 2 1\n%begin 1 3 1\n%end 1 3 1\n\
            %begin 1 4 1\n0 0 0 0\n%end 1 4 1\n\
            %begin 1 5 1\n%end 1 5 1\n%output %2 x\n";
        let bytes = read(input, &[
            Expect::Nothing,
            Expect::Capture,
            Expect::Cursor,
            Expect::Nothing,
        ]);
        assert_eq!(bytes, b"\x1b[0m\x1b[1;1Hx");
    }
}
