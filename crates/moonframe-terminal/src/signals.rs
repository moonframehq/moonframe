//! What a shell says about itself between the bytes it paints: a command
//! starting and ending (OSC 133), its folder (OSC 7), its title (OSC 0 and
//! 2), and notices for the user (OSC 9 and 777).
//!
//! The session reads them from the raw output, ahead of the emulator, so
//! both engines and every backend report the same. Any program can print
//! these sequences, `cat` of a file included: a [`Signal`] is what the
//! output claims, to show or to tell someone about, never a reason to act.

use std::time::{Duration, Instant};

/// Longest sequence read; a longer one is skipped whole. It holds a
/// percent-encoded command line of [`MAX_TEXT`] characters.
const MAX_SEQUENCE: usize = 8 * 1024;
/// Longest text passed on, in characters.
const MAX_TEXT: usize = 1024;

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
/// CAN and SUB end a sequence without completing it.
const CANCEL: [u8; 2] = [0x18, 0x1a];

/// One thing a shell or a program in it reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    /// The shell shows its prompt, ready for a command.
    Prompt,
    /// A command started; shells that say which give its line.
    Started {
        command: Option<String>,
    },
    /// The command that [`Signal::Started`] announced ended.
    Finished {
        command: Option<String>,
        /// `None` when the shell went back to its prompt without saying.
        status: Option<i32>,
        took: Duration,
    },
    /// The shell's folder. `host` names the machine it is on, which is
    /// another one in an `ssh` session.
    Folder {
        path: String,
        host: Option<String>,
    },
    Title(String),
    /// A program asks to tell the user something, as agents do when they
    /// finish or wait for an answer.
    Notice {
        title: Option<String>,
        body: String,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
    #[default]
    Text,
    /// After an ESC in the text.
    Escape,
    /// Inside an operating system command.
    Sequence,
    /// After an ESC inside one: a terminator, or the end of it.
    SequenceEscape,
}

struct Running {
    command: Option<String>,
    since: Instant,
}

/// Finds signals in a stream of output, however it is cut into chunks.
#[derive(Default)]
pub(crate) struct Scanner {
    state: State,
    sequence: Vec<u8>,
    /// The sequence outgrew [`MAX_SEQUENCE`]; it is read to its end only.
    skipping: bool,
    running: Option<Running>,
    /// The last mark was the prompt, so a redraw of it says nothing new.
    at_prompt: bool,
}

impl Scanner {
    pub(crate) fn feed(&mut self, bytes: &[u8], signals: &mut Vec<Signal>) {
        let mut rest = bytes;
        while let Some((&byte, after)) = rest.split_first() {
            match self.state {
                State::Text => {
                    // Nearly all output: go straight to the next escape.
                    match rest.iter().position(|byte| *byte == ESC) {
                        Some(at) => {
                            self.state = State::Escape;
                            rest = &rest[at + 1..];
                        },
                        None => return,
                    }
                    continue;
                },
                State::Escape => self.after_escape(byte),
                State::Sequence => match byte {
                    BEL => self.finish(signals),
                    ESC => self.state = State::SequenceEscape,
                    byte if CANCEL.contains(&byte) => self.state = State::Text,
                    byte if self.sequence.len() < MAX_SEQUENCE => self.sequence.push(byte),
                    _ => self.skipping = true,
                },
                State::SequenceEscape if byte == b'\\' => self.finish(signals),
                // An ESC starting something else ended it unfinished.
                State::SequenceEscape => self.after_escape(byte),
            }
            rest = after;
        }
    }

    fn after_escape(&mut self, byte: u8) {
        self.state = match byte {
            b']' => {
                self.sequence.clear();
                self.skipping = false;
                State::Sequence
            },
            ESC => State::Escape,
            _ => State::Text,
        };
    }

    fn finish(&mut self, signals: &mut Vec<Signal>) {
        self.state = State::Text;
        if self.skipping {
            return;
        }
        let sequence = String::from_utf8_lossy(&self.sequence).into_owned();
        let (code, rest) = sequence.split_once(';').unwrap_or((&sequence, ""));
        match code {
            "0" | "2" => signals.push(Signal::Title(clean(rest))),
            "7" => signals.extend(folder(rest)),
            "9" => signals.extend(notice(rest)),
            "133" => self.mark(rest, signals),
            "777" => signals.extend(desktop_notice(rest)),
            _ => {},
        }
    }

    /// One OSC 133 mark: `A` prompt, `C` output starts, `D;<status>` it
    /// ended. A shell's own marks and ours can both arrive, and a prompt
    /// is redrawn as the window resizes, so only a change is reported.
    fn mark(&mut self, mark: &str, signals: &mut Vec<Signal>) {
        let mut fields = mark.split(';');
        match fields.next() {
            Some("A") if !self.at_prompt => {
                // Back at the prompt without an end mark.
                self.end(None, signals);
                self.at_prompt = true;
                signals.push(Signal::Prompt);
            },
            Some("C") => {
                let command = fields
                    .find_map(|field| field.strip_prefix("cmdline_url="))
                    .map(|line| clean(&decode(line)));
                self.at_prompt = false;
                if self
                    .running
                    .as_ref()
                    .is_none_or(|run| run.command != command)
                {
                    signals.push(Signal::Started {
                        command: command.clone(),
                    });
                    self.running = Some(Running {
                        command,
                        since: Instant::now(),
                    });
                }
            },
            Some("D") => {
                let status = fields.next().and_then(|status| status.parse().ok());
                self.end(status, signals);
            },
            _ => {},
        }
    }

    /// Only a start that was seen has an end: an empty line at the prompt
    /// ends nothing.
    fn end(&mut self, status: Option<i32>, signals: &mut Vec<Signal>) {
        if let Some(run) = self.running.take() {
            signals.push(Signal::Finished {
                command: run.command,
                status,
                took: run.since.elapsed(),
            });
        }
    }
}

/// `file://host/path`, percent-encoded.
fn folder(url: &str) -> Option<Signal> {
    let rest = url
        .strip_prefix("file://")
        .or_else(|| url.strip_prefix("kitty-shell-cwd://"))?;
    let (host, path) = rest.split_at(rest.find('/')?);
    Some(Signal::Folder {
        path: clean(&decode(path)),
        host: (!host.is_empty()).then(|| clean(host)),
    })
}

/// iTerm2's `9;<body>`. ConEmu's `9;<number>;...` means other things
/// (progress, mostly), and is not a notice.
fn notice(body: &str) -> Option<Signal> {
    let first = body.split(';').next().unwrap_or(body);
    let numbered = !first.is_empty() && first.bytes().all(|byte| byte.is_ascii_digit());
    (!numbered && !body.is_empty()).then(|| Signal::Notice {
        title: None,
        body: clean(body),
    })
}

/// rxvt's `777;notify;<title>;<body>`.
fn desktop_notice(sequence: &str) -> Option<Signal> {
    let mut fields = sequence.splitn(3, ';');
    (fields.next()? == "notify").then_some(())?;
    let title = fields.next()?;
    Some(Signal::Notice {
        title: (!title.is_empty()).then(|| clean(title)),
        body: clean(fields.next().unwrap_or_default()),
    })
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let escaped = (byte == b'%')
            .then(|| bytes.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        decoded.push(escaped.unwrap_or(byte));
        at += if escaped.is_some() {
            3
        } else {
            1
        };
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// What may be shown: no control characters, and no more than [`MAX_TEXT`].
fn clean(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .take(MAX_TEXT)
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "signals/tests.rs"]
mod tests;
