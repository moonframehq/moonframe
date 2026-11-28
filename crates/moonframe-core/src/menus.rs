//! What the shell asks of the menu bar and the About panel, in
//! `main.luau`:
//!
//! ```lua
//! return mod {
//!     about = { version = "1.2", description = "...", website = "https://...", copyright = "..." },
//!     menus = {
//!         { title = "File", items = {
//!             { label = "New Shell", msg = "new", to = "agent", shortcut = "secondary-t" },
//!             { label = "Compact", msg = { density = "compact" }, checked = { density = "compact" } },
//!             "-",
//!             { label = "Close Window", command = "close-window", shortcut = "secondary-w" },
//!         } },
//!     },
//!     ...
//! }
//! ```
//!
//! The host draws them after its own app menu (About, Quit) and before its
//! View menu, into which a menu of the same title merges.

use crate::{
    Action, Error,
    keys::Chord,
    read::{Cx, Fields, Read},
};

/// The most menus, and items in one, a shell may ask for.
const MOST_MENUS: usize = 12;
const MOST_ITEMS: usize = 64;

/// One menu in the bar.
#[derive(Clone, Debug, PartialEq)]
pub struct AppMenu {
    pub title: String,
    pub items: Vec<MenuEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry {
    /// `"-"`: a line between groups.
    Separator,
    Item(MenuCommand),
}

/// A menu item: what it says, what it does, and its key.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuCommand {
    pub label: String,
    /// A message for a mod (the shell, without `to`), or an app command.
    pub action: Action,
    /// A key such as `secondary-t`, checked as `keys` are; or a single key
    /// that types text (`j`), which works only while no text field or
    /// terminal has focus.
    pub shortcut: Option<Chord>,
    /// Whether it shows a check mark now.
    pub checked: bool,
    /// `checked = { layout = "library" }`: checked while the shell's state
    /// has each of these values, so a choice the shell keeps shows as
    /// ticked. None for an item checked always or never.
    pub check: Option<Vec<(String, crate::Data)>>,
}

impl MenuCommand {
    /// Names it among the menus, so a key bound to it finds it after a
    /// reload: its menu's title and its label.
    pub fn id(&self, menu: &str) -> String {
        format!("{menu}\u{1f}{}", self.label)
    }
}

/// What the About panel says besides the app's name and icon.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct About {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub website: Option<String>,
    pub copyright: Option<String>,
}

impl Read for About {
    fn read(value: &mlua::Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&[
            "name",
            "version",
            "description",
            "website",
            "copyright",
        ]])?;
        Ok(Self {
            name: fields.get("name")?,
            version: fields.get("version")?,
            description: fields.get("description")?,
            website: fields.get("website")?,
            copyright: fields.get("copyright")?,
        })
    }
}

/// `menus = { ... }`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Menus(pub Vec<AppMenu>);

impl Read for Menus {
    fn read(value: &mlua::Value, cx: &Cx) -> Result<Self, Error> {
        let mlua::Value::Table(list) = value else {
            return Err(cx.fail("menus is a list of { title, items }"));
        };
        let mut menus = Vec::new();
        for (index, menu) in list.sequence_values::<mlua::Value>().enumerate() {
            let cx = cx.index(index + 1);
            if index >= MOST_MENUS {
                return Err(cx.fail(format!("at most {MOST_MENUS} menus")));
            }
            let menu = menu.map_err(|error| cx.fail(error.to_string()))?;
            let fields = Fields::read(&menu, &cx, &[&["title", "items"]])?;
            let title: String = fields.need("title")?;
            let items = match fields.raw("items") {
                Some(mlua::Value::Table(items)) => items.clone(),
                _ => return Err(cx.key("items").fail("items is a list of items and \"-\"")),
            };
            let mut entries = Vec::new();
            for (at, item) in items.sequence_values::<mlua::Value>().enumerate() {
                let cx = cx.key("items").index(at + 1);
                if at >= MOST_ITEMS {
                    return Err(cx.fail(format!("at most {MOST_ITEMS} items in a menu")));
                }
                let item = item.map_err(|error| cx.fail(error.to_string()))?;
                entries.push(entry(&item, &cx)?);
            }
            menus.push(AppMenu {
                title,
                items: entries,
            });
        }
        Ok(Self(menus))
    }
}

/// `"-"`, or `{ label, msg, to, command, shortcut, checked }`.
fn entry(item: &mlua::Value, cx: &Cx) -> Result<MenuEntry, Error> {
    if let mlua::Value::String(text) = item {
        return match &*text.to_str().map_err(|error| cx.fail(error.to_string()))? {
            "-" => Ok(MenuEntry::Separator),
            _ => Err(cx.fail("a menu item is \"-\" or { label, msg or command }")),
        };
    }
    let fields = Fields::read(item, cx, &[&[
        "label", "msg", "to", "command", "shortcut", "checked",
    ]])?;
    let label: String = fields.need("label")?;
    let action = Action::from_fields(&fields)?
        .ok_or_else(|| cx.fail("a menu item needs `msg` (with `to`) or `command`"))?;
    if matches!(action, Action::Browser { .. }) {
        return Err(cx.key("command").fail("a menu item cannot drive a browser"));
    }
    let shortcut = fields
        .get::<String>("shortcut")?
        .map(|name| Chord::parse_typed(&name).map_err(|problem| cx.key("shortcut").fail(problem)))
        .transpose()?;
    let (checked, check) = match fields.raw("checked") {
        None | Some(mlua::Value::Nil) => (false, None),
        Some(mlua::Value::Boolean(on)) => (*on, None),
        Some(value @ mlua::Value::Table(_)) => {
            let cx = cx.key("checked");
            let crate::Data::Map(values) =
                crate::Data::read(value).map_err(|error| cx.fail(error.to_string()))?
            else {
                return Err(
                    cx.fail("checked is true, false, or { key = value } of the shell's state")
                );
            };
            if values.is_empty() {
                return Err(cx.fail("checked = { key = value } names at least one key"));
            }
            (false, Some(values.into_iter().collect()))
        },
        Some(_) => {
            return Err(cx
                .key("checked")
                .fail("checked is true, false, or { key = value } of the shell's state"));
        },
    };
    Ok(MenuEntry::Item(MenuCommand {
        label,
        action,
        shortcut,
        checked,
        check,
    }))
}
