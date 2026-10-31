//! Reading Lua values into Rust types. A type that comes from a script
//! implements [`Read`]; a table of named keys goes through [`Fields`], which
//! refuses keys its reader does not take. Every reader carries a [`Cx`] that
//! knows where it is, so a problem names its place:
//! `column[2].button.style.background: unknown color `chartreuse``.

mod fields;

pub(crate) use fields::Fields;

use {
    crate::{Error, style::Names},
    mlua::{Function, Table, Value},
    std::{fmt::Display, ops::RangeInclusive},
};

/// Longer strings are refused; no label or color needs more.
const MAX_TEXT: usize = 16 * 1024;

/// No stylesheet names, for values that cannot use any.
pub(crate) static NO_NAMES: Names = Names::NONE;

/// Where a reader is, and the stylesheet names it reads against.
#[derive(Clone, Debug)]
pub(crate) struct Cx<'p> {
    /// A path such as `panel[2].item.style`; empty at the root.
    at: String,
    names: &'p Names,
    /// List levels below the root, to bound how deep a tree goes.
    depth: usize,
}

impl<'p> Cx<'p> {
    pub(crate) fn root(names: &'p Names) -> Self {
        Self {
            at: String::new(),
            names,
            depth: 0,
        }
    }

    pub(crate) fn names(&self) -> &'p Names {
        self.names
    }

    pub(crate) fn depth(&self) -> usize {
        self.depth
    }

    /// A named key: `style.background`.
    pub(crate) fn key(&self, key: &str) -> Self {
        let at = if self.at.is_empty() {
            key.to_owned()
        } else {
            format!("{}.{key}", self.at)
        };
        Self { at, ..self.clone() }
    }

    /// A list position, counted from 1 as Lua does: `column[2]`.
    pub(crate) fn index(&self, index: usize) -> Self {
        Self {
            at: format!("{}[{index}]", self.at),
            depth: self.depth + 1,
            ..self.clone()
        }
    }

    /// A key that is not a name, such as a selector: `rules[".card"]`.
    pub(crate) fn quoted(&self, key: &str) -> Self {
        Self {
            at: format!("{}[\"{key}\"]", self.at),
            ..self.clone()
        }
    }

    pub(crate) fn fail(&self, problem: impl Display) -> Error {
        Error::at(&self.at, problem)
    }
}

/// A type a script can produce.
pub(crate) trait Read: Sized {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error>;
}

/// `nil` is `None`; anything else must read as `T`.
impl<T: Read> Read for Option<T> {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Nil => Ok(None),
            value => T::read(value, cx).map(Some),
        }
    }
}

/// A list, skipping `nil` and `false` so a script can write
/// `cond and ui.text "..."`.
impl<T: Read> Read for Vec<T> {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let Value::Table(table) = value else {
            return Err(cx.fail("expected a list"));
        };
        list(table)?
            .iter()
            .enumerate()
            .map(|(index, item)| T::read(item, &cx.index(index + 1)))
            .collect()
    }
}

impl Read for Value {
    fn read(value: &Value, _: &Cx) -> Result<Self, Error> {
        Ok(value.clone())
    }
}

impl Read for String {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let text = match value {
            Value::String(text) => text.to_string_lossy(),
            // A number where a label goes is its text.
            Value::Integer(number) => number.to_string(),
            Value::Number(number) => number.to_string(),
            _ => return Err(cx.fail("expected a string")),
        };
        if text.len() > MAX_TEXT {
            return Err(cx.fail(format!("longer than {MAX_TEXT} bytes")));
        }
        Ok(text)
    }
}

impl Read for bool {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Boolean(value) => Ok(*value),
            _ => Err(cx.fail("expected true or false")),
        }
    }
}

impl Read for f32 {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let number = match value {
            Value::Integer(number) => *number as f64,
            Value::Number(number) if number.is_finite() => *number,
            _ => return Err(cx.fail("expected a number")),
        };
        Ok(number as f32)
    }
}

impl Read for i64 {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Integer(number) => Ok(*number),
            Value::Number(number) if number.fract() == 0.0 => Ok(*number as i64),
            _ => Err(cx.fail("expected an integer")),
        }
    }
}

impl Read for Function {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Function(function) => Ok(function.clone()),
            _ => Err(cx.fail("expected a function")),
        }
    }
}

impl Read for Table {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match value {
            Value::Table(table) => Ok(table.clone()),
            _ => Err(cx.fail("expected a table")),
        }
    }
}

/// A number that must fall in `range`.
pub(crate) fn within(value: f32, range: RangeInclusive<f32>, cx: &Cx) -> Result<f32, Error> {
    if range.contains(&value) {
        Ok(value)
    } else {
        Err(cx.fail(format!("expected {} to {}", range.start(), range.end())))
    }
}

/// The list part of a table, skipping `nil` and `false`.
pub(crate) fn list(table: &Table) -> Result<Vec<Value>, Error> {
    let mut items = Vec::new();
    for index in 1..=table.raw_len() {
        match table.raw_get::<Value>(index)? {
            Value::Nil | Value::Boolean(false) => {},
            value => items.push(value),
        }
    }
    Ok(items)
}
