//! `app:edit`: changes the text of one of the sending mod's fields around
//! its selection, as a formatting toolbar or a completion does, so a mod
//! never has to track where the caret is. The field keeps its undo, and
//! sends its `change` as if the user had typed the edit.
//!
//! `{ field = "notes", wrap = { "**", "**" } }` puts the two around the
//! selection (or takes them off it again), `{ field, lines = "- " }`
//! starts each selected line with the prefix (or takes it off them all),
//! and `{ field, insert = "@Sam ", back = 3 }` replaces the selection and
//! the 3 bytes before it with the text.

use {
    super::APP,
    crate::{Data, Error},
    std::ops::Range,
};

/// Longest text an edit puts in, in bytes.
const MAX_TEXT: usize = 64 * 1024;

/// What `app:edit` asks: an edit of the field with this `id`.
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    /// The field's `id` among the sending mod's fields.
    pub field: String,
    pub change: EditChange,
    /// A completion is conditional on an empty selection at this byte
    /// offset and on the exact text immediately before it.
    pub expected: Option<(usize, String)>,
}

/// The edit itself.
#[derive(Clone, Debug, PartialEq)]
pub enum EditChange {
    /// Put `before` and `after` around the selection, which stays on the
    /// text inside; when they are already there, take them off.
    Wrap { before: String, after: String },
    /// Start each line the selection touches with `prefix`; when they all
    /// start with it, take it off.
    Lines { prefix: String },
    /// Replace the selection and the `back` bytes before it with `text`,
    /// and put the caret after it.
    Insert { text: String, back: usize },
}

/// A change to a text: replace `range` with `text`, then select `select`
/// (in the new text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Splice {
    pub range: Range<usize>,
    pub text: String,
    pub select: Range<usize>,
}

impl Edit {
    /// Whether a completion still refers to what the user was typing.
    /// A stale menu must never replace text at a newly moved caret.
    pub fn applies(&self, text: &str, selection: Range<usize>) -> bool {
        self.expected.as_ref().is_none_or(|(at, expected)| {
            selection.is_empty()
                && selection.start == *at
                && text
                    .get(..*at)
                    .is_some_and(|before| before.ends_with(expected))
        })
    }

    /// What the edit does to `text` whose selection is `selection`, in
    /// bytes. A selection off a character's edge moves back to it.
    pub fn splice(&self, text: &str, selection: Range<usize>) -> Splice {
        let edge = |at: usize| {
            let mut at = at.min(text.len());
            while !text.is_char_boundary(at) {
                at -= 1;
            }
            at
        };
        let (start, end) = (
            edge(selection.start.min(selection.end)),
            edge(selection.end),
        );
        match &self.change {
            EditChange::Wrap { before, after } => wrap(text, start..end, before, after),
            EditChange::Lines { prefix } => lines(text, start..end, prefix),
            EditChange::Insert { text: new, back } => {
                let from = edge(start.saturating_sub(*back));
                let caret = from + new.len();
                Splice {
                    range: from..end,
                    text: new.clone(),
                    select: caret..caret,
                }
            },
        }
    }
}

fn wrap(text: &str, selection: Range<usize>, before: &str, after: &str) -> Splice {
    let Range { start, end } = selection;
    let wrapped =
        start >= before.len() && text[..start].ends_with(before) && text[end..].starts_with(after);
    if wrapped {
        let outer = start - before.len()..end + after.len();
        let inner = text[start..end].to_owned();
        let at = outer.start;
        return Splice {
            range: outer,
            select: at..at + inner.len(),
            text: inner,
        };
    }
    let inner = &text[start..end];
    let at = start + before.len();
    Splice {
        range: selection,
        text: format!("{before}{inner}{after}"),
        select: at..at + inner.len(),
    }
}

fn lines(text: &str, selection: Range<usize>, prefix: &str) -> Splice {
    // From the start of the first line touched to the end of the last.
    let from = text[..selection.start].rfind('\n').map_or(0, |at| at + 1);
    // A nonempty selection is half-open: ending just after a newline
    // selects that newline, not the next line's contents.
    let end = if !selection.is_empty() && text[..selection.end].ends_with('\n') {
        selection.end - 1
    } else {
        selection.end
    };
    let to = text[end..].find('\n').map_or(text.len(), |at| end + at);
    let block = &text[from..to];
    let all = block.split('\n').all(|line| line.starts_with(prefix));
    let changed: Vec<String> = block
        .split('\n')
        .map(|line| match line.strip_prefix(prefix) {
            Some(rest) if all => rest.to_owned(),
            _ => format!("{prefix}{line}"),
        })
        .collect();
    let new = changed.join("\n");
    let end = from + new.len();
    Splice {
        range: from..to,
        text: new,
        select: if selection.is_empty() {
            end..end
        } else {
            from..end
        },
    }
}

