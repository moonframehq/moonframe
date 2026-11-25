//! `app:text`: the text a mod draws, and the window's most recent text
//! selection, back to the mod that asked, as its `reply` with `text` and
//! `selection` added. An assistant sends them with a question: what the
//! page showed, and what the user last highlighted on it, kept after they
//! click into the field they ask in.
//!
//! `{ mod = "notes", reply = "page" }` reads what `notes` draws, with what
//! it mounts; `{ region = "main", reply = "page" }` what a region's mods
//! draw; `{ reply = "page" }` only the selection. `max` caps each, in
//! characters.

use {
    super::APP,
    crate::{Data, Error, with_fields},
    std::collections::BTreeMap,
};

/// What `max` is when unset, and the most it can be, in characters.
const DEFAULT_MAX: usize = 64_000;
const MOST: usize = 1024 * 1024;

/// What `app:text` asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRequest {
    /// Whose drawn text to read, if any.
    pub of: Option<TextOf>,
    /// The most characters of the text, and of the selection, sent.
    pub max: usize,
    /// What comes back, with `text` and `selection` added.
    pub reply: Data,
}

/// Where the text is read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextOf {
    /// A mod, or an instance by its key, with what it mounts.
    Mod(String),
    /// The mods that draw in a region, in order.
    Region(String),
}

/// A selection the user made: its text, and the mod that drew it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selected {
    pub text: String,
    pub owner: String,
}

impl TextRequest {
    /// The answer: `text` when it was asked for and found, and `selection`
    /// (`{ text, mod }`) when the user has selected something, each cut to
    /// `max` characters.
    pub fn answer(&self, text: Option<String>, selection: Option<Selected>) -> Data {
        let mut fields = BTreeMap::new();
        if let Some(text) = text {
            fields.insert("text".to_owned(), Data::String(cut(text, self.max)));
        }
        if let Some(selected) = selection {
            fields.insert(
                "selection".to_owned(),
                Data::map([
                    ("text", Data::String(cut(selected.text, self.max))),
                    ("mod", Data::String(selected.owner)),
                ]),
            );
        }
        with_fields(&self.reply, fields)
    }
}

/// `text` with at most `max` characters.
fn cut(mut text: String, max: usize) -> String {
    if let Some((at, _)) = text.char_indices().nth(max) {
        text.truncate(at);
    }
    text
}

/// `{ mod = "<name>" | region = "<name>", max = n, reply = <msg> }`.
pub(super) fn text(msg: &Data) -> Result<TextRequest, Error> {
    let fail = |problem: &str| Error::new(format!("`{APP}text`: {problem}"));
    let usage =
        "takes { mod = \"<name>\" or region = \"<name>\", max = <characters>, reply = <msg> }";
    let Data::Map(fields) = msg else {
        return Err(fail(usage));
    };
    if let Some(key) = fields
        .keys()
        .find(|key| !matches!(key.as_str(), "mod" | "region" | "max" | "reply"))
    {
        return Err(fail(&format!(
            "unknown key `{key}`; it takes mod, region, max and reply"
        )));
    }
    let name = |key: &str| match fields.get(key) {
        None => Ok(None),
        Some(Data::String(name)) if !name.is_empty() && name.len() <= 256 => Ok(Some(name.clone())),
        Some(_) => Err(fail(&format!("`{key}` is a name"))),
    };
    let of = match (name("mod")?, name("region")?) {
        (Some(_), Some(_)) => return Err(fail("takes `mod` or `region`, not both")),
        (Some(name), None) => Some(TextOf::Mod(name)),
        (None, Some(name)) => Some(TextOf::Region(name)),
        (None, None) => None,
    };
    let max = match fields.get("max") {
        None => DEFAULT_MAX,
        Some(Data::Integer(max)) if (1..=MOST as i64).contains(max) => *max as usize,
        Some(_) => {
            return Err(fail(&format!(
                "`max` is a number of characters, 1 to {MOST}"
            )));
        },
    };
    let Some(reply) = fields.get("reply").cloned() else {
        return Err(fail(&format!("needs `reply`, what comes back; {usage}")));
    };
    Ok(TextRequest { of, max, reply })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_request_names_a_mod_or_a_region_and_a_reply() {
        let read = |msg: Data| text(&msg);
        let asked = read(Data::map([
            ("mod", Data::from("notes")),
            ("reply", Data::from("page")),
        ]));
        assert_eq!(
            asked.ok().and_then(|asked| asked.of),
            Some(TextOf::Mod("notes".to_owned()))
        );
        let region = read(Data::map([
            ("region", Data::from("main")),
            ("max", Data::Integer(10)),
            ("reply", Data::from("page")),
        ]));
        assert!(matches!(
            region,
            Ok(TextRequest {
                of: Some(TextOf::Region(_)),
                max: 10,
                ..
            })
        ));
        for bad in [
            Data::map([
                ("reply", Data::from("page")),
                ("mod", Data::from("a")),
                ("region", Data::from("b")),
            ]),
            Data::map([("mod", Data::from("a"))]),
            Data::map([("reply", Data::from("page")), ("max", Data::Integer(0))]),
            Data::map([("reply", Data::from("page")), ("page", Data::Bool(true))]),
        ] {
            assert!(read(bad).is_err());
        }
    }

    #[test]
    fn the_answer_cuts_the_text_and_the_selection_to_max() {
        let asked = TextRequest {
            of: None,
            max: 3,
            reply: Data::from("page"),
        };
        let answer = asked.answer(
            Some("héllo".to_owned()),
            Some(Selected {
                text: "wörld".to_owned(),
                owner: "notes".to_owned(),
            }),
        );
        let Data::Map(answer) = answer else {
            panic!("a table");
        };
        assert_eq!(answer.get("msg"), Some(&Data::from("page")));
        assert_eq!(answer.get("text"), Some(&Data::from("hél")));
        assert_eq!(
            answer.get("selection"),
            Some(&Data::map([
                ("text", Data::from("wör")),
                ("mod", Data::from("notes"))
            ]))
        );
        let Data::Map(bare) = asked.answer(None, None) else {
            panic!("a table");
        };
        assert!(!bare.contains_key("text") && !bare.contains_key("selection"));
    }
}
