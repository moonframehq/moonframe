//! Elements the host keeps by an id: terminals, browsers and registered
//! views, and the ids and URLs they take.

use super::*;

impl Build for Terminal {
    const KEYS: &'static [&'static [&'static str]] = &[
        TYPE,
        LOOK,
        &["id", "cwd", "engine", "backend", "exit", "shell"],
    ];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields
                .get::<TerminalId>("id")?
                .map_or_else(|| "terminal".to_owned(), |id| id.0),
            cwd: fields.get("cwd")?,
            engine: fields.get("engine")?.unwrap_or(TerminalEngine::Alacritty),
            backend: fields.get("backend")?.unwrap_or(TerminalBackend::Auto),
            exit: fields.get("exit")?,
            shell: fields.get("shell")?,
        })
    }
}

/// A terminal's `id`: one shell per mod and id, so it is short and plain.
struct TerminalId(String);

/// Longest a terminal or browser `id` can be.
const MAX_TERMINAL_ID: usize = 64;

impl Read for TerminalId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        plain_id(value, cx).map(Self)
    }
}

/// An id the host keeps something by: 1 to 64 of `a-z A-Z 0-9 - _`.
pub(super) fn plain_id(value: &Value, cx: &Cx) -> Result<String, Error> {
    let id = String::read(value, cx)?;
    let plain = !id.is_empty()
        && id.len() <= MAX_TERMINAL_ID
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if plain {
        Ok(id)
    } else {
        Err(cx.fail(format!(
            "an id is 1 to {MAX_TERMINAL_ID} of a-z, A-Z, 0-9, - and _"
        )))
    }
}

impl Build for Browser {
    const KEYS: &'static [&'static [&'static str]] =
        &[TYPE, LOOK, &["id", "url", "zoom", "navigate", "loading"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields
                .get::<BrowserId>("id")?
                .map_or_else(|| "browser".to_owned(), |id| id.0),
            url: fields.get::<WebUrl>("url")?.map(|url| url.0),
            zoom: fields.number("zoom", 0.25..=5.0)?.unwrap_or(1.0),
            navigate: fields.get("navigate")?,
            loading: fields.get("loading")?,
        })
    }
}

impl Build for View {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &["name", "id", "props"]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let name = fields.get::<ViewName>("name")?.ok_or_else(|| {
            fields
                .cx()
                .key("name")
                .fail("a view needs the `name` the app registered")
        })?;
        Ok(Self {
            look: Look::from_fields(fields)?,
            name: name.0,
            id: fields
                .get::<BrowserId>("id")?
                .map_or_else(|| "view".to_owned(), |id| id.0),
            props: fields.get("props")?.unwrap_or(Data::Nil),
        })
    }
}

/// The name an app registered a view by: `herdr.workspace`, `chart`.
struct ViewName(String);

impl Read for ViewName {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let name = String::read(value, cx)?;
        let plain = !name.is_empty()
            && name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if plain {
            Ok(Self(name))
        } else {
            Err(cx.fail("a view's name is 1 to 64 of a-z, A-Z, 0-9, -, _ and ."))
        }
    }
}

/// A browser's `id`, kept by the mod like a terminal's.
pub(super) struct BrowserId(pub(super) String);

impl Read for BrowserId {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        plain_id(value, cx).map(Self)
    }
}

/// A browser's `url`: only web and file addresses, never a script or an
/// app's own scheme.
struct WebUrl(String);

impl Read for WebUrl {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let url = String::read(value, cx)?;
        if is_web_url(&url) {
            Ok(Self(url))
        } else {
            Err(cx.fail(format!(
                "`{}` is not an http, https or file address",
                url.chars().take(80).collect::<String>()
            )))
        }
    }
}
