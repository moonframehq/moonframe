//! `app:pick`: the system's open, folder or save panel. The answer comes
//! back to the mod that asked, as its `reply` with `paths` (or `path` for
//! a save) added, or `cancelled = true`. What the user picks they chose to
//! share with that mod, so a picked file (or a folder's pictures) may show
//! in its `ui.image`, and its components', from then on.

use {
    super::APP,
    crate::{Data, Error, with_fields},
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
    },
};

/// Most file types one panel filters on.
const MAX_TYPES: usize = 32;
/// Longest a label, a file name or a folder can be.
const MAX_TEXT: usize = 1024;

/// What `app:pick` asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    pub kind: PickKind,
    /// Allow more than one file or folder.
    pub multiple: bool,
    /// The panel's button or title, such as "Open scan".
    pub prompt: Option<String>,
    /// File extensions to keep, lowercase, without the dot; all when empty.
    pub types: Vec<String>,
    /// A save panel's suggested file name.
    pub name: Option<String>,
    /// The folder a save panel starts in, `~/...` or absolute.
    pub folder: Option<String>,
    /// What comes back, with the answer's fields added.
    pub reply: Data,
}

tokens! {
    /// Which panel `app:pick` opens.
    PickKind {
        /// Files to open.
        Open = "open",
        /// Folders.
        Folder = "folder",
        /// A file to save, which need not exist yet.
        Save = "save",
    }
}

impl Pick {
    /// The answer to send back: the picked paths, those `types` keeps.
    pub fn answer(&self, picked: Option<Vec<PathBuf>>) -> Data {
        let kept: Vec<PathBuf> = picked
            .unwrap_or_default()
            .into_iter()
            .filter(|path| self.keeps(path))
            .collect();
        let mut fields = BTreeMap::new();
        if kept.is_empty() {
            fields.insert("cancelled".to_owned(), Data::Bool(true));
        } else if self.kind == PickKind::Save {
            fields.insert("path".to_owned(), text(&kept[0]));
        } else {
            fields.insert(
                "paths".to_owned(),
                Data::List(kept.iter().map(|path| text(path)).collect()),
            );
        }
        with_fields(&self.reply, fields)
    }

    /// Whether `types` lets `path` through: folders and saves always.
    fn keeps(&self, path: &Path) -> bool {
        if self.kind != PickKind::Open || self.types.is_empty() {
            return true;
        }
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| self.types.contains(&extension.to_lowercase()))
    }
}

fn text(path: &Path) -> Data {
    Data::String(path.to_string_lossy().into_owned())
}

/// `{ kind = "open", multiple = true, prompt = "Open", types = { "tif" },
/// name = "scan.tif", folder = "~/Pictures", reply = "opened" }`.
pub(super) fn pick(msg: &Data) -> Result<Pick, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}pick`: {problem}"));
    let Data::Map(fields) = msg else {
        return Err(fail(
            "takes { kind = \"open\" | \"folder\" | \"save\", reply = <msg>, ... }",
        ));
    };
    const KEYS: &[&str] = &[
        "kind", "multiple", "prompt", "types", "name", "folder", "reply",
    ];
    if let Some(key) = fields.keys().find(|key| !KEYS.contains(&key.as_str())) {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes {}",
            KEYS.join(", ")
        )));
    }
    let string = |key: &str| match fields.get(key) {
        None => Ok(None),
        Some(Data::String(text)) if text.len() <= MAX_TEXT => Ok(Some(text.clone())),
        Some(_) => Err(fail(&format!(
            "`{key}` is a string of at most {MAX_TEXT} bytes"
        ))),
    };
    let kind = match string("kind")? {
        None => PickKind::Open,
        Some(name) => PickKind::parse(&name).ok_or_else(|| {
            fail(&format!(
                "unknown kind `{name}`; expected one of {}",
                PickKind::NAMES.join(", ")
            ))
        })?,
    };
    let multiple = match fields.get("multiple") {
        None => false,
        Some(Data::Bool(multiple)) => *multiple,
        Some(_) => return Err(fail("`multiple` is true or false")),
    };
    let types = match fields.get("types") {
        None => Vec::new(),
        Some(Data::List(types)) if types.len() <= MAX_TYPES => types
            .iter()
            .map(|kind| match kind {
                Data::String(kind) if !kind.is_empty() && kind.len() <= 16 => {
                    Ok(kind.trim_start_matches('.').to_lowercase())
                },
                _ => Err(fail("`types` lists extensions such as \"tif\"")),
            })
            .collect::<Result<_, _>>()?,
        Some(_) => {
            return Err(fail(&format!(
                "`types` is a list of at most {MAX_TYPES} extensions"
            )));
        },
    };
    if multiple && kind == PickKind::Save {
        return Err(fail("a save panel picks one file; drop `multiple`"));
    }
    if kind != PickKind::Open && !types.is_empty() {
        return Err(fail("`types` filters files to open"));
    }
    if kind != PickKind::Save && (fields.contains_key("name") || fields.contains_key("folder")) {
        return Err(fail("`name` and `folder` go with kind = \"save\""));
    }
    Ok(Pick {
        kind,
        multiple,
        prompt: string("prompt")?,
        types,
        name: string("name")?,
        folder: string("folder")?,
        reply: fields
            .get("reply")
            .cloned()
            .ok_or_else(|| fail("needs a `reply`, the message the answer comes back in"))?,
    })
}
