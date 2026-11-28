//! `ui.document`'s keys: its HTML or file, the messages it sends, and
//! where to scroll it.

use super::*;

/// The longest `id` `scroll_to` names, and the longest `src`.
const MAX_ID: usize = 256;
const MAX_SRC: usize = 4096;
/// How far down a document may be asked to scroll.
const Y: std::ops::RangeInclusive<f32> = 0.0..=1.0e9;

impl Build for Document {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "id",
        "html",
        "src",
        "link",
        "scroll",
        "scroll_to",
        "select",
        "editable",
        "change",
        "done",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let source = match (fields.get::<Html>("html")?, fields.get::<Src>("src")?) {
            (Some(html), None) => DocumentSource::Html(html.0),
            (None, Some(src)) => DocumentSource::File(src.0),
            (Some(_), Some(_)) => {
                return Err(fields
                    .cx()
                    .key("src")
                    .fail("give `html` or `src`, not both"));
            },
            (None, None) => {
                return Err(fields
                    .cx()
                    .fail("a document shows its `html`, or the HTML file `src` names"));
            },
        };
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields
                .get::<BrowserId>("id")?
                .map_or_else(|| "document".to_owned(), |id| id.0),
            source,
            link: fields.get("link")?,
            scroll: fields.get("scroll")?,
            scroll_to: fields.get("scroll_to")?,
            select: fields.get("select")?,
            editable: fields.get("editable")?.unwrap_or(false),
            change: fields.get("change")?,
            done: fields.get("done")?,
        })
    }
}

/// A document's HTML, up to [`MAX_HTML`] bytes.
struct Html(String);

impl Read for Html {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let Value::String(text) = value else {
            return Err(cx.fail("expected the HTML, a string"));
        };
        if text.as_bytes().len() > MAX_HTML {
            return Err(cx.fail(format!("longer than {MAX_HTML} bytes")));
        }
        Ok(Self(text.to_string_lossy()))
    }
}

/// A file's name, as `ui.image` takes one.
struct Src(String);

impl Read for Src {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let src = String::read(value, cx)?;
        if src.is_empty() || src.len() > MAX_SRC || src.chars().any(char::is_control) {
            return Err(cx.fail(format!("a file's name, 1 to {MAX_SRC} bytes")));
        }
        Ok(Self(src))
    }
}

/// `scroll_to = 480` (points down), `"part-2"` (an element's `id`), or
/// `{ y = 480, seq = n }` and `{ id = "part-2", seq = n }`.
impl Read for DocumentScrollTo {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let id = |value: &Value, cx: &Cx| {
            let id = String::read(value, cx)?;
            if id.is_empty() || id.len() > MAX_ID || id.chars().any(char::is_control) {
                return Err(cx.fail(format!("an element's id, 1 to {MAX_ID} bytes")));
            }
            Ok(DocumentScroll::Id(id))
        };
        let y = |value: &Value, cx: &Cx| {
            let y = f32::read(value, cx)?;
            if Y.contains(&y) {
                Ok(DocumentScroll::Y(y))
            } else {
                Err(cx.fail("expected points down, from 0"))
            }
        };
        let to = match value {
            Value::Integer(_) | Value::Number(_) => y(value, cx)?,
            Value::String(_) => id(value, cx)?,
            Value::Table(_) => {
                let fields = Fields::read(value, cx, &[&["y", "id", "seq"]])?;
                let to = match (fields.raw("y"), fields.raw("id")) {
                    (Some(value), None) => y(value, &cx.key("y"))?,
                    (None, Some(value)) => id(value, &cx.key("id"))?,
                    (Some(_), Some(_)) => return Err(cx.fail("give `y` or `id`, not both")),
                    (None, None) => {
                        return Err(cx.fail("`scroll_to` takes `y` (points) or `id`"));
                    },
                };
                return Ok(Self {
                    to,
                    seq: fields.get("seq")?.unwrap_or(Data::Nil),
                });
            },
            _ => return Err(cx.fail("expected points, an id, or `{ y | id, seq }`")),
        };
        Ok(Self { to, seq: Data::Nil })
    }
}
