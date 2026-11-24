//! `moonframe.page`, the capability that renders a web page for a provider:
//! `render` loads it in a hidden web view, lets its scripts run, and
//! answers with the text a reader sees and, when asked, a picture of it.
//! The page has a session of its own that reaches the public internet
//! alone (through the PDF proxy, `pdf/macos/proxy.rs`), and is let go once
//! it has answered. On macOS that is WebKit, part of the `browser`
//! feature; there is none elsewhere yet.

#![cfg_attr(
    not(all(feature = "browser", target_os = "macos")),
    allow(dead_code, reason = "what reads a page is macOS's alone, so far")
)]

#[cfg(all(feature = "browser", target_os = "macos"))]
mod macos;

use {
    moonframe_providers::{Bounds, Caller, HostCapability},
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

/// The capability's name, in provider.toml's `capabilities`.
pub(crate) const NAME: &str = "moonframe.page";

/// How long a page may take, by default and at most.
const TIMEOUT: Duration = Duration::from_secs(20);
const MAX_TIMEOUT: Duration = Duration::from_secs(60);
/// How long reading a page that has loaded (or run out of time) may take.
const READING: Duration = Duration::from_secs(10);
/// The window the page is laid out in, by default, in points.
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 900;
/// Most text kept, in bytes as the answer's JSON writes it: the rest is
/// cut, and `truncated` says so.
const MAX_TEXT: usize = 2 * 1024 * 1024;
/// Most of a title and an address kept, the same way.
const MAX_TITLE: usize = 4 * 1024;
const MAX_URL: usize = 8 * 1024;
/// Largest picture answered, as PNG bytes.
const MAX_PICTURE: usize = 4 * 1024 * 1024;
/// Most selectors `drop` takes, and the longest.
const MAX_DROPS: usize = 64;
const MAX_SELECTOR: usize = 256;

/// The bounds of a call: a page that runs its whole time, then is read.
pub(crate) fn bounds() -> Bounds {
    Bounds {
        // The text, title and address as JSON writes them and the picture
        // as base64, with room for the rest.
        max_bytes: MAX_TEXT + MAX_TITLE + MAX_URL + MAX_PICTURE.div_ceil(3) * 4 + 64 * 1024,
        timeout: MAX_TIMEOUT + READING + Duration::from_secs(5),
    }
}

/// `render`'s arguments.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Args {
    /// An http or https address.
    url: String,
    /// Whether to answer with a picture of what shows.
    #[serde(default)]
    screenshot: bool,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    /// Seconds the page has to load before it is read as it is.
    #[serde(default)]
    timeout: Option<f64>,
    /// CSS selectors of what to leave out of the text (`nav`, `footer`).
    #[serde(default)]
    drop: Vec<String>,
}

/// A page to render, checked.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Request {
    pub(crate) url: String,
    pub(crate) screenshot: bool,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) timeout: Duration,
    pub(crate) drop: Vec<String>,
}

impl Request {
    fn read(args: &str) -> Result<Self, String> {
        let args: Args =
            serde_json::from_str(args).map_err(|error| format!("render's arguments: {error}"))?;
        let url = url::Url::parse(&args.url).map_err(|error| format!("`url`: {error}"))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("`url` must be an http or https address".to_owned());
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("`url` may not carry a user name or password".to_owned());
        }
        let width = args.width.unwrap_or(WIDTH);
        let height = args.height.unwrap_or(HEIGHT);
        if !(320..=3840).contains(&width) || !(240..=4096).contains(&height) {
            return Err("`width` is 320 to 3840 and `height` 240 to 4096".to_owned());
        }
        let timeout = match args.timeout {
            None => TIMEOUT,
            Some(seconds) if (1.0..=MAX_TIMEOUT.as_secs_f64()).contains(&seconds) => {
                Duration::from_secs_f64(seconds)
            },
            Some(_) => {
                return Err(format!(
                    "`timeout` is 1 to {} seconds",
                    MAX_TIMEOUT.as_secs()
                ));
            },
        };
        if args.drop.len() > MAX_DROPS || args.drop.iter().any(|drop| drop.len() > MAX_SELECTOR) {
            return Err(format!(
                "`drop` takes up to {MAX_DROPS} selectors of up to {MAX_SELECTOR} bytes"
            ));
        }
        Ok(Self {
            url: url.into(),
            screenshot: args.screenshot,
            width,
            height,
            timeout,
            drop: args.drop,
        })
    }
}

/// What a page came to.
#[derive(Debug, Default, PartialEq, Serialize)]
pub(crate) struct Rendered {
    /// Where the page ended up, after its redirects.
    pub(crate) url: String,
    pub(crate) title: String,
    /// The text a reader sees (`innerText`), without what `drop` named.
    pub(crate) text: String,
    /// The text was longer than the most kept, and cut.
    pub(crate) truncated: bool,
    /// The page had not finished loading in its time; this is what it
    /// showed then.
    pub(crate) timed_out: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) screenshot: Option<Screenshot>,
    /// Why there is no picture, when one was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) screenshot_error: Option<String>,
}