/// `{ field = "...", wrap = { before, after } | lines = "..." | insert =
/// "...", back = n }`.
pub(super) fn edit(msg: &Data) -> Result<Edit, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}edit`: {problem}"));
    let usage = "takes { field = \"<input id>\", wrap = { \"**\", \"**\" } }, { field, lines = \
                 \"- \" } or { field, insert = \"text\", back = n }";
    let Data::Map(fields) = msg else {
        return Err(fail(usage));
    };
    if let Some(key) = fields.keys().find(|key| {
        !matches!(
            key.as_str(),
            "field" | "wrap" | "lines" | "insert" | "back" | "at" | "expect"
        )
    }) {
        return Err(fail(&format!("unknown key `{key}`; it {usage}")));
    }
    let text = |value: &Data, key: &str| match value {
        Data::String(text) if text.len() <= MAX_TEXT => Ok(text.clone()),
        Data::String(_) => Err(fail(&format!("`{key}` is longer than {MAX_TEXT} bytes"))),
        _ => Err(fail(&format!("`{key}` is a string"))),
    };
    let field = match fields.get("field") {
        Some(Data::String(field)) if !field.is_empty() => field.clone(),
        _ => {
            return Err(fail(&format!(
                "needs the `field`: the `id` of one of your inputs; it {usage}"
            )));
        },
    };
    let back = match fields.get("back") {
        None => None,
        Some(Data::Integer(back)) if *back >= 0 => usize::try_from(*back).ok(),
        Some(_) => return Err(fail("`back` is a count of bytes, 0 or more")),
    };
    let change = match (
        fields.get("wrap"),
        fields.get("lines"),
        fields.get("insert"),
    ) {
        (Some(Data::List(pair)), None, None) => match &pair[..] {
            [before, after] => EditChange::Wrap {
                before: text(before, "wrap")?,
                after: text(after, "wrap")?,
            },
            _ => return Err(fail("`wrap` is { before, after }")),
        },
        (Some(_), None, None) => return Err(fail("`wrap` is { before, after }")),
        (None, Some(prefix), None) => EditChange::Lines {
            prefix: text(prefix, "lines")?,
        },
        (None, None, Some(insert)) => EditChange::Insert {
            text: text(insert, "insert")?,
            back: back.unwrap_or(0),
        },
        _ => {
            return Err(fail(&format!(
                "give one of `wrap`, `lines` and `insert`; it {usage}"
            )));
        },
    };
    if back.is_some() && !matches!(change, EditChange::Insert { .. }) {
        return Err(fail("`back` goes with `insert`"));
    }
    let expected = match (fields.get("at"), fields.get("expect")) {
        (None, None) => None,
        (Some(Data::Integer(at)), Some(value)) if *at >= 0 => {
            let at = usize::try_from(*at).map_err(|_| fail("`at` is a byte offset"))?;
            let expected = text(value, "expect")?;
            match &change {
                EditChange::Insert { back, .. } if *back == expected.len() => Some((at, expected)),
                _ => {
                    return Err(fail(
                        "`at` and `expect` go with `insert`, with `back` equal to the expected text's byte length",
                    ));
                },
            }
        },
        _ => {
            return Err(fail(
                "give both `at` (a byte offset, 0 or more) and `expect` (the text to replace)",
            ));
        },
    };
    Ok(Edit {
        field,
        change,
        expected,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn apply(change: EditChange, text: &str, selection: Range<usize>) -> (String, String) {
        let splice = Edit {
            field: "notes".to_owned(),
            change,
            expected: None,
        }
        .splice(text, selection);
        let mut new = text.to_owned();
        new.replace_range(splice.range.clone(), &splice.text);
        let selected = new[splice.select.clone()].to_owned();
        (new, selected)
    }

    fn bold() -> EditChange {
        EditChange::Wrap {
            before: "**".to_owned(),
            after: "**".to_owned(),
        }
    }

    #[test]
    fn wrap_puts_marks_around_the_selection_and_takes_them_off_again() {
        let (on, selected) = apply(bold(), "make it bold now", 8..12);
        assert_eq!(
            (on.as_str(), selected.as_str()),
            ("make it **bold** now", "bold")
        );
        let (off, selected) = apply(bold(), &on, 10..14);
        assert_eq!(
            (off.as_str(), selected.as_str()),
            ("make it bold now", "bold")
        );
    }

    #[test]
    fn wrap_at_the_caret_leaves_it_between_the_marks() {
        let splice = Edit {
            field: "notes".to_owned(),
            change: bold(),
            expected: None,
        }
        .splice("ab", 1..1);
        assert_eq!(splice.text, "****");
        assert_eq!(splice.select, 3..3);
    }

    #[test]
    fn lines_prefix_each_touched_line_and_toggle_off() {
        let list = EditChange::Lines {
            prefix: "- ".to_owned(),
        };
        let (on, _) = apply(list.clone(), "intro\none\ntwo\nend", 8..11);
        assert_eq!(on, "intro\n- one\n- two\nend");
        let (off, _) = apply(list, &on, 9..15);
        assert_eq!(off, "intro\none\ntwo\nend");
    }

    #[test]
    fn lines_exclude_the_line_at_a_nonempty_selections_end() {
        let list = EditChange::Lines {
            prefix: "- ".to_owned(),
        };
        let (on, _) = apply(list.clone(), "one\ntwo", 0..4);
        assert_eq!(on, "- one\ntwo");
        let (off, _) = apply(list.clone(), "- one\ntwo", 0..6);
        assert_eq!(off, "one\ntwo");
        let (caret, _) = apply(list, "one\ntwo", 4..4);
        assert_eq!(caret, "one\n- two");
    }

    #[test]
    fn insert_replaces_what_was_typed_before_the_caret() {
        let splice = Edit {
            field: "comment".to_owned(),
            expected: None,
            change: EditChange::Insert {
                text: "@Sarah Chen ".to_owned(),
                back: 3,
            },
        }
        .splice("hi @sa", 6..6);
        assert_eq!(splice.range, 3..6);
        assert_eq!(splice.select, 15..15);
    }

    #[test]
    fn a_selection_inside_a_character_moves_to_its_edge() {
        let splice = Edit {
            field: "notes".to_owned(),
            expected: None,
            change: EditChange::Insert {
                text: "x".to_owned(),
                back: 1,
            },
        }
        .splice("é", 2..2);
        assert_eq!(splice.range, 0..2);
    }

    #[test]
    fn a_completion_rejects_a_moved_caret_selection_or_changed_word() {
        let edit = Edit {
            field: "comment".to_owned(),
            change: EditChange::Insert {
                text: "@Sarah Chen ".to_owned(),
                back: 3,
            },
            expected: Some((6, "@sa".to_owned())),
        };
        assert!(edit.applies("hi @sa", 6..6));
        assert!(!edit.applies("hi @sa", 5..5));
        assert!(!edit.applies("hi @sa", 3..6));
        assert!(!edit.applies("hi @jo", 6..6));
        assert!(!edit.applies("hi", 6..6));
    }

    #[test]
    fn requests_are_read_and_mistakes_named() {
        let read = |lua: Data| edit(&lua);
        let map = |pairs: &[(&str, Data)]| {
            Data::Map(
                pairs
                    .iter()
                    .map(|(key, value)| ((*key).to_owned(), value.clone()))
                    .collect(),
            )
        };
        let ok = read(map(&[
            ("field", Data::from("notes")),
            ("lines", Data::from("> ")),
        ]))
        .unwrap();
        assert_eq!(
            ok.change,
            EditChange::Lines {
                prefix: "> ".to_owned()
            }
        );
        let both = read(map(&[
            ("field", Data::from("notes")),
            ("lines", Data::from("> ")),
            ("insert", Data::from("x")),
        ]));
        assert!(both.unwrap_err().to_string().contains("one of"));
        let stray = read(map(&[
            ("field", Data::from("notes")),
            ("wrap", Data::List(vec![Data::from("*"), Data::from("*")])),
            ("back", Data::Integer(2)),
        ]));
        assert!(
            stray
                .unwrap_err()
                .to_string()
                .contains("`back` goes with `insert`")
        );
        assert!(read(map(&[("wrap", Data::List(vec![]))])).is_err());
    }
}
