//! Components: ready-made pieces of view that mods share. Each is a file
//! `components/<name>.luau` in one of the app's [`Roots`](crate::Roots),
//! returning `component { title, build, ... }`; a mod gets its `build` with
//! `local stat = use "stat"` and calls it like a `ui` function. The window
//! lists every component, with a preview drawn from its `example`, so the
//! user can pick one for an agent to add.

use {
    crate::{
        Data, Error, IconName, Node, Stylesheet, node,
        read::{Cx, Fields, NO_NAMES},
        vm::{self, Meter},
    },
    mlua::{Function, Lua, Table, Value},
    std::{collections::BTreeMap, fmt::Write as _, path::PathBuf, rc::Rc},
};

/// Every component's source, by name, as the reload found them.
#[derive(Clone, Debug, Default)]
pub(crate) struct Library(Rc<BTreeMap<String, Source>>);

#[derive(Debug)]
pub(crate) struct Source {
    path: PathBuf,
    text: Result<String, Error>,
}

/// Components loaded into one Luau state, by name; `false` while one loads,
/// to catch a component that uses itself.
const LOADED: &str = "moonframe.components";

impl Library {
    pub(crate) fn read(files: BTreeMap<String, PathBuf>) -> Self {
        Self(Rc::new(
            files
                .into_iter()
                .map(|(name, path)| {
                    let text = std::fs::read_to_string(&path).map_err(|error| {
                        Error::new(format!("cannot read {}: {error}", path.display()))
                    });
                    (name, Source { path, text })
                })
                .collect(),
        ))
    }

    /// `use(name)` for a module's state: the component's `build`.
    pub(crate) fn install(&self, lua: &Lua) -> mlua::Result<()> {
        let library = self.clone();
        let use_ = lua.create_function(move |lua, name: String| {
            library
                .load(lua, &name)
                .and_then(|table| table.get::<Function>("build"))
        })?;
        lua.globals().raw_set("use", use_)
    }

    /// The component `name`'s definition, run once per state.
    fn load(&self, lua: &Lua, name: &str) -> mlua::Result<Table> {
        let loaded = match lua.named_registry_value::<Option<Table>>(LOADED)? {
            Some(loaded) => loaded,
            None => {
                let loaded = lua.create_table()?;
                lua.set_named_registry_value(LOADED, &loaded)?;
                loaded
            },
        };
        match loaded.raw_get::<Value>(name)? {
            Value::Table(table) => return Ok(table),
            Value::Boolean(false) => {
                return Err(mlua::Error::runtime(format!(
                    "component `{name}` uses itself"
                )));
            },
            _ => {},
        }
        let source = self.0.get(name).ok_or_else(|| {
            let known: Vec<&str> = self.0.keys().map(String::as_str).collect();
            mlua::Error::runtime(format!(
                "no component named `{name}`; there are: {}",
                known.join(", ")
            ))
        })?;
        let text = source
            .text
            .as_ref()
            .map_err(|error| mlua::Error::runtime(error.to_string()))?;
        loaded.raw_set(name, false)?;
        let value = vm::text(lua, text, &format!("components/{name}")).eval::<Value>();
        let table = match value {
            Ok(Value::Table(table)) if table.get::<Function>("build").is_ok() => Ok(table),
            Ok(_) => Err(mlua::Error::runtime(format!(
                "components/{name}.luau must return component {{ build = function(props) ... end, ... }}"
            ))),
            Err(error) => Err(error),
        };
        match &table {
            Ok(table) => loaded.raw_set(name, table)?,
            Err(_) => loaded.raw_set(name, Value::Nil)?,
        }
        table
    }

