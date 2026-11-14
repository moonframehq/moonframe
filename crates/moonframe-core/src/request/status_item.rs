//! `app:status-item`: a mod's item in the menu bar, beside the clock: an
//! icon or a short title, a tooltip, and a menu whose items send the mod
//! (or `to`) their `msg`. Each mod has one at most; asking again replaces
//! it, `false` takes it away, and `{ popup = items }` drops a menu down
//! from it at once. Only macOS draws one; elsewhere it does nothing.

use {
    super::APP,
    crate::{Chord, Data, Error, SHELL},
};

/// Longest a title, a tooltip and an item's label can be, in bytes.
const MAX_TITLE: usize = 64;
const MAX_TOOLTIP: usize = 256;
const MAX_LABEL: usize = 256;
/// Most items in its menu, separators included.
const MAX_ITEMS: usize = 64;
/// Longest a symbol's name or an image's path can be.
const MAX_NAME: usize = 128;
const MAX_PATH: usize = 1024;

/// What `app:status-item` asks.
#[derive(Clone, Debug, PartialEq)]
pub enum StatusItem {
    /// Shows the mod's item as this, or changes it to this.
    Show(StatusLook),
    /// Drops this menu down from the mod's item, once.
    Popup(Vec<StatusEntry>),
    /// Takes the mod's item away.
    Remove,
}

/// What the item shows, and its menu.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusLook {
    pub icon: Option<StatusIcon>,
    /// Drawn in the menu bar's own colour, as its other icons are: its
    /// shape counts, not its colours. On unless `template = false`.
    pub template: bool,
    /// Text beside the icon (or alone): `✓`, `3`.
    pub title: Option<String>,
    pub tooltip: Option<String>,
    pub items: Vec<StatusEntry>,
}

/// The item's icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusIcon {
    /// One of the system's symbols (SF Symbols on macOS): `tray.and.arrow.down`.
    Symbol(String),
    /// An image file, as `ui.image` names one: relative to the mod's
    /// folder, `~/...` or absolute, inside the app's folders.
    Image(String),
}

/// A line in the item's menu.
#[derive(Clone, Debug, PartialEq)]
pub enum StatusEntry {
    /// `"-"`.
    Separator,
    Item {
        label: String,
        /// What a click sends; an item without one is a heading, never
        /// clickable.
        msg: Option<Data>,
        /// Who hears it: the mod that owns the item unless named.
        to: Option<String>,
        enabled: bool,
        /// Shows a check mark.
        checked: bool,
        /// A key shown beside the label, such as a hotkey's: it shows,
        /// and does nothing by itself.
        shortcut: Option<Chord>,
    },
}

