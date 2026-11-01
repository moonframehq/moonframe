//! Components: ready-made pieces of view that mods share. Each is a file
//! `components/<name>.luau` in one of the app's [`Roots`](crate::Roots),
//! returning `component { title, build, ... }`; a mod gets its `build` with
//! `local stat = use "stat"` and calls it like a `ui` function. The window
//! lists every component, with a preview drawn from its `example`, so the
//! user can pick one for an agent to add.

use {
    crate::{
        Data, Error, IconName, Node, Rules, Stylesheet, TabKind, node,
        read::{Cx, Fields, NO_NAMES, Read as _},
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

    /// `use(name)` for a module's state: the component's `build`. For a
    /// component that keeps state (it has `update`), a function that mounts
    /// an instance of it instead: `tabbed { id = "main", ... }` becomes an
    /// `instance` element, which the app gives state of its own.
    pub(crate) fn install(&self, lua: &Lua) -> mlua::Result<()> {
        let library = self.clone();
        let use_ = lua.create_function(move |lua, name: String| {
            let table = library.load(lua, &name)?;
            if table.get::<Option<Function>>("update")?.is_none() {
                return table.get::<Function>("build");
            }
            lua.create_function(move |lua, props: Value| {
                let id = match &props {
                    Value::Table(props) => props.get::<Option<String>>("id")?,
                    _ => None,
                };
                let node = lua.create_table()?;
                node.raw_set("type", "instance")?;
                node.raw_set("component", name.as_str())?;
                node.raw_set("id", id.unwrap_or_else(|| "main".to_owned()))?;
                node.raw_set("props", props)?;
                Ok(node)
            })
        })?;
        lua.globals().raw_set("use", use_)
    }

    /// The file the component `name` comes from.
    pub(crate) fn path(&self, name: &str) -> Option<PathBuf> {
        self.0.get(name).map(|source| source.path.clone())
    }

    /// The component `name`'s definition table, for an instance's own state.
    pub(crate) fn definition(&self, lua: &Lua, name: &str) -> mlua::Result<Table> {
        self.load(lua, name)
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
                // A component whose styles do not read shows why instead of
                // a preview drawn without them.
                let (rules, preview) = match about.rules {
                    Ok(rules) => (rules, preview),
                    Err(error) => (Rules::default(), Err(error)),
                };
                Component {
                    name: name.clone(),
                    path: source.path.clone(),
                    title: about.title.unwrap_or_else(|| name.clone()),
                    description: about.description.unwrap_or_default(),
                    category: about.category.unwrap_or_else(|| "Other".to_owned()),
                    when: about.when.unwrap_or_default(),
                    props: about.props,
                    icon: about.icon,
                    tab: about.tab,
                    demo: about
                        .usage
                        .is_none()
                        .then(|| about.example.as_ref().map(|example| demo(name, example)))
                        .flatten(),
                    usage: about
                        .usage
                        .unwrap_or_else(|| usage(name, about.example.as_ref())),
                    rules,
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
    /// When to reach for it, and what to use instead when it is not the one.
    pub when: String,
    /// Each prop and what it does, by name.
    pub props: BTreeMap<String, String>,
    /// Shown beside its title in the browser.
    pub icon: Option<IconName>,
    /// Offered as a kind of tab, when it says so.
    pub tab: Option<TabKind>,
    /// Luau that uses it, for the user or an agent to paste into a mod.
    pub usage: String,
    /// A whole mod that shows it, run live in the browser's preview: the
    /// file `components/demos/<name>.luau`, else one made from `example`
    /// (none when the example holds elements, which cannot be written out).
    pub demo: Option<Demo>,
    /// Its default `styles`, under every mod's and the stylesheet's.
    pub rules: Rules,
    /// Its `build(example)`, or why it could not be drawn.
    pub preview: Result<Node, Error>,
}

impl Component {
    /// Whether it moves: a transition in its own styles, or a popover,
    /// overlay or split in its preview, which the stylesheet animates.
    pub fn animated(&self) -> bool {
        self.rules.animated() || self.preview.as_ref().is_ok_and(Node::moves)
    }

    /// How problems name its demo file.
    pub fn demo_name(&self) -> &str {
        self.demo
            .as_ref()
            .and_then(|demo| demo.path.as_deref())
            .and_then(|path| path.to_str())
            .and_then(|path| path.rsplit_once("/components/").map(|(_, file)| file))
            .unwrap_or(&self.name)
    }

    /// How problems name it.
    pub fn path_name(&self) -> &str {
        self.path
            .to_str()
            .and_then(|path| path.rsplit_once("/components/").map(|(_, file)| file))
            .unwrap_or(&self.name)
    }
}

struct About {
    title: Option<String>,
    description: Option<String>,
    category: Option<String>,
    when: Option<String>,
    props: BTreeMap<String, String>,
    rules: Result<Rules, Error>,
    icon: Option<IconName>,
    tab: Option<TabKind>,
    usage: Option<String>,
    example: Option<Data>,
}

impl Default for About {
    fn default() -> Self {
        Self {
            title: None,
            description: None,
            category: None,
            when: None,
            props: BTreeMap::new(),
            rules: Ok(Rules::default()),
            icon: None,
            tab: None,
            usage: None,
            example: None,
        }
    }
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
        "when",
        "props",
        "styles",
        "icon",
        "usage",
        "example",
        "build",
        "init",
        "update",
        "tab",
        "subscribe",
    ]])?;
    let styles = fields.raw("styles").cloned().unwrap_or(Value::Nil);
    let example = fields.raw("example").cloned().unwrap_or(Value::Nil);
    let about = About {
        title: fields.get("title")?,
        description: fields.get("description")?,
        category: fields.get("category")?,
        when: fields.get("when")?,
        props: fields.get("props")?.unwrap_or_default(),
        rules: Option::<Rules>::read(&styles, &Cx::root(sheet.names()).key("styles"))
            .map(Option::unwrap_or_default),
        icon: fields.get("icon")?,
        tab: fields.get("tab")?,
        usage: fields.get("usage")?,
        // No example is no example, not an empty one.
        example: Data::read(&example)
            .ok()
            .filter(|example| *example != Data::Nil),
    };
    meter.reset();
    let preview = table
        .get::<Function>("build")
        .and_then(|build| {
            // One that keeps state is drawn as it starts: `init`, then
            // `build(props, state)`.
            if table.get::<Option<Function>>("update")?.is_none() {
                return build.call::<Value>(example);
            }
            let state = match table.get::<Option<Function>>("init")? {
                Some(init) => init.call::<Value>((Value::Nil, example.clone()))?,
                None => Value::Nil,
            };
            build.call::<Value>((example, state))
        })
        .map_err(|error| Error::from(error).context("build"))
        .and_then(|tree| {
            // A preview shows its marked children; a problem with them still
            // counts as the component's.
            let (node, problems) = node::read(&tree, sheet, &Default::default())
                .map_err(|error| error.context("build"))?;
            match problems.into_iter().next() {
                Some(problem) => Err(problem.context("build")),
                None => Ok(node),
            }
        });
    Ok((about, preview))
}