    /// Every component, read for the window's list.
    pub(crate) fn catalogue(&self, sheet: &Stylesheet) -> Vec<Component> {
        self.0
            .iter()
            .map(|(name, source)| {
                let read = describe(self, name, sheet);
                let (about, preview) = match read {
                    Ok((about, preview)) => (about, preview),
                    Err(error) => (About::default(), Err(error)),
                };
                Component {
                    name: name.clone(),
                    path: source.path.clone(),
                    title: about.title.unwrap_or_else(|| name.clone()),
                    description: about.description.unwrap_or_default(),
                    category: about.category.unwrap_or_else(|| "Other".to_owned()),
                    icon: about.icon,
                    usage: about
                        .usage
                        .unwrap_or_else(|| usage(name, about.example.as_ref())),
                    preview,
                }
            })
            .collect()
    }
}

/// A component as the window lists it.
#[derive(Clone, Debug)]
pub struct Component {
    pub name: String,
    pub path: PathBuf,
    pub title: String,
    pub description: String,
    pub category: String,
    /// Shown beside its title in the browser.
    pub icon: Option<IconName>,
    /// Luau that uses it, for the user or an agent to paste into a mod.
    pub usage: String,
    /// Its `build(example)`, or why it could not be drawn.
    pub preview: Result<Node, Error>,
}

impl Component {
    /// How problems name it.
    pub fn path_name(&self) -> &str {
        self.path
            .to_str()
            .and_then(|path| path.rsplit_once("/components/").map(|(_, file)| file))
            .unwrap_or(&self.name)
    }
}

#[derive(Default)]
struct About {
    title: Option<String>,
    description: Option<String>,
    category: Option<String>,
    icon: Option<IconName>,
    usage: Option<String>,
    example: Option<Data>,
}

/// Loads the component `name` on its own and draws its example.
fn describe(
    library: &Library,
    name: &str,
    sheet: &Stylesheet,
) -> Result<(About, Result<Node, Error>), Error> {
    let meter = Meter::default();
    let lua = vm::open(&format!("components/{name}"), &meter, library)?;
    let table = library.load(&lua, name)?;
    let fields = Fields::read(&Value::Table(table.clone()), &Cx::root(&NO_NAMES), &[&[
        "title",
        "description",
        "category",
        "icon",
        "usage",
        "example",
        "build",
    ]])?;
    let example = fields.raw("example").cloned().unwrap_or(Value::Nil);
    let about = About {
        title: fields.get("title")?,
        description: fields.get("description")?,
        category: fields.get("category")?,
        icon: fields.get("icon")?,
        usage: fields.get("usage")?,
        example: Data::read(&example).ok(),
    };
    meter.reset();
    let preview = table
        .get::<Function>("build")
        .and_then(|build| build.call::<Value>(example))
        .map_err(|error| Error::from(error).context("build"))
        .and_then(|tree| {
            // A preview shows its marked children; a problem with them still
            // counts as the component's.
            let (node, problems) =
                node::read(&tree, sheet).map_err(|error| error.context("build"))?;
            match problems.into_iter().next() {
                Some(problem) => Err(problem.context("build")),
                None => Ok(node),
            }
        });
    Ok((about, preview))
}

/// Luau that uses the component with its example.
fn usage(name: &str, example: Option<&Data>) -> String {
    let variable = name.replace('-', "_");
    let props = example.map_or_else(|| "{}".to_owned(), literal);
    format!("local {variable} = use \"{name}\"\n\n{variable} {props}")
}

/// `data` as a Luau table constructor or value.
fn literal(data: &Data) -> String {
    match data {
        Data::Nil => "nil".to_owned(),
        Data::Bool(value) => value.to_string(),
        Data::Integer(value) => value.to_string(),
        Data::Number(value) => value.to_string(),
        Data::String(text) => format!("{text:?}"),
        Data::List(items) => {
            let items: Vec<String> = items.iter().map(literal).collect();
            format!("{{ {} }}", items.join(", "))
        },
        Data::Map(fields) => {
            let mut out = String::from("{ ");
            for (index, (key, value)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                let plain = key
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                    && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if plain {
                    let _ = write!(out, "{key} = {}", literal(value));
                } else {
                    let _ = write!(out, "[{key:?}] = {}", literal(value));
                }
            }
            out.push_str(" }");
            out
        },
    }
}
