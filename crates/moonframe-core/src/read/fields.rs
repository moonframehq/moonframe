use {
    super::{Cx, Read, list_to, position, within},
    crate::Error,
    mlua::{Table, Value},
    std::{collections::BTreeMap, ops::RangeInclusive},
};

/// A table's named keys, checked against the keys its reader takes, and its
/// list part.
pub(crate) struct Fields<'p> {
    cx: Cx<'p>,
    named: BTreeMap<String, Value>,
    items: Vec<Value>,
    line: Option<u32>,
}

/// The key `ui` functions stamp on each element: the line that called them.
/// Every table may have it, whatever keys its reader takes.
pub(crate) const SOURCE_LINE: &str = "__line";

impl<'p> Fields<'p> {
    /// Reads `value` as a table whose names all appear in `keys`, which come
    /// in groups so readers can share them.
    pub(crate) fn read(value: &Value, cx: &Cx<'p>, keys: &[&[&str]]) -> Result<Self, Error> {
        let Value::Table(table) = value else {
            return Err(cx.fail("expected a table"));
        };
        Self::table(table, cx, keys)
    }

    pub(crate) fn table(table: &Table, cx: &Cx<'p>, keys: &[&[&str]]) -> Result<Self, Error> {
        let mut named = BTreeMap::new();
        let (mut line, mut last) = (None, 0);
        for pair in table.pairs::<Value, Value>() {
            match pair? {
                (Value::String(key), value) if key == SOURCE_LINE => {
                    line = match value {
                        Value::Integer(number) => u32::try_from(number).ok(),
                        _ => None,
                    };
                },
                (Value::String(key), value) => {
                    let key = key.to_string_lossy();
                    if !keys.iter().any(|group| group.contains(&key.as_str())) {
                        return Err(cx.fail(unknown(&key, keys)));
                    }
                    named.insert(key, value);
                },
                (Value::Integer(index), _) => last = last.max(position(index)),
                _ => return Err(cx.fail("keys are names or list positions")),
            }
        }
        Ok(Self {
            cx: cx.clone(),
            named,
            items: list_to(table, last)?,
            line,
        })
    }

    /// Where the element was built, when `ui` stamped it.
    pub(crate) fn line(&self) -> Option<u32> {
        self.line
    }

    pub(crate) fn cx(&self) -> &Cx<'p> {
        &self.cx
    }

    pub(crate) fn items(&self) -> &[Value] {
        &self.items
    }

    pub(crate) fn raw(&self, key: &str) -> Option<&Value> {
        self.named.get(key)
    }

    /// The key as a `T`, or `None` when it is absent.
    pub(crate) fn get<T: Read>(&self, key: &str) -> Result<Option<T>, Error> {
        self.named
            .get(key)
            .map(|value| T::read(value, &self.cx.key(key)))
            .transpose()
    }

    /// The key as a `T`, which must be present.
    pub(crate) fn need<T: Read>(&self, key: &str) -> Result<T, Error> {
        self.get(key)?
            .ok_or_else(|| self.cx.key(key).fail("missing"))
    }

    /// A number key that must fall in `range`.
    pub(crate) fn number(
        &self,
        key: &str,
        range: RangeInclusive<f32>,
    ) -> Result<Option<f32>, Error> {
        self.get::<f32>(key)?
            .map(|number| within(number, range, &self.cx.key(key)))
            .transpose()
    }
}

fn unknown(key: &str, keys: &[&[&str]]) -> String {
    // `type` tags elements; it is never a key a script chooses.
    let takes: Vec<&str> = keys
        .iter()
        .flat_map(|group| group.iter().copied())
        .filter(|key| *key != "type")
        .collect();
    format!("unknown key `{key}`; it takes {}", takes.join(", "))
}
