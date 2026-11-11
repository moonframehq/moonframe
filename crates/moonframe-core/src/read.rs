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

/// The most positions a list may have.
const MAX_ITEMS: usize = 100_000;

/// Longer strings are refused; no label or color needs more.
const MAX_TEXT: usize = 16 * 1024;

/// The longest document a view can pass: a `ui.markdown`'s text, or a
/// multiline `ui.input`'s value. Files past it are not opened as text.
pub const MAX_DOCUMENT: usize = 1024 * 1024;

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
    /// The element being read, its path and kind, for problems to name.
    element: Option<std::rc::Rc<(String, &'static str)>>,
}

impl<'p> Cx<'p> {
    pub(crate) fn root(names: &'p Names) -> Self {
        Self {
            at: String::new(),
            names,
            depth: 0,
            element: None,
        }
    }

    /// The element `kind` under this path: `column[2].button`.
    pub(crate) fn element(&self, kind: &'static str) -> Self {
        let cx = self.key(kind);
        Self {
            element: Some(std::rc::Rc::new((cx.at.clone(), kind))),
            ..cx
        }
    }

    pub(crate) fn names(&self) -> &'p Names {
        self.names
    }

    /// The kind of the element being read, if any.
    pub(crate) fn kind(&self) -> Option<&'static str> {
        self.element.as_deref().map(|(_, kind)| *kind)
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

    /// A named key that holds an element, one level down: a popover's
    /// `content`. It counts toward the tree's depth as a list position
    /// does, so a chain of them cannot recurse past the limit.
    pub(crate) fn nested(&self, key: &str) -> Self {
        Self {
            depth: self.depth + 1,
            ..self.key(key)
        }
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
        let element = self.element.as_deref().cloned();
        Error::at(&self.at, problem).with(|detail| detail.element = element)
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

/// A table of string keys, such as a component's `props`.
impl<T: Read> Read for std::collections::BTreeMap<String, T> {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let Value::Table(table) = value else {
            return Err(cx.fail("expected a table of named values"));
        };
        let mut map = Self::new();
        for pair in table.pairs::<Value, Value>() {
            let (key, value) = pair?;
            let Value::String(key) = key else {
                return Err(cx.fail("expected string keys"));
            };
            let key = key.to_string_lossy();
            let read = T::read(&value, &cx.key(&key))?;
            map.insert(key, read);
        }
        Ok(map)
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
            // Measured before it is copied out of Luau.
            Value::String(text) if text.as_bytes().len() > MAX_TEXT => {
                return Err(cx.fail(format!("longer than {MAX_TEXT} bytes")));
            },
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

/// A whole document, up to [`MAX_DOCUMENT`] bytes, where a label's
/// [`MAX_TEXT`] would be too short: a markdown file, a text area's value.
pub(crate) struct Document(pub String);

impl Read for Document {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let Value::String(text) = value else {
            return Err(cx.fail("expected a string"));
        };
        if text.as_bytes().len() > MAX_DOCUMENT {
            return Err(cx.fail(format!("longer than {MAX_DOCUMENT} bytes")));
        }
        Ok(Self(text.to_string_lossy()))
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
            Value::Number(number) => *number,
            _ => return Err(cx.fail("expected a number")),
        };
        // Finite as a Lua number may still overflow to infinity as an f32.
        let number = number as f32;
        if !number.is_finite() {
            return Err(cx.fail("expected a finite number"));
        }
        Ok(number)
    }
}

impl Read for f64 {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let number = match value {
            Value::Integer(number) => *number as f64,
            Value::Number(number) => *number,
            _ => return Err(cx.fail("expected a number")),
        };
        if !number.is_finite() {
            return Err(cx.fail("expected a finite number"));
        }
        Ok(number)
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
    // Up to the highest position, not Lua's length: a `nil` child such as
    // `cond and ui.icon "x"` leaves a hole, and the length may stop at it
    // and drop the children after it.
    let mut last = 0;
    for pair in table.pairs::<Value, Value>() {
        if let (Value::Integer(index), _) = pair? {
            last = last.max(position(index));
        }
    }
    list_to(table, last)
}

/// A list position as a count; one below 1 is no position.
pub(crate) fn position(index: i64) -> usize {
    usize::try_from(index).unwrap_or(0)
}

/// The list part of a table up to `last`, its highest position, skipping
/// `nil` and `false`.
pub(crate) fn list_to(table: &Table, last: usize) -> Result<Vec<Value>, Error> {
    if last > MAX_ITEMS {
        return Err(Error::new(format!("more than {MAX_ITEMS} list positions")));
    }
    let mut items = Vec::new();
    for index in 1..=last {
        match table.raw_get::<Value>(index)? {
            Value::Nil | Value::Boolean(false) => {},
            value => items.push(value),
        }
    }
    Ok(items)
}
