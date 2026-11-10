//! Element problems as data: what a view's reader refused, where, and what
//! to write instead. The window draws a warning sign in a refused element's
//! place; these say the same to the host, an agent, or a check.

use {
    crate::{
        Error,
        error::Detail,
        schema::{SCHEMA_VERSION, nearest},
    },
    mlua::Value,
    std::fmt,
};

/// Element keys on their way out: `(element, key, replacement, removed in)`,
/// against [`SCHEMA_VERSION`](crate::SCHEMA_VERSION).
/// `*` is any element. While `removed in` is above [`SCHEMA_VERSION`] the
/// key still works and draws a deprecation warning; from then on it is an
/// unknown key, whose diagnostic still names the replacement.
pub(crate) const DEPRECATED: &[Deprecation] = &[];

/// `(element, key, replacement, removed in)`.
type Deprecation = (&'static str, &'static str, &'static str, u32);

/// How a view problem counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ViewLevel {
    /// The element could not be read: a warning sign stands in its place.
    #[default]
    Broken,
    /// It draws, but uses a key that is going away.
    Deprecated,
    /// It draws, but a strict check refuses it: a class or state no rule
    /// styles, or a deprecated key.
    Strict,
}

/// One element a view drew wrong, for the host and agents.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ViewDiagnostic {
    /// The mod or instance that drew it: `shell`, `files`, `user:notes`.
    pub module: String,
    /// The element's path in its view: `column[1].row[2].button`.
    pub path: String,
    /// Its kind, such as `button`, when the reader got that far.
    pub element: Option<String>,
    /// The key at fault, such as `padding` or `style.background`.
    pub key: Option<String>,
    /// What the key was given, short.
    pub value: Option<String>,
    /// The line of the module's file that built the element.
    pub line: Option<u32>,
    /// What is wrong, without the path.
    pub message: String,
    /// What to write instead.
    pub suggestion: Option<String>,
    pub level: ViewLevel,
}

impl ViewDiagnostic {
    /// Reads `error`, a problem `module`'s view had.
    pub(crate) fn new(module: &str, error: &Error) -> Self {
        let Some(detail) = error.detail() else {
            return Self {
                module: module.to_owned(),
                path: String::new(),
                element: None,
                key: None,
                value: None,
                line: None,
                message: error.to_string(),
                suggestion: None,
                level: ViewLevel::Broken,
            };
        };
        let (path, element) = match &detail.element {
            Some((path, kind)) => (path.clone(), Some((*kind).to_owned())),
            None => (detail.at.clone(), None),
        };
        Self {
            module: module.to_owned(),
            key: key_of(detail),
            path,
            element,
            value: detail.value.clone(),
            line: detail.line,
            message: detail.problem.clone(),
            suggestion: detail.suggestion.clone(),
            level: detail.level,
        }
    }
}

impl fmt::Display for ViewDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.module)?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
        }
        write!(f, ": {}", self.path)?;
        if let Some(key) = &self.key {
            write!(f, ".{key}")?;
        }
        write!(f, ": {}", self.message)?;
        if let Some(suggestion) = &self.suggestion {
            write!(f, " ({suggestion})")?;
        }
        Ok(())
    }
}

/// The key a problem is about: named outright, else the path past the
/// element's own.
fn key_of(detail: &Detail) -> Option<String> {
    if detail.key.is_some() {
        return detail.key.clone();
    }
    let (element, _) = detail.element.as_ref()?;
    let rest = detail
        .at
        .strip_prefix(element.as_str())?
        .strip_prefix('.')?;
    (!rest.is_empty()).then(|| rest.to_owned())
}

/// A value as a diagnostic quotes it: a number, a short string, or its type.
pub(crate) fn short(value: &Value) -> String {
    const LONGEST: usize = 40;
    match value {
        Value::String(text) => {
            let text = text.to_string_lossy();
            let cut: String = text.chars().take(LONGEST).collect();
            if cut.len() < text.len() {
                format!("\"{cut}…\"")
            } else {
                format!("\"{cut}\"")
            }
        },
        Value::Integer(number) => number.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Boolean(flag) => flag.to_string(),
        Value::Nil => "nil".to_owned(),
        Value::Table(_) => "{…}".to_owned(),
        other => other.type_name().to_owned(),
    }
}

/// "did you mean `a` or `b`?", when any are close.
pub(crate) fn did_you_mean(name: &str, candidates: &[&str]) -> Option<String> {
    let close = nearest(name, candidates.iter().copied());
    (!close.is_empty()).then(|| {
        let quoted: Vec<String> = close.iter().map(|name| format!("`{name}`")).collect();
        format!("did you mean {}?", quoted.join(" or "))
    })
}

