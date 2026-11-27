//! What makes a `ui.document`'s page sealed, and what it tells its mod.
//!
//! The page is built with its scripts off and a session of its own that
//! keeps nothing (see `native`), on macOS with WebKit's content rules
//! blocking every load but `about:` and `data:`. The document it is given
//! starts with a Content-Security-Policy that refuses scripts and every
//! load besides inline styles and `data:` pictures and fonts, so it holds
//! where the rules do not reach (Windows), and with `color-scheme`, so its
//! defaults follow the app's light or dark look. The host's own script
//! ([`SCRIPT`]), the only one that runs, reports links, scrolling, the
//! selection and edits; nothing is followed: every navigation but the load
//! itself is refused.

#![cfg_attr(
    not(all(feature = "browser", any(target_os = "macos", windows))),
    allow(dead_code, reason = "only the native web views build and report")
)]

use {
    super::with,
    moonframe_core::{Data, Document, DocumentScroll, DocumentScrollTo, MAX_HTML},
    serde::Deserialize,
};

/// The host's script, run in every load before the document's own content.
pub(crate) const SCRIPT: &str = include_str!("sealed.js");

/// The address a sealed page shows, the only one it may load.
pub(crate) const BLANK: &str = "about:blank";

/// The longest report read: a whole document's HTML as JSON writes it.
/// One past the most `html` takes is read, to tell the mod it is too large.
pub(crate) const MAX_REPORT: usize = 3 * MAX_HTML;

/// What comes first in every sealed document, marked for the editor to
/// leave out of the HTML it reports.
const SEAL: &str = concat!(
    "<!doctype html>",
    "<meta data-moonframe http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; ",
    "style-src 'unsafe-inline' data:; img-src data:; font-src data:; media-src data:; ",
    "form-action 'none'; base-uri 'none'\">",
    "<meta data-moonframe http-equiv=\"x-dns-prefetch-control\" content=\"off\">",
    "<meta data-moonframe name=\"color-scheme\" content=\"light dark\">",
);

/// `html` as the page loads it: [`SEAL`] first, so nothing in it runs or
/// loads before the policy holds.
pub(crate) fn sealed(html: &str) -> String {
    let mut out = String::with_capacity(SEAL.len() + html.len());
    out.push_str(SEAL);
    out.push_str(html);
    out
}

/// What the host's script reports.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Reported {
    Link {
        url: String,
        #[serde(default)]
        text: String,
    },
    Scroll {
        y: f64,
        height: f64,
        content: f64,
    },
    Select {
        text: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    /// An edit; `large` when the document grew past what `html` takes,
    /// and is not sent.
    Change {
        #[serde(default)]
        html: String,
        #[serde(default)]
        large: bool,
    },
    Done {
        #[serde(default)]
        html: String,
        #[serde(default)]
        large: bool,
    },
}

/// What `body`, a message from the page, reports; `None` for anything else.
pub(crate) fn read(body: &str) -> Option<Reported> {
    if body.len() > MAX_REPORT {
        return None;
    }
    serde_json::from_str(body).ok()
}

/// A document page's state: what its mod gave, what the view shows, and
/// what is still to do once the view can take it.
#[derive(Debug, Default)]
pub(crate) struct State {
    /// The HTML the mod gave last (the file's, for `src`).
    html: String,
    /// The mod gave other HTML since the view last took it.
    given: bool,
    /// The HTML the view shows: what it was given, not sealed yet, or what
    /// its editor reported since; `None` before its first load. New HTML
    /// from the mod loads unless it is this, so an edit given back loads
    /// nothing, the HTML from before the edit loads again, and HTML the mod
    /// leaves as it was never undoes edits it was not given.
    shown: Option<String>,
    /// The view finished its last load.
    ready: bool,
    /// The editing switch the loaded document was last given.
    switched: Option<bool>,
    /// The view may load: built, and on macOS with its rules in place.
    sealed: bool,
    editable: bool,
    dark: Option<bool>,
    asked: Option<DocumentScrollTo>,
    /// Where to scroll once the document is loaded.
    pending: Option<DocumentScroll>,
    link: Option<Data>,
    scroll: Option<Data>,
    select: Option<Data>,
    change: Option<Data>,
    done: Option<Data>,
}

