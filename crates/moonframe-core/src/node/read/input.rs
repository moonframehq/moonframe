//! Text elements: inputs (with their keys and ids) and markdown.

use {super::*, crate::read::Document};

impl Build for Input {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "id",
        "value",
        "placeholder",
        "to",
        "change",
        "select",
        "submit",
        "blur",
        "keys",
        "capture",
        "multiline",
        "rows",
        "max_rows",
        "disabled",
        "focus",
        "mono",
        "width",
        "grow",
        "browser",
        "prose",
        "dim",
        "typewriter",
        "code",
        "line",
        "scrolled",
        "scroll_at",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let code = code_language(fields)?;
        let multiline = fields.get("multiline")?.unwrap_or(false);
        let prose = fields.get("prose")?.unwrap_or(false);
        let dim = fields.get::<Dim>("dim")?;
        let typewriter = fields.get("typewriter")?.unwrap_or(false);
        for (key, set) in [
            ("prose", prose),
            ("dim", dim.is_some()),
            ("typewriter", typewriter),
            ("code", code.is_some()),
        ] {
            if set && !multiline {
                return Err(fields
                    .cx()
                    .key(key)
                    .fail(format!("`{key}` goes with `multiline = true`")));
            }
        }
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
        let select = fields.get::<Data>("select")?;
        let submit = fields.get::<Data>("submit")?;
        let blur = fields.get::<Data>("blur")?;
        let capture = fields.get::<Data>("capture")?;
        let keys = fields
            .get::<InputKeys>("keys")?
            .map(|keys| keys.0)
            .unwrap_or_default();
        let to = fields.get::<String>("to")?;
        let browser = fields.get::<BrowserId>("browser")?.map(|id| id.0);
        let (scrolled, scroll_at) = scrolling(fields, multiline, "multiline = true")?;
        if to.is_some()
            && scrolled.is_none()
            && change.is_none()
            && select.is_none()
            && submit.is_none()
            && blur.is_none()
            && keys.is_empty()
            && capture.is_none()
            && browser.is_none()
        {
            return Err(fields.cx().key("to").fail(
                "`to` goes with `change`, `select`, `submit`, `blur`, `keys`, `scrolled` or \
                     `browser`",
            ));
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
            select,
            submit,
            blur,
            keys,
            capture,
            multiline,
            rows,
            max_rows,
            disabled: fields.get("disabled")?.unwrap_or(false),
            focus: fields.get("focus")?.unwrap_or(false),
            mono: fields.get("mono")?.unwrap_or(false),
            width: fields.number("width", PIXELS)?,
            grow: fields.get("grow")?.unwrap_or(false),
            browser,
            prose,
            dim,
            typewriter,
            line: caret_line(fields, code.is_some())?,
            code,
            scrolled,
            scroll_at,
        })
    }
}

/// `line`, a whole number from 1, for a code editor only.
fn caret_line(fields: &Fields, code: bool) -> Result<Option<u32>, Error> {
    let Some(line) = fields.get::<f32>("line")? else {
        return Ok(None);
    };
    if !code {
        return Err(fields.cx().key("line").fail("`line` goes with `code`"));
    }
    if line.fract() != 0.0 || !(1.0..=u32::MAX as f32).contains(&line) {
        return Err(fields.cx().key("line").fail("a line number, from 1"));
    }
    Ok(Some(line as u32))
}

/// Longest language name `code` takes.
const MAX_LANGUAGE: usize = 32;

