//! A command's output, from the bytes it wrote to lines a view can colour.
//!
//! Each line is text with style marks in it: `\u{1}STYLE\u{2}` starts a
//! piece drawn in STYLE, which is `FG;FLAGS` (`red;b`, `#5f87ff;`, `;bi`,
//! or empty for the default). FG is a colour name (`red`, `bright-blue`)
//! or a hex colour; FLAGS are `b` bold, `d` dim, `i` italic, `u`
//! underline. A line whose first piece is not in the default style starts
//! with a mark, so every line stands alone. The output's own control
//! characters are dropped, so it can never forge a mark.
//!
//! What a terminal would do is followed where a log can: colours and
//! weights (SGR), tabs, a carriage return overwriting its line (progress
//! bars), and a backspace. Cursor moves, titles and links are dropped.

use std::collections::VecDeque;

/// Starts a style mark.
pub(crate) const MARK: char = '\u{1}';
/// Ends a style mark.
pub(crate) const END: char = '\u{2}';
/// Characters kept of one line; the rest is cut.
const MAX_LINE: usize = 4000;

/// The 16 colours by their SGR index, as their names.
const NAMES: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright-black",
    "bright-red",
    "bright-green",
    "bright-yellow",
    "bright-blue",
    "bright-magenta",
    "bright-cyan",
    "bright-white",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Style {
    fg: Option<String>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
}

impl Style {
    fn mark(&self) -> String {
        let mut mark = String::new();
        mark.push(MARK);
        if let Some(fg) = &self.fg {
            mark.push_str(fg);
        }
        mark.push(';');
        for (on, flag) in [
            (self.bold, 'b'),
            (self.dim, 'd'),
            (self.italic, 'i'),
            (self.underline, 'u'),
        ] {
            if on {
                mark.push(flag);
            }
        }
        mark.push(END);
        mark
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Escape {
    /// Plain text.
    None,
    /// After ESC.
    Start,
    /// In `ESC [`, gathering parameters.
    Csi,
    /// In `ESC ]`, until BEL or `ESC \`.
    Osc,
    /// In an OSC, after an ESC.
    OscEsc,
}

/// Lines of output, built as bytes arrive.
pub(crate) struct Output {
    lines: VecDeque<String>,
    max_lines: usize,
    /// Lines dropped from the front to stay under `max_lines`.
    pub(crate) dropped: usize,
    /// The line being written, as characters with their style.
    line: Vec<(char, Style)>,
    /// Where the next character goes in `line`: a carriage return moves it
    /// back to the start.
    at: usize,
    style: Style,
    escape: Escape,
    params: String,
    /// Bytes of a character cut between two reads.
    partial: Vec<u8>,
}

impl Output {
    pub(crate) fn new(max_lines: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            max_lines: max_lines.max(1),
            dropped: 0,
            line: Vec::new(),
            at: 0,
            style: Style::default(),
            escape: Escape::None,
            params: String::new(),
            partial: Vec::new(),
        }
    }

    /// Takes what the command wrote next.
    pub(crate) fn feed(&mut self, bytes: &[u8]) {
        let mut buffer = std::mem::take(&mut self.partial);
        buffer.extend_from_slice(bytes);
        let mut rest = &buffer[..];
        loop {
            match std::str::from_utf8(rest) {
                Ok(text) => {
                    self.text(text);
                    break;
                },
                Err(error) => {
                    let (good, bad) = rest.split_at(error.valid_up_to());
                    // Checked by `valid_up_to`.
                    self.text(std::str::from_utf8(good).unwrap_or_default());
                    match error.error_len() {
                        Some(len) => {
                            self.put('\u{fffd}');
                            rest = &bad[len..];
                        },
                        None => {
                            self.partial = bad.to_vec();
                            break;
                        },
                    }
                },
            }
        }
    }

    fn text(&mut self, text: &str) {
        for character in text.chars() {
            self.char(character);
        }
    }

    fn char(&mut self, character: char) {
        match self.escape {
            Escape::Start => {
                self.escape = match character {
                    '[' => {
                        self.params.clear();
                        Escape::Csi
                    },
                    ']' => Escape::Osc,
                    _ => Escape::None,
                };
            },
            Escape::Csi => {
                if ('\u{40}'..='\u{7e}').contains(&character) {
                    self.escape = Escape::None;
                    if character == 'm' {
                        let params = std::mem::take(&mut self.params);
                        self.sgr(&params);
                    }
                } else if self.params.len() < 64 {
                    self.params.push(character);
                }
            },
            Escape::Osc => match character {
                '\u{7}' => self.escape = Escape::None,
                '\u{1b}' => self.escape = Escape::OscEsc,
                _ => {},
            },
            Escape::OscEsc => {
                self.escape = if character == '\\' {
                    Escape::None
                } else {
                    Escape::Osc
                };
            },
            Escape::None => match character {
                '\u{1b}' => self.escape = Escape::Start,
                '\n' => self.newline(),
                '\r' => self.at = 0,
                '\t' => {
                    // Never past the cap, where `put` stops moving.
                    let next = ((self.at / 8 + 1) * 8).min(MAX_LINE);
                    while self.at < next {
                        self.put(' ');
                    }
                },
                '\u{8}' => self.at = self.at.saturating_sub(1),
                _ if character.is_control() => {},
                _ => self.put(character),
            },
        }
    }

    fn put(&mut self, character: char) {
        if self.at >= MAX_LINE {
            return;
        }
        let cell = (character, self.style.clone());
        if self.at < self.line.len() {
            self.line[self.at] = cell;
        } else {
            self.line.push(cell);
        }
        self.at += 1;
    }

    fn newline(&mut self) {
        let mut text = String::new();
        let mut current = Style::default();
        for (character, style) in self.line.drain(..) {
            if style != current {
                text.push_str(&style.mark());
                current = style;
            }
            text.push(character);
        }
        self.at = 0;
        self.lines.push_back(text);
        while self.lines.len() > self.max_lines {
            self.lines.pop_front();
            self.dropped += 1;
        }
    }

    fn sgr(&mut self, params: &str) {
        let mut codes = params
            .split([';', ':'])
            .map(|code| code.parse::<u16>().unwrap_or(0));
        // `ESC [ m` is a reset, as `ESC [ 0 m`.
        let mut any = false;
        while let Some(code) = codes.next() {
            any = true;
            match code {
                0 => self.style = Style::default(),
                1 => self.style.bold = true,
                2 => self.style.dim = true,
                3 => self.style.italic = true,
                4 => self.style.underline = true,
                22 => {
                    self.style.bold = false;
                    self.style.dim = false;
                },
                23 => self.style.italic = false,
                24 => self.style.underline = false,
                30..=37 => self.style.fg = Some(NAMES[usize::from(code - 30)].to_owned()),
                90..=97 => self.style.fg = Some(NAMES[usize::from(code - 90 + 8)].to_owned()),
                39 => self.style.fg = None,
                38 => self.style.fg = extended(&mut codes),
                // A background's own colour, read and left out.
                48 => {
                    extended(&mut codes);
                },
                _ => {},
            }
        }
        if !any {
            self.style = Style::default();
        }
    }

    /// The finished lines, then the one being written if it has anything.
    pub(crate) fn lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self.lines.iter().cloned().collect();
        if !self.line.is_empty() {
            let mut copy = Self::new(1);
            copy.line = self.line.clone();
            copy.newline();
            lines.extend(copy.lines);
        }
        lines
    }

