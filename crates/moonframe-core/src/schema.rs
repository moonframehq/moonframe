//! What a view may say, as data: every element `ui` builds with the keys it
//! takes and their types, the icon names, the style properties and the
//! token sets. Editors, agents and checkers read it instead of the reader's
//! code; the key lists come from the reader itself, so they cannot drift.

mod json;
mod keys;
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

use {
    crate::{Data, ICONS},
    keys::key_type,
};

/// The schema's version, raised when an element key is removed or renamed
/// (or a key's type narrows), so a cached copy knows it is stale; a key is
/// listed in the reader's deprecations, still working, for at least one
/// version before. Additions keep it.
pub const SCHEMA_VERSION: u32 = 1;

/// Everything a view and a stylesheet may name.
#[derive(Clone, Debug, PartialEq)]
pub struct Schema {
    pub version: u32,
    /// Each element type, in the order `ui` declares them.
    pub elements: Vec<ElementSchema>,
    /// The bundled icon names, sorted.
    pub icons: &'static [&'static str],
    pub style: StyleSchema,
    /// The closed sets of names keys take, by the set's name.
    pub tokens: Vec<TokenSet>,
}

/// One element type: `ui.<name> { ... }`.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementSchema {
    pub name: &'static str,
    pub keys: Vec<KeySchema>,
    /// Whether its list part holds child elements.
    pub children: bool,
}

/// One key of an element, and what it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct KeySchema {
    pub name: &'static str,
    pub kind: KeyType,
}

/// What a key holds.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyType {
    /// Not described yet: see the docs for the element.
    Any,
    Bool,
    /// A number, within `min..=max` when known.
    Number {
        min: Option<f64>,
        max: Option<f64>,
    },
    String,
    /// One name of a token set.
    Tokens(TokenSet),
    /// A bundled icon's name (see [`Schema::icons`]).
    Icon,
    /// A color: a theme name, `name/alpha`, or `#rrggbb`.
    Color,
    /// Plain data: a message sent back to `update`, or props.
    Data,
    /// One element.
    Element,
    /// A list of elements.
    Elements,
    /// A function, such as a list's `row(index)`.
    Function,
    /// An element's own `style` table: the [`StyleSchema::properties`].
    Style,
    /// Any of these.
    OneOf(Vec<KeyType>),
}

/// The keys a stylesheet and a `style` table take.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleSchema {
    /// The properties of a rule or an element's `style`.
    pub properties: &'static [&'static str],
    /// The top-level keys of `style.luau`'s `style { ... }`.
    pub sheet: &'static [&'static str],
}

/// A closed set of names, such as the tones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenSet {
    pub name: &'static str,
    pub names: &'static [&'static str],
}

/// The schema of this build of the reader.
pub fn schema() -> Schema {
    let elements = crate::node::element_keys()
        .into_iter()
        .map(|(name, groups, children)| {
            let mut keys: Vec<KeySchema> = Vec::new();
            // `type` is the tag `ui` sets, never written by hand.
            for key in groups.iter().flat_map(|group| group.iter()) {
                if *key != "type" && !keys.iter().any(|known| known.name == *key) {
                    keys.push(KeySchema {
                        name: key,
                        kind: key_type(name, key),
                    });
                }
            }
            ElementSchema {
                name,
                keys,
                children,
            }
        })
        .collect();
    Schema {
        version: SCHEMA_VERSION,
        elements,
        icons: ICONS,
        style: StyleSchema {
            properties: crate::style::STYLE_KEYS,
            sheet: crate::style::SHEET_KEYS,
        },
        tokens: keys::token_sets(),
    }
}

impl Schema {
    /// The element named `name` (`"row"`, not `"ui.row"`).
    pub fn element(&self, name: &str) -> Option<&ElementSchema> {
        self.elements.iter().find(|element| element.name == name)
    }

    /// The schema as plain data: a table of `version`, `elements` (by
    /// name, each with `children` and `keys`, each key's type a table with
    /// a `type`), `icons`, `style` and `tokens`.
    pub fn to_data(&self) -> Data {
        let names = |names: &[&str]| Data::List(names.iter().map(|name| (*name).into()).collect());
        let elements = self.elements.iter().map(|element| {
            let keys = element
                .keys
                .iter()
                .map(|key| (key.name, key.kind.to_data()));
            (
                element.name,
                Data::map([
                    ("children", element.children.into()),
                    ("keys", Data::map(keys)),
                ]),
            )
        });
        let tokens = self.tokens.iter().map(|set| (set.name, names(set.names)));
        Data::map([
            ("version", i64::from(self.version).into()),
            ("elements", Data::map(elements)),
            ("icons", names(self.icons)),
            (
                "style",
                Data::map([
                    ("properties", names(self.style.properties)),
                    ("sheet", names(self.style.sheet)),
                ]),
            ),
            ("tokens", Data::map(tokens)),
        ])
    }

    /// [`Self::to_data`] as JSON, with keys sorted.
    pub fn to_json(&self) -> String {
        json::to_json(&self.to_data())
    }
}

impl KeyType {
    /// `{ type = "bool" }`, `{ type = "number", min = 0, max = 100 }`,
    /// `{ type = "tokens", set = "Space", names = { ... } }`,
    /// `{ type = "one_of", options = { ... } }`, and so on.
    pub fn to_data(&self) -> Data {
        let plain = |name: &str| Data::map([("type", name.into())]);
        match self {
            Self::Any => plain("any"),
            Self::Bool => plain("bool"),
            Self::Number { min, max } => {
                let mut entries = vec![("type", "number".into())];
                entries.extend(min.map(|min| ("min", min.into())));
                entries.extend(max.map(|max| ("max", max.into())));
                Data::map(entries)
            },
            Self::String => plain("string"),
            Self::Tokens(set) => Data::map([
                ("type", "tokens".into()),
                ("set", set.name.into()),
                (
                    "names",
                    Data::List(set.names.iter().map(|name| (*name).into()).collect()),
                ),
            ]),
            Self::Icon => plain("icon"),
            Self::Color => plain("color"),
            Self::Data => plain("data"),
            Self::Element => plain("element"),
            Self::Elements => plain("elements"),
            Self::Function => plain("function"),
            Self::Style => plain("style"),
            Self::OneOf(options) => Data::map([
                ("type", "one_of".into()),
                (
                    "options",
                    Data::List(options.iter().map(Self::to_data).collect()),
                ),
            ]),
        }
    }
}

/// The candidates closest to `name`, best first and at most five: those a
/// couple of letters away, then those that hold it or that it holds. For
/// "did you mean" hints on an unknown key, element, icon or token.
pub(crate) fn nearest<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<&'a str> {
    let mut close: Vec<(usize, usize, &str)> = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(order, candidate)| {
            let distance = edits(name, candidate);
            let related = name.len() >= 2
                && candidate.len() >= 2
                && (candidate.contains(name) || name.contains(candidate));
            (distance <= 2 || related).then_some((distance.min(3), order, candidate))
        })
        .collect();
    close.sort_unstable();
    let mut seen = Vec::new();
    for (_, _, candidate) in close {
        if !seen.contains(&candidate) {
            seen.push(candidate);
        }
        if seen.len() == 5 {
            break;
        }
    }
    seen
}

/// How many single-letter changes turn `a` into `b`.
pub(crate) fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut last = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(last + usize::from(ca != *cb));
            last = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}
