//! [`Data`] as Luau, for people and agents to read: cut to a budget, so
//! a value holding a whole document still reads as a few lines.

use {crate::Data, std::fmt::Write as _};

/// How deep a value is written, how many entries of one table, and how
/// much of one string.
const DEPTH: usize = 6;
const ENTRIES: usize = 40;
const STRING: usize = 160;
/// Luau's reserved words, which a table key must quote.
const KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local",
    "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

impl Data {
    /// The value as Luau laid out on lines, at most about `budget` bytes:
    /// what does not fit (deep tables, long lists, long strings, the rest
    /// past the budget) is cut with a comment saying so.
    pub fn to_luau(&self, budget: usize) -> String {
        Writer::write(self, budget, false)
    }

    /// The same on one line, for a message or one field.
    pub fn to_luau_line(&self, budget: usize) -> String {
        Writer::write(self, budget, true)
    }
}

struct Writer {
    out: String,
    budget: usize,
    cut: bool,
    /// One line: entries are parted by spaces, and comments are `--[[ ]]`.
    inline: bool,
}

impl Writer {
    fn write(data: &Data, budget: usize, inline: bool) -> String {
        let mut writer = Self {
            out: String::new(),
            budget,
            cut: false,
            inline,
        };
        writer.value(data, 0);
        if writer.cut {
            writer.out.push_str(if inline {
                " --[[ cut to fit ]]"
            } else {
                " -- cut to fit"
            });
        }
        writer.out
    }

    /// Where the next entry of a table at `depth` starts.
    fn break_line(&mut self, depth: usize) {
        if self.inline {
            self.out.push(' ');
        } else {
            self.out.push('\n');
            self.out.push_str(&"  ".repeat(depth));
        }
    }

    fn full(&mut self) -> bool {
        if self.out.len() >= self.budget {
            self.cut = true;
        }
        self.cut
    }

    fn value(&mut self, data: &Data, depth: usize) {
        match data {
            Data::Nil => self.out.push_str("nil"),
            Data::Bool(value) => {
                let _ = write!(self.out, "{value}");
            },
            Data::Integer(value) => {
                let _ = write!(self.out, "{value}");
            },
            Data::Number(value) => self.number(*value),
            Data::String(text) => self.string(text),
            Data::List(items) if items.is_empty() => self.out.push_str("{}"),
            Data::Map(entries) if entries.is_empty() => self.out.push_str("{}"),
            Data::List(items) => {
                let entries = items.iter().map(|item| (None, item));
                self.table(entries, items.len(), depth);
            },
            Data::Map(entries) => {
                let named = entries
                    .iter()
                    .map(|(key, value)| (Some(key.as_str()), value));
                self.table(named, entries.len(), depth);
            },
        }
    }

    fn table<'a>(
        &mut self,
        entries: impl Iterator<Item = (Option<&'a str>, &'a Data)>,
        len: usize,
        depth: usize,
    ) {
        if depth >= DEPTH {
            let _ = write!(self.out, "{{ --[[ {len} entries ]] }}");
            return;
        }
        self.out.push('{');
        for (index, (key, value)) in entries.enumerate() {
            if index >= ENTRIES || self.full() {
                self.break_line(depth + 1);
                let more = len - index;
                let _ = if self.inline {
                    write!(self.out, "--[[ {more} more ]]")
                } else {
                    write!(self.out, "-- {more} more")
                };
                break;
            }
            self.break_line(depth + 1);
            if let Some(key) = key {
                self.key(key);
                self.out.push_str(" = ");
            }
            self.value(value, depth + 1);
            self.out.push(',');
        }
        self.break_line(depth);
        self.out.push('}');
    }

    /// A number as Luau reads it back: what has no literal as the
    /// expression that makes it, and the very large or small in exponent
    /// form rather than hundreds of digits.
    fn number(&mut self, value: f64) {
        let _ = if value.is_nan() {
            write!(self.out, "0/0")
        } else if value.is_infinite() {
            let sign = if value < 0. {
                "-"
            } else {
                ""
            };
            write!(self.out, "{sign}math.huge")
        } else if value != 0. && !(1e-5..1e15).contains(&value.abs()) {
            write!(self.out, "{value:e}")
        } else {
            write!(self.out, "{value}")
        };
    }

    /// A table key: bare when Luau reads it as a name, else quoted (and,
    /// like any string, cut to [`STRING`] characters).
    fn key(&mut self, key: &str) {
        let bare = key.len() <= STRING
            && !KEYWORDS.contains(&key)
            && key
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if bare {
            self.out.push_str(key);
        } else {
            self.out.push('[');
            self.string(key);
            self.out.push(']');
        }
    }

    /// A Luau string literal, cut to [`STRING`] characters.
    fn string(&mut self, text: &str) {
        let count = text.chars().count();
        self.out.push('"');
        for c in text.chars().take(STRING) {
            match c {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                c if c.is_control() => {
                    let _ = write!(self.out, "\\u{{{:x}}}", u32::from(c));
                },
                c => self.out.push(c),
            }
        }
        self.out.push('"');
        if count > STRING {
            let _ = write!(self.out, " --[[ {} more chars ]]", count - STRING);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "luau/tests.rs"]
mod tests;