/// Keys an element does not take that people reach for, with what to write.
const MISTAKES: &[(&str, &str, &str)] = &[
    (
        "button",
        "selected",
        "ui.button has no `selected`; use `variant` (\"primary\", \"ghost\", ...) to mark the current one, or ui.item { selected = true } in a list",
    ),
    (
        "button",
        "text",
        "ui.button takes its text as `label` (or first, positionally)",
    ),
    (
        "button",
        "onclick",
        "ui.button sends `msg` (with `to` for another mod) when clicked",
    ),
    (
        "button",
        "on_click",
        "ui.button sends `msg` (with `to` for another mod) when clicked",
    ),
    (
        "text",
        "color",
        "ui.text takes `tone`; colors belong in styles (`style = { color = ... }` or a class rule)",
    ),
    (
        "*",
        "onclick",
        "elements send `msg` (with `to`) rather than run callbacks",
    ),
    (
        "*",
        "margin",
        "elements take no `margin`; use the parent's `gap`, or `style = { margin = ... }`",
    ),
    (
        "*",
        "align",
        "a row or column aligns its children with `self_align` on each, or `style`",
    ),
];

/// Space keys, which take a token on an element and pixels in styles.
const SPACE_KEYS: &[&str] = &["padding", "gap"];

/// The suggestion for an unknown `key` on the element `kind`, which takes
/// `takes`.
pub(crate) fn unknown_key(kind: Option<&str>, key: &str, takes: &[&str]) -> Option<String> {
    unknown_key_in(DEPRECATED, kind, key, takes)
}

fn unknown_key_in(
    table: &[Deprecation],
    kind: Option<&str>,
    key: &str,
    takes: &[&str],
) -> Option<String> {
    if let Some((_, _, replacement, removed)) = table
        .iter()
        .find(|(element, old, ..)| *old == key && (Some(*element) == kind || *element == "*"))
    {
        return Some(format!(
            "`{key}` was renamed to `{replacement}` (removed in schema {removed})"
        ));
    }
    MISTAKES
        .iter()
        .find(|(element, wrong, _)| *wrong == key && (Some(*element) == kind || *element == "*"))
        .map(|(.., hint)| (*hint).to_owned())
        .or_else(|| did_you_mean(key, takes))
}

/// A suggestion for a problem the reader gave none for, from what it was
/// given.
pub(crate) fn suggest(detail: &mut Detail) {
    if detail.suggestion.is_some() {
        return;
    }
    let key = key_of(detail);
    let last = key
        .as_deref()
        .and_then(|key| key.rsplit('.').next())
        .unwrap_or_default();
    let numeric = detail
        .value
        .as_deref()
        .is_some_and(|value| value.parse::<f64>().is_ok());
    if SPACE_KEYS.contains(&last) && numeric && key.as_deref() == Some(last) {
        detail.suggestion = Some(format!(
            "{last} takes {} on an element; pixels belong in styles (`style = {{ {last} = {} }}` or a class rule)",
            crate::Space::NAMES.join("|"),
            detail.value.as_deref().unwrap_or("8"),
        ));
    }
}

/// The deprecated keys among `keys` of the element `kind`, still taken:
/// `(key, replacement)`.
pub(crate) fn deprecated<'a>(
    kind: &str,
    keys: impl Iterator<Item = &'a str>,
) -> Vec<(&'a str, &'static str)> {
    deprecated_in(DEPRECATED, kind, keys)
}

fn deprecated_in<'a>(
    table: &[Deprecation],
    kind: &str,
    keys: impl Iterator<Item = &'a str>,
) -> Vec<(&'a str, &'static str)> {
    keys.filter_map(|key| {
        table
            .iter()
            .find(|(element, old, _, removed)| {
                *old == key && (*element == kind || *element == "*") && *removed > SCHEMA_VERSION
            })
            .map(|(_, _, replacement, _)| (key, *replacement))
    })
    .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const TABLE: &[Deprecation] = &[
        ("button", "kind", "variant", SCHEMA_VERSION + 1),
        ("*", "grow_x", "grow", SCHEMA_VERSION),
    ];

    #[test]
    fn a_deprecated_key_warns_until_removed_then_names_its_replacement() {
        let keys = ["label", "kind"];
        assert_eq!(deprecated_in(TABLE, "button", keys.into_iter()), vec![(
            "kind", "variant"
        )]);
        assert!(deprecated_in(TABLE, "text", keys.into_iter()).is_empty());
        // Removed: no longer taken, and the unknown key says what replaced it.
        assert!(deprecated_in(TABLE, "row", ["grow_x"].into_iter()).is_empty());
        let hint = unknown_key_in(TABLE, Some("row"), "grow_x", &["grow"]).unwrap();
        assert!(hint.contains("renamed to `grow`"), "{hint}");
    }

    #[test]
    fn suggestions_name_the_right_form() {
        let hint = unknown_key(Some("button"), "selected", &["label", "variant"]).unwrap();
        assert!(hint.contains("variant"), "{hint}");
        let hint = unknown_key(Some("row"), "paddng", &["padding", "gap"]).unwrap();
        assert_eq!(hint, "did you mean `padding`?");
        assert_eq!(unknown_key(Some("row"), "zzzzzz", &["padding"]), None);
    }
}