/// `false`, `{ popup = { items } }`, or `{ symbol | image, template,
/// title, tooltip, items }`.
pub(super) fn status_item(msg: &Data) -> Result<StatusItem, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}status-item`: {problem}"));
    let usage = "takes { symbol = \"...\" or image = \"...\", title, tooltip, items }, \
                 { popup = items } or false";
    let fields = match msg {
        Data::Bool(false) => return Ok(StatusItem::Remove),
        Data::Map(fields) => fields,
        _ => return Err(fail(usage)),
    };
    if let Some(popup) = fields.get("popup") {
        if fields.len() > 1 {
            return Err(fail("`popup` comes alone"));
        }
        let items = entries(popup).map_err(|problem| fail(&problem))?;
        if items.is_empty() {
            return Err(fail("`popup` is a list of at least one item"));
        }
        return Ok(StatusItem::Popup(items));
    }
    if let Some(key) = fields.keys().find(|key| {
        !matches!(
            key.as_str(),
            "symbol" | "image" | "template" | "title" | "tooltip" | "items"
        )
    }) {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes symbol, image, template, title, tooltip, items \
             and popup"
        )));
    }
    let text = |key: &str, max: usize| match fields.get(key) {
        None => Ok(None),
        Some(Data::String(text)) if text.len() <= max && !text.contains('\n') => {
            Ok(Some(text.clone()))
        },
        Some(_) => Err(fail(&format!("`{key}` is one line of at most {max} bytes"))),
    };
    let symbol = text("symbol", MAX_NAME)?;
    if symbol.as_ref().is_some_and(|symbol| {
        symbol.is_empty()
            || !symbol
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
    }) {
        return Err(fail(
            "`symbol` is a system symbol's name: a-z, 0-9 and dots, such as `tray.and.arrow.down`",
        ));
    }
    let image = text("image", MAX_PATH)?;
    if image
        .as_ref()
        .is_some_and(|image| image.is_empty() || image.contains("://"))
    {
        return Err(fail("`image` is an image file in the app's folders"));
    }
    let icon = match (symbol, image) {
        (Some(_), Some(_)) => return Err(fail("`symbol` or `image`, not both")),
        (Some(symbol), None) => Some(StatusIcon::Symbol(symbol)),
        (None, Some(image)) => Some(StatusIcon::Image(image)),
        (None, None) => None,
    };
    let template = match fields.get("template") {
        None => true,
        Some(Data::Bool(template)) => *template,
        Some(_) => return Err(fail("`template` is true or false")),
    };
    let title = text("title", MAX_TITLE)?.filter(|title| !title.is_empty());
    if icon.is_none() && title.is_none() {
        return Err(fail("needs a `symbol`, an `image` or a `title` to show"));
    }
    let items = match fields.get("items") {
        None => Vec::new(),
        Some(items) => entries(items).map_err(|problem| fail(&problem))?,
    };
    Ok(StatusItem::Show(StatusLook {
        icon,
        template,
        title,
        tooltip: text("tooltip", MAX_TOOLTIP)?.filter(|tooltip| !tooltip.is_empty()),
        items,
    }))
}

/// A menu's lines: `"-"`, or `{ label, msg, to, enabled, checked, shortcut }`.
fn entries(items: &Data) -> Result<Vec<StatusEntry>, String> {
    let Data::List(items) = items else {
        return Err("a menu is a list of items and \"-\"".to_owned());
    };
    if items.len() > MAX_ITEMS {
        return Err(format!("at most {MAX_ITEMS} items in a menu"));
    }
    items
        .iter()
        .enumerate()
        .map(|(at, item)| entry(item).map_err(|problem| format!("item {}: {problem}", at + 1)))
        .collect()
}

fn entry(item: &Data) -> Result<StatusEntry, String> {
    let fields = match item {
        Data::String(line) if line == "-" => return Ok(StatusEntry::Separator),
        Data::Map(fields) => fields,
        _ => return Err("an item is \"-\" or { label, msg }".to_owned()),
    };
    if let Some(key) = fields.keys().find(|key| {
        !matches!(
            key.as_str(),
            "label" | "msg" | "to" | "enabled" | "checked" | "shortcut"
        )
    }) {
        return Err(format!(
            "unknown key `{key}`; an item takes label, msg, to, enabled, checked and shortcut"
        ));
    }
    let label = match fields.get("label") {
        Some(Data::String(label))
            if !label.trim().is_empty() && label.len() <= MAX_LABEL && !label.contains('\n') =>
        {
            label.clone()
        },
        _ => {
            return Err(format!("`label` is one line of 1 to {MAX_LABEL} bytes"));
        },
    };
    let to = match fields.get("to") {
        None => None,
        Some(Data::String(to)) if to == SHELL || crate::entries::module_name(to) => {
            Some(to.clone())
        },
        Some(_) => return Err("`to` is a mod's name, or \"shell\"".to_owned()),
    };
    let flag = |key: &str, default: bool| match fields.get(key) {
        None => Ok(default),
        Some(Data::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("`{key}` is true or false")),
    };
    let shortcut = match fields.get("shortcut") {
        None => None,
        Some(Data::String(name)) => {
            Some(Chord::parse(name).map_err(|problem| format!("`shortcut`: {problem}"))?)
        },
        Some(_) => return Err("`shortcut` is a key such as \"ctrl-shift-space\"".to_owned()),
    };
    let msg = fields.get("msg").filter(|msg| **msg != Data::Nil).cloned();
    if to.is_some() && msg.is_none() {
        return Err("`to` without a `msg` to send".to_owned());
    }
    Ok(StatusEntry::Item {
        label,
        msg,
        to,
        enabled: flag("enabled", true)?,
        checked: flag("checked", false)?,
        shortcut,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, std::collections::BTreeMap};

    fn map(fields: &[(&str, Data)]) -> Data {
        Data::Map(
            fields
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    fn item(fields: &[(&str, Data)]) -> Data {
        map(fields)
    }

    #[test]
    fn an_item_reads_its_look_and_menu() {
        let request = status_item(&map(&[
            ("symbol", Data::from("text.viewfinder")),
            ("title", Data::from("✓")),
            ("tooltip", Data::from("Capture")),
            (
                "items",
                Data::List(vec![
                    item(&[("label", Data::from("Branch: main"))]),
                    Data::from("-"),
                    item(&[
                        ("label", Data::from("Capture Selection")),
                        ("msg", Data::from("capture")),
                        ("shortcut", Data::from("ctrl-shift-space")),
                    ]),
                    item(&[
                        ("label", Data::from("Open")),
                        ("msg", Data::from("open")),
                        ("to", Data::from("knowledge")),
                        ("checked", Data::Bool(true)),
                        ("enabled", Data::Bool(false)),
                    ]),
                ]),
            ),
        ]))
        .unwrap();
        let StatusItem::Show(look) = request else {
            panic!("{request:?}");
        };
        assert_eq!(
            look.icon,
            Some(StatusIcon::Symbol("text.viewfinder".to_owned()))
        );
        assert!(look.template);
        assert_eq!(look.title.as_deref(), Some("✓"));
        assert_eq!(look.items.len(), 4);
        assert_eq!(look.items[1], StatusEntry::Separator);
        let StatusEntry::Item {
            msg, shortcut, to, ..
        } = &look.items[2]
        else {
            panic!()
        };
        assert_eq!(msg, &Some(Data::from("capture")));
        assert!(shortcut.as_ref().unwrap().ctrl);
        assert_eq!(to, &None);
        let StatusEntry::Item {
            enabled, checked, ..
        } = &look.items[3]
        else {
            panic!()
        };
        assert!(!enabled && *checked);
    }

    #[test]
    fn it_can_be_taken_away_or_pop_a_menu() {
        assert_eq!(status_item(&Data::Bool(false)).unwrap(), StatusItem::Remove);
        let popup = status_item(&map(&[(
            "popup",
            Data::List(vec![item(&[
                ("label", Data::from("Capture Area…")),
                ("msg", Data::from("area")),
            ])]),
        )]))
        .unwrap();
        assert!(matches!(popup, StatusItem::Popup(items) if items.len() == 1));
    }

    #[test]
    fn what_it_cannot_show_is_the_senders_error() {
        for (bad, says) in [
            (map(&[]), "needs a `symbol`"),
            (map(&[("title", Data::from(""))]), "needs a `symbol`"),
            (
                map(&[("symbol", Data::from("a")), ("image", Data::from("a.png"))]),
                "not both",
            ),
            (map(&[("symbol", Data::from("Bad Name"))]), "system symbol"),
            (
                map(&[("image", Data::from("https://x/y.png"))]),
                "image file",
            ),
            (map(&[("title", Data::from("x".repeat(65)))]), "64 bytes"),
            (map(&[("colour", Data::from("red"))]), "unknown key"),
            (
                map(&[("title", Data::from("x")), ("items", Data::from("-"))]),
                "a list",
            ),
            (
                map(&[
                    ("title", Data::from("x")),
                    ("items", Data::List(vec![item(&[("msg", Data::from("x"))])])),
                ]),
                "item 1: `label`",
            ),
            (
                map(&[
                    ("title", Data::from("x")),
                    (
                        "items",
                        Data::List(vec![item(&[
                            ("label", Data::from("x")),
                            ("msg", Data::from("x")),
                            ("shortcut", Data::from("k")),
                        ])]),
                    ),
                ]),
                "types text",
            ),
            (
                map(&[
                    ("title", Data::from("x")),
                    (
                        "items",
                        Data::List(vec![item(&[
                            ("label", Data::from("x")),
                            ("to", Data::from("other")),
                        ])]),
                    ),
                ]),
                "without a `msg`",
            ),
            (
                map(&[("popup", Data::List(vec![])), ("title", Data::from("x"))]),
                "alone",
            ),
            (map(&[("popup", Data::List(vec![]))]), "at least one"),
            (
                map(&[
                    ("title", Data::from("x")),
                    ("items", Data::List(vec![Data::from("-"); MAX_ITEMS + 1])),
                ]),
                "at most 64",
            ),
            (Data::Bool(true), "takes"),
        ] {
            let problem = status_item(&bad).unwrap_err().to_string();
            assert!(problem.contains(says), "{bad:?}: {problem}");
        }
    }
}