/// A picture of what showed, as a PNG.
#[derive(Debug, PartialEq, Serialize)]
pub(crate) struct Screenshot {
    pub(crate) mime: &'static str,
    pub(crate) base64: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Rendered {
    /// What a page that ended up at `url`, titled `title`, showed: each
    /// kept within its most as the answer's JSON writes it, on a
    /// character's edge; a cut text says so.
    pub(crate) fn page(url: &str, title: &str, text: &str, timed_out: bool) -> Self {
        let (text, truncated) = within(text, MAX_TEXT);
        Self {
            url: within(url, MAX_URL).0.to_owned(),
            title: within(title, MAX_TITLE).0.to_owned(),
            text: text.to_owned(),
            truncated,
            timed_out,
            ..Self::default()
        }
    }
}

/// The start of `text` that JSON writes in at most `most` bytes, and
/// whether that is less than all of it.
fn within(text: &str, most: usize) -> (&str, bool) {
    let mut written = 0;
    for (at, c) in text.char_indices() {
        written += match c {
            '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
            c if u32::from(c) < 0x20 => 6,
            c => c.len_utf8(),
        };
        if written > most {
            return (&text[..at], true);
        }
    }
    (text, false)
}

/// The capability providers call.
pub(crate) struct Page;

impl HostCapability for Page {
    fn call(&self, caller: &Caller, method: &str, args: &str) -> Result<String, String> {
        match method {
            "render" => {
                let request = Request::read(args)?;
                tracing::info!(provider = %caller.provider, url = %request.url, "rendering a page");
                let rendered = render(request)?;
                serde_json::to_string(&rendered).map_err(|error| error.to_string())
            },
            other => Err(format!("`{NAME}` has no method `{other}`; it has `render`")),
        }
    }
}

#[cfg(all(feature = "browser", target_os = "macos"))]
fn render(request: Request) -> Result<Rendered, String> {
    macos::render(request)
}

#[cfg(not(all(feature = "browser", target_os = "macos")))]
fn render(_: Request) -> Result<Rendered, String> {
    Err(if cfg!(feature = "browser") {
        "pages are rendered on macOS only, so far".to_owned()
    } else {
        "this app was built without the `browser` feature".to_owned()
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_request_is_a_web_address_with_bounds() {
        let request = Request::read(r#"{ "url": "https://example.com/a" }"#).unwrap();
        assert_eq!(request, Request {
            url: "https://example.com/a".to_owned(),
            screenshot: false,
            width: WIDTH,
            height: HEIGHT,
            timeout: TIMEOUT,
            drop: Vec::new(),
        });
        let asked = Request::read(
            r#"{ "url": "http://example.com", "screenshot": true, "width": 800, "timeout": 5, "drop": ["nav"] }"#,
        )
        .unwrap();
        assert_eq!(
            (asked.screenshot, asked.width, asked.timeout, asked.drop),
            (true, 800, Duration::from_secs(5), vec!["nav".to_owned()])
        );
        for (args, problem) in [
            (r#"{ "url": "file:///etc/hosts" }"#, "http or https"),
            (r#"{ "url": "data:text/html,hi" }"#, "http or https"),
            (r#"{ "url": "https://me:secret@example.com" }"#, "password"),
            (
                r#"{ "url": "https://example.com", "width": 10 }"#,
                "`width`",
            ),
            (
                r#"{ "url": "https://example.com", "timeout": 600 }"#,
                "`timeout`",
            ),
            (
                r#"{ "url": "https://example.com", "zoom": 2 }"#,
                "unknown field",
            ),
            (r#"{ "screenshot": true }"#, "missing field `url`"),
        ] {
            let error = Request::read(args).unwrap_err();
            assert!(error.contains(problem), "{args}: {error}");
        }
    }

    #[test]
    fn long_text_is_cut_on_a_character_as_json_writes_it() {
        let rendered = Rendered::page("", "", &"é".repeat(MAX_TEXT), false);
        assert!(rendered.truncated);
        assert!(rendered.text.len() <= MAX_TEXT);
        assert!(rendered.text.chars().all(|c| c == 'é'));
        // Quotes take two bytes each in the answer, which stays in bounds.
        let quoted = Rendered::page("", "", &"\"".repeat(MAX_TEXT), false);
        assert_eq!((quoted.text.len(), quoted.truncated), (MAX_TEXT / 2, true));
        let json = serde_json::to_string(&quoted.text).unwrap();
        assert!(json.len() <= MAX_TEXT + 2, "{}", json.len());
        let title = Rendered::page("", &"\u{1}".repeat(MAX_TITLE), "", false).title;
        assert_eq!(title.len(), MAX_TITLE / 6);
        let short = Rendered::page("", "", "hello", false);
        assert_eq!((short.text.as_str(), short.truncated), ("hello", false));
    }

    #[test]
    fn the_answer_names_its_parts() {
        let rendered = Rendered::page("https://example.com/", "Example", "Hello", false);
        let json: serde_json::Value = serde_json::to_value(&rendered).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "url": "https://example.com/", "title": "Example", "text": "Hello",
                "truncated": false, "timed_out": false,
            })
        );
    }

    #[test]
    fn every_app_offers_it() {
        let launch = crate::Moonframe::new("pages");
        assert!(launch.errors.is_empty(), "{:?}", launch.errors);
        assert!(launch.providers.capabilities.contains(NAME));
    }

    #[test]
    fn other_methods_are_refused() {
        let caller = Caller {
            provider: "test".to_owned(),
            folder: "/p".into(),
            root: "/".into(),
            namespace: None,
            capabilities: vec![NAME.to_owned()],
        };
        assert!(
            Page.call(&caller, "click", "{}")
                .unwrap_err()
                .contains("has `render`")
        );
    }
}