/// The source of a component's live preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Demo {
    pub source: String,
    /// Where it was read from; `None` when made from the example.
    pub path: Option<PathBuf>,
}

/// A mod that draws the component `name` with its example.
fn demo(name: &str, example: &Data) -> Demo {
    let variable = name.replace('-', "_");
    let props = literal(example);
    Demo {
        source: format!(
            "-- The preview: a mod drawing `{name}` with its example. Write\n\
             -- components/demos/{name}.luau to make it a demo you can play with.\n\
             local {variable} = use \"{name}\"\n\n\
             return mod {{\n\tview = function()\n\t\treturn {variable} {props}\n\tend,\n}}\n"
        ),
        path: None,
    }
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

/// The host's elements, for the catalogue: `ui.*` and when to use each.
const ELEMENTS: &str = include_str!("catalogue/elements.md");

/// The catalogue agents read before building UI: every host element and
/// every component, grouped by category, with when to use each, its props,
/// the classes it styles, and how to call it. `just components` writes it
/// as the `moonframe-components` skill.
pub fn catalogue(components: &[Component]) -> String {
    let mut out = String::from(CATALOGUE_HEAD);
    out.push_str(ELEMENTS.trim_end());
    out.push_str("\n\n## Components\n\n");
    out.push_str(
        "Call one with `local name = use \"name\"`, then `name { ... }` wherever an \
         element goes. Its classes can be restyled in `style.luau`.\n\n",
    );
    let mut categories: BTreeMap<&str, Vec<&Component>> = BTreeMap::new();
    for component in components {
        categories
            .entry(component.category.as_str())
            .or_default()
            .push(component);
    }
    out.push_str("| Component | Category | Use it for |\n| --- | --- | --- |\n");
    for (category, list) in &categories {
        for component in list {
            let _ = writeln!(
                out,
                "| [`{}`](#{}) | {category} | {} |",
                component.name,
                component.name,
                cell(&component.when_or_description())
            );
        }
    }
    for (category, list) in &categories {
        let _ = write!(out, "\n### {category}\n");
        for component in list {
            let _ = write!(out, "\n#### `{}`\n\n", component.name);
            let _ = writeln!(out, "**{}.** {}\n", component.title, component.description);
            if !component.when.is_empty() {
                let _ = writeln!(out, "**When:** {}\n", component.when);
            }
            if !component.props.is_empty() {
                out.push_str("| Prop | Meaning |\n| --- | --- |\n");
                for (name, meaning) in &component.props {
                    let _ = writeln!(out, "| `{name}` | {} |", cell(meaning));
                }
                out.push('\n');
            }
            let selectors = component.rules.selectors();
            if !selectors.is_empty() {
                let selectors: Vec<String> = selectors
                    .iter()
                    .map(|selector| format!("`{selector}`"))
                    .collect();
                let _ = writeln!(out, "**Styles:** {}\n", selectors.join(" "));
            }
            let _ = writeln!(out, "```lua\n{}\n```", component.usage.trim_end());
            if component
                .demo
                .as_ref()
                .is_some_and(|demo| demo.path.is_some())
            {
                let _ = writeln!(
                    out,
                    "\nA whole mod using it, with state: `app/components/demos/{}.luau`.",
                    component.name
                );
            }
        }
    }
    out
}

impl Component {
    fn when_or_description(&self) -> String {
        if self.when.is_empty() {
            self.description.clone()
        } else {
            self.when.clone()
        }
    }
}

/// Text that fits in one table cell.
fn cell(text: &str) -> String {
    text.replace('\n', " ").replace('|', "\\|")
}

const CATALOGUE_HEAD: &str = "\
---
name: moonframe-components
description: The full catalogue of ready-made UI in Moonframe, every host element (`ui.*`) and every component (`use \"name\"`), with when to use each, its props, the classes it styles, and how to call it. Read it before building or changing any UI in Moonframe (a mod, a panel, a sidebar, a dialog, a menu, a status bar item, a settings page), to pick an existing piece instead of writing a new one. Generated by `just components`; pairs with the moonframe skill.
---

<!-- Generated by `just components` from app/components and
crates/moonframe-core/src/catalogue/elements.md. Do not edit by hand. -->

# Moonframe components

Everything a mod can draw. Prefer, in order: a component that fits, a
component with a `class` and a `style.luau` rule, a composition of
components, and only then new `ui.*` code. See the moonframe skill for
how mods, styles and providers work.

";