/// `code`'s language, lowercase: a name, never a path. Luau is colored as
/// Lua, which it extends.
fn code_language(fields: &Fields) -> Result<Option<String>, Error> {
    let Some(name) = fields.get::<String>("code")? else {
        return Ok(None);
    };
    let name = name.to_ascii_lowercase();
    let plain = !name.is_empty()
        && name.len() <= MAX_LANGUAGE
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'+'));
    if !plain {
        return Err(fields.cx().key("code").fail(format!(
            "a language's name, such as \"luau\" or \"rust\" (at most {MAX_LANGUAGE} letters)"
        )));
    }
    if fields.get("prose")?.unwrap_or(false) {
        return Err(fields
            .cx()
            .key("code")
            .fail("`code` and `prose` are two kinds of field: choose one"));
    }
    Ok(Some(name))
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
        "lines",
        "images",
        "image",
        "outline",
        "section",
        "reveal",
        "find",
        "find_index",
        "found",
        "stream",
        "scrolled",
        "scroll_at",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let link = fields.get::<Data>("link")?;
        let to = fields.get::<String>("to")?;
        let image = fields.get::<Data>("image")?;
        let outline = fields.get::<Data>("outline")?;
        let section = fields.get::<Data>("section")?;
        let found = fields.get::<Data>("found")?;
        let scroll = fields.get("scroll")?.unwrap_or(false);
        let (scrolled, scroll_at) = scrolling(fields, scroll, "scroll = true")?;
        let sends = [&link, &image, &outline, &section, &found, &scrolled]
            .iter()
            .any(|msg| msg.is_some());
        if to.is_some() && !sends {
            return Err(fields.cx().key("to").fail("`to` goes with `link`"));
        }
        let lines = fields.get::<f32>("lines")?;
        let lines = match lines {
            None => None,
            Some(_) if scroll => {
                return Err(fields.cx().key("lines").fail("`lines` is not for `scroll`"));
            },
            Some(lines) if lines.fract() == 0.0 && (1.0..=MAX_LINES).contains(&lines) => {
                Some(lines as usize)
            },
            Some(_) => {
                return Err(fields
                    .cx()
                    .key("lines")
                    .fail("`lines` is a whole number of lines, from 1"));
            },
        };
        let find_index = match fields.get::<f32>("find_index")? {
            None => None,
            Some(index) if index.fract() == 0.0 && index >= 1.0 => Some(index as usize),
            Some(_) => {
                return Err(fields
                    .cx()
                    .key("find_index")
                    .fail("`find_index` is a match's number, from 1"));
            },
        };
        Ok(Self {
            look: Look::from_fields(fields)?,
            text: fields
                .get::<Document>("text")?
                .map(|text| text.0)
                .unwrap_or_default(),
            link,
            to,
            grow: fields.get("grow")?.unwrap_or(false),
            scroll,
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            max_width: fields.number("max_width", PIXELS)?,
            lines,
            images: fields.get("images")?.unwrap_or(false),
            image,
            outline,
            section,
            reveal: fields.get("reveal")?,
            find: fields.get("find")?,
            find_index,
            found,
            stream: fields.get("stream")?.unwrap_or(false),
            scrolled,
            scroll_at,
        })
    }
}

/// Most lines a `ui.markdown` can be cut to.
const MAX_LINES: f32 = 1000.0;

impl Read for Reveal {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&["at", "seq"]])?;
        let Some(at) = fields.number("at", 0.0..=f32::MAX)? else {
            return Err(cx
                .key("at")
                .fail("`reveal` needs `at`, bytes into the text"));
        };
        Ok(Self {
            at: at as usize,
            seq: fields.get::<f32>("seq")?.unwrap_or(0.0) as i64,
        })
    }
}

/// `scrolled` and `scroll_at`, which go with an element that scrolls its
/// own text: `scrolls` says whether this one does, `needs` what makes it.
fn scrolling(
    fields: &Fields,
    scrolls: bool,
    needs: &str,
) -> Result<(Option<Data>, Option<ScrollAt>), Error> {
    let scrolled = fields.get::<Data>("scrolled")?;
    let scroll_at = fields.get::<ScrollAt>("scroll_at")?;
    if !scrolls {
        for (key, set) in [
            ("scrolled", scrolled.is_some()),
            ("scroll_at", scroll_at.is_some()),
        ] {
            if set {
                return Err(fields
                    .cx()
                    .key(key)
                    .fail(format!("`{key}` goes with `{needs}`")));
            }
        }
    }
    Ok((scrolled, scroll_at))
}

impl Read for ScrollAt {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&["fraction", "seq"]])?;
        let Some(fraction) = fields.number("fraction", 0.0..=1.0)? else {
            return Err(cx
                .key("fraction")
                .fail("`scroll_at` needs `fraction`, from 0 (the top) to 1 (the end)"));
        };
        Ok(Self {
            fraction,
            // As a double: an f32 runs out of whole numbers past 2^24.
            seq: fields.get::<f64>("seq")?.unwrap_or(0.0) as i64,
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

impl Read for Dim {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        match String::read(value, cx)?.as_str() {
            "sentence" => Ok(Self::Sentence),
            "paragraph" => Ok(Self::Paragraph),
            other => Err(cx.fail(format!(
                "`{other}` is no dim; use \"sentence\" or \"paragraph\""
            ))),
        }
    }
}

/// An input's `id`, kept by the mod like a terminal's.
struct InputId(String);

impl Read for InputId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        plain_id(value, cx).map(Self)
    }
}
