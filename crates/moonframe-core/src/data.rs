//! Plain values that cross between modules and the app: messages, and state
//! carried over a reload. Free of functions, userdata, and cycles, and
//! bounded, so a module cannot hand the host something it cannot keep.

use {
    crate::{
        Error,
        read::{Cx, NO_NAMES, Read},
    },
    mlua::{Lua, Table, Value},
    std::collections::BTreeMap,
};

const MAX_DEPTH: usize = 16;
const MAX_ENTRIES: usize = 10_000;
/// A string as long as a document, so a file's text can travel in a
/// message and live in state.
const MAX_STRING: usize = crate::read::MAX_DOCUMENT;
/// All the strings of one value together: a few documents at most.
const MAX_BYTES: usize = 4 * crate::read::MAX_DOCUMENT;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Data {
    #[default]
    Nil,
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
    List(Vec<Data>),
    Map(BTreeMap<String, Data>),
}

impl Data {
    /// Reads a Lua value, refusing what cannot cross: functions, userdata,
    /// mixed or sparse tables, and anything past the bounds.
    pub fn read(value: &Value) -> Result<Self, Error> {
        <Self as Read>::read(value, &Cx::root(&NO_NAMES))
    }

    pub fn to_lua(&self, lua: &Lua) -> mlua::Result<Value> {
        Ok(match self {
            Self::Nil => Value::Nil,
            Self::Bool(value) => Value::Boolean(*value),
            Self::Integer(value) => Value::Integer(*value),
            Self::Number(value) => Value::Number(*value),
            Self::String(value) => Value::String(lua.create_string(value)?),
            Self::List(items) => {
                let table = lua.create_table_with_capacity(items.len(), 0)?;
                for item in items {
                    table.raw_push(item.to_lua(lua)?)?;
                }
                Value::Table(table)
            },
            Self::Map(entries) => {
                let table = lua.create_table_with_capacity(0, entries.len())?;
                for (key, value) in entries {
                    table.raw_set(key.as_str(), value.to_lua(lua)?)?;
                }
                Value::Table(table)
            },
        })
    }
}

impl From<&str> for Data {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl Read for Data {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        Budget::default().data(value, cx, 0)
    }
}

/// What one value may still spend, so a module cannot hand over a huge or
/// deep table.
#[derive(Default)]
struct Budget {
    entries: usize,
    bytes: usize,
}

impl Budget {
    fn data(&mut self, value: &Value, cx: &Cx, depth: usize) -> Result<Data, Error> {
        self.entries += 1;
        if self.entries > MAX_ENTRIES {
            return Err(cx.fail(format!("more than {MAX_ENTRIES} values")));
        }
        match value {
            Value::Nil => Ok(Data::Nil),
            Value::Boolean(value) => Ok(Data::Bool(*value)),
            Value::Integer(value) => Ok(Data::Integer(*value)),
            Value::Number(value) => Ok(Data::Number(*value)),
            Value::String(value) => {
                let value = value.to_string_lossy();
                if value.len() > MAX_STRING {
                    return Err(cx.fail(format!("a string longer than {MAX_STRING} bytes")));
                }
                self.spend(value.len(), cx)?;
                Ok(Data::String(value))
            },
            Value::Table(_) if depth == MAX_DEPTH => {
                Err(cx.fail(format!("nested deeper than {MAX_DEPTH}")))
            },
            Value::Table(table) if table.raw_len() > 0 => self.list(table, cx, depth),
            Value::Table(table) => self.map(table, cx, depth),
            other => Err(cx.fail(format!("a {} cannot leave its module", other.type_name()))),
        }
    }

    fn spend(&mut self, bytes: usize, cx: &Cx) -> Result<(), Error> {
        self.bytes += bytes;
        if self.bytes > MAX_BYTES {
            return Err(cx.fail(format!("more than {MAX_BYTES} bytes of text")));
        }
        Ok(())
    }

    fn list(&mut self, table: &Table, cx: &Cx, depth: usize) -> Result<Data, Error> {
        let len = table.raw_len();
        if table.pairs::<Value, Value>().count() != len {
            return Err(cx.fail("a table is either a list without gaps or has only string keys"));
        }
        (1..=len)
            .map(|index| self.data(&table.raw_get(index)?, &cx.index(index), depth + 1))
            .collect::<Result<_, _>>()
            .map(Data::List)
    }

    fn map(&mut self, table: &Table, cx: &Cx, depth: usize) -> Result<Data, Error> {
        let mut map = BTreeMap::new();
        for pair in table.pairs::<Value, Value>() {
            match pair? {
                (Value::String(key), value) => {
                    let key = key.to_string_lossy();
                    if key.len() > MAX_STRING {
                        return Err(cx.fail(format!("a key longer than {MAX_STRING} bytes")));
                    }
                    self.spend(key.len(), cx)?;
                    let value = self.data(&value, &cx.key(&key), depth + 1)?;
                    map.insert(key, value);
                },
                _ => {
                    return Err(
                        cx.fail("a table is either a list without gaps or has only string keys")
                    );
                },
            }
        }
        Ok(Data::Map(map))
    }
}
