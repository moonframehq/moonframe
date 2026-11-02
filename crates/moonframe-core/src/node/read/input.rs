//! Text elements: inputs (with their keys and ids) and markdown.

use {super::*, crate::read::Document};

impl Build for Input {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "id",
        "value",
        "placeholder",
        "to",
        "change",
        "submit",
        "blur",
        "keys",
        "multiline",
        "rows",
        "max_rows",
        "disabled",
        "focus",
        "mono",
        "width",
        "grow",
        "browser",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let multiline = fields.get("multiline")?.unwrap_or(false);
        let rows = fields.get::<f32>("rows")?;
        if rows.is_some() && !multiline {
            return Err(fields
                .cx()
                .key("rows")
                .fail("`rows` goes with `multiline = true`"));
        }
        let rows = match rows {
            None => Self::DEFAULT_ROWS,
            Some(rows) if rows.fract() == 0.0 && (1.0..=Self::MAX_ROWS as f32).contains(&rows) => {
                rows as u32
            },
            Some(_) => {
                return Err(fields
                    .cx()
                    .key("rows")
                    .fail(format!("a whole number from 1 to {}", Self::MAX_ROWS)));
            },
        };
        let max_rows = match fields.get::<f32>("max_rows")? {
            None => None,
            Some(_) if !multiline => {
                return Err(fields
                    .cx()
                    .key("max_rows")
                    .fail("`max_rows` goes with `multiline = true`"));
            },
            Some(most)
                if most.fract() == 0.0 && (rows as f32..=Self::MAX_ROWS as f32).contains(&most) =>
            {
                Some(most as u32)
            },
            Some(_) => {
                return Err(fields.cx().key("max_rows").fail(format!(
                    "a whole number from `rows` ({rows}) to {}",
                    Self::MAX_ROWS
                )));
            },
        };
        let change = fields.get::<Data>("change")?;
        let submit = fields.get::<Data>("submit")?;
        let blur = fields.get::<Data>("blur")?;
        let keys = fields
            .get::<InputKeys>("keys")?
            .map(|keys| keys.0)
            .unwrap_or_default();
        let to = fields.get::<String>("to")?;
        let browser = fields.get::<BrowserId>("browser")?.map(|id| id.0);
        if to.is_some()
            && change.is_none()
            && submit.is_none()
            && blur.is_none()
            && keys.is_empty()
            && browser.is_none()
        {
            return Err(fields
                .cx()
                .key("to")
                .fail("`to` goes with `change`, `submit`, `blur`, `keys` or `browser`"));
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields.get::<InputId>("id")?.map(|id| id.0),
            // A text area may hold a whole document.
            value: if multiline {
                fields.get::<Document>("value")?.map(|text| text.0)
            } else {
                fields.get("value")?
            },
            placeholder: fields.get("placeholder")?,
            to,
            change,
            submit,
            blur,
            keys,
            multiline,
            rows,
            max_rows,
            disabled: fields.get("disabled")?.unwrap_or(false),
            focus: fields.get("focus")?.unwrap_or(false),
            mono: fields.get("mono")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            grow: fields.get("grow")?.unwrap_or(false),
            browser,
        })
    }
}

impl Build for Markdown {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "text",
        "link",
        "to",
        "grow",
        "scroll",
        "width",
        "height",
        "max_width",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let link = fields.get::<Data>("link")?;
        let to = fields.get::<String>("to")?;
        if to.is_some() && link.is_none() {
            return Err(fields.cx().key("to").fail("`to` goes with `link`"));
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            text: fields
                .get::<Document>("text")?
                .map(|text| text.0)
                .unwrap_or_default(),
            link,
            to,
            grow: fields.get("grow")?.unwrap_or(false),
            scroll: fields.get("scroll")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            max_width: fields.number("max_width", PIXELS)?,
        })
    }
}

/// An input's `keys`: a table from key names to the messages they send.
struct InputKeys(Vec<(KeyChord, Data)>);

/// Most keys one input can take over.
const MAX_KEYS: usize = 32;

impl Read for InputKeys {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let Value::Table(table) = value else {
            return Err(
                cx.fail("expected a table such as { up = \"previous\", escape = \"close\" }")
            );
        };
        let mut keys: Vec<(String, KeyChord, Data)> = Vec::new();
        for pair in table.pairs::<Value, Value>() {
            let (name, msg) = pair?;
            let Value::String(name) = name else {
                return Err(cx.fail("keys are key names such as `up` or `ctrl-n`"));
            };
            let name = name.to_string_lossy();
            let cx = cx.quoted(&name);
            let chord = KeyChord::parse(&name).map_err(|problem| cx.fail(problem))?;
            if let Some((other, ..)) = keys.iter().find(|(_, other, _)| *other == chord) {
                return Err(cx.fail(format!("the same key as `{other}`")));
            }
            keys.push((name, chord, <Data as Read>::read(&msg, &cx)?));
            if keys.len() > MAX_KEYS {
                return Err(cx.fail(format!("more than {MAX_KEYS} keys")));
            }
        }
        // Table order is not the script's; sort so a view reads the same each time.
        keys.sort_by(|(a, ..), (b, ..)| a.cmp(b));
        Ok(Self(
            keys.into_iter()
                .map(|(_, chord, msg)| (chord, msg))
                .collect(),
        ))
    }
}

/// An input's `id`, kept by the mod like a terminal's.
struct InputId(String);

impl Read for InputId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        plain_id(value, cx).map(Self)
    }
}