    /// How many lines there are, the one being written included.
    pub(crate) fn len(&self) -> usize {
        self.lines.len() + usize::from(!self.line.is_empty())
    }
}

/// The colour after `38` or `48`: `5;N` from the 256, or `2;R;G;B`.
fn extended(codes: &mut impl Iterator<Item = u16>) -> Option<String> {
    match codes.next()? {
        5 => {
            let index = codes.next()?;
            Some(indexed(index))
        },
        2 => {
            let red = codes.next()?.min(255);
            let green = codes.next()?.min(255);
            let blue = codes.next()?.min(255);
            Some(format!("#{red:02x}{green:02x}{blue:02x}"))
        },
        _ => None,
    }
}

/// One of the 256 colours: the 16 by name, then the cube and the greys.
fn indexed(index: u16) -> String {
    match index {
        0..=15 => NAMES[usize::from(index)].to_owned(),
        16..=231 => {
            let index = index - 16;
            let level = |value: u16| {
                if value == 0 {
                    0
                } else {
                    value * 40 + 55
                }
            };
            format!(
                "#{:02x}{:02x}{:02x}",
                level(index / 36),
                level(index / 6 % 6),
                level(index % 6)
            )
        },
        _ => {
            let grey = (index.min(255) - 232) * 10 + 8;
            format!("#{grey:02x}{grey:02x}{grey:02x}")
        },
    }
}

/// `text` without its style marks.
pub(crate) fn plain(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut in_mark = false;
    for character in text.chars() {
        match character {
            MARK => in_mark = true,
            END => in_mark = false,
            _ if !in_mark => plain.push(character),
            _ => {},
        }
    }
    plain
}