/// What the view is to do now.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Work {
    /// Load this sealed HTML.
    pub load: Option<String>,
    /// Look light or dark.
    pub dark: Option<bool>,
    /// Calls to the host's script: a function of `__moonframeDocument` and
    /// its argument.
    pub calls: Vec<(&'static str, serde_json::Value)>,
}

impl State {
    /// Takes what `element` asks this frame, with its HTML and the app's
    /// look.
    pub(crate) fn update(&mut self, element: &Document, html: &str, dark: bool) {
        self.link.clone_from(&element.link);
        self.scroll.clone_from(&element.scroll);
        self.select.clone_from(&element.select);
        self.change.clone_from(&element.change);
        self.done.clone_from(&element.done);
        if self.html != html {
            html.clone_into(&mut self.html);
            self.given = true;
        }
        self.editable = element.editable;
        self.dark = Some(dark);
        if self.asked != element.scroll_to {
            self.asked.clone_from(&element.scroll_to);
            self.pending = element.scroll_to.as_ref().map(|asked| asked.to.clone());
        }
    }

    /// The view is built and may load.
    pub(crate) fn seal(&mut self) {
        self.sealed = true;
    }

    /// The view finished a load.
    pub(crate) fn loaded(&mut self) {
        if self.shown.is_some() {
            self.ready = true;
        }
    }

    /// What the view is to do, given the look it was last given: the mod's
    /// document to load, unless it is the one shown; then, once it has
    /// loaded, the editing switch and where to scroll.
    pub(crate) fn work(&mut self, look: Option<bool>) -> Work {
        let mut work = Work::default();
        if !self.sealed {
            return work;
        }
        if look != self.dark {
            work.dark = self.dark;
        }
        let given = std::mem::take(&mut self.given) || self.shown.is_none();
        if given && self.shown.as_deref() != Some(self.html.as_str()) {
            work.load = Some(sealed(&self.html));
            self.shown = Some(self.html.clone());
            self.ready = false;
            self.switched = None;
        }
        if self.ready {
            if self.switched != Some(self.editable) {
                self.switched = Some(self.editable);
                work.calls
                    .push(("setEditable", serde_json::Value::Bool(self.editable)));
            }
            match self.pending.take() {
                Some(DocumentScroll::Y(y)) => work.calls.push(("scrollToY", serde_json::json!(y))),
                Some(DocumentScroll::Id(id)) => {
                    work.calls
                        .push(("scrollToId", serde_json::Value::String(id)));
                },
                None => {},
            }
        }
        work
    }

    /// The message `reported` makes for the mod, if it asked for one.
    pub(crate) fn report(&mut self, reported: Reported) -> Option<Data> {
        let number = |value: f64| Data::Number(value.round());
        match reported {
            Reported::Link { url, text } => Some(with(self.link.as_ref()?, [
                ("url", Data::String(url)),
                ("text", Data::String(text)),
            ])),
            Reported::Scroll { y, height, content } => Some(with(self.scroll.as_ref()?, [
                ("y", number(y)),
                ("height", number(height)),
                ("content_height", number(content)),
            ])),
            Reported::Select {
                text,
                x,
                y,
                width,
                height,
            } => Some(with(self.select.as_ref()?, [
                ("text", Data::String(text)),
                ("x", number(x)),
                ("y", number(y)),
                ("width", number(width)),
                ("height", number(height)),
            ])),
            Reported::Change { html, large } if self.editable => {
                self.edited(html, large, self.change.clone())
            },
            Reported::Done { html, large } if self.editable => {
                self.edited(html, large, self.done.clone())
            },
            Reported::Change { .. } | Reported::Done { .. } => None,
        }
    }
}

impl State {
    /// `msg` with the edited `html`, which the view now shows; or, when it
    /// grew past what `html` takes back, with `error` saying so instead.
    fn edited(&mut self, html: String, large: bool, msg: Option<Data>) -> Option<Data> {
        let large = large || html.len() > MAX_HTML;
        if !large {
            self.shown = Some(html.clone());
        }
        if large {
            return Some(with(msg.as_ref()?, [(
                "error",
                Data::String(format!(
                    "the document is larger than the {MAX_HTML} bytes `html` takes"
                )),
            )]));
        }
        Some(with(msg.as_ref()?, [("html", Data::String(html))]))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
