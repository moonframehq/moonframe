//! Links that leave for the web, `http:` or `https:`: markdown links
//! (`[text](url)`, `[text](<url>)`), autolinks (`<https://…>`) and bare
//! urls in the text. The markdown parser finds them, so nothing in code,
//! math or HTML counts, and `%% comments %%` are masked as for the rest.

use {
    super::{Lines, analyse, body_start, url_span},
    std::ops::Range,
};

/// One web link in a note.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "provider"), allow(dead_code))]
pub(crate) struct WebLink {
    /// The whole link as written, what a title replaces.
    pub span: Range<usize>,
    pub url: String,
    /// Whether it shows only its url (a bare url, an autolink, or a
    /// markdown link whose text is its url), so a title would help.
    pub bare: bool,
}

/// Every web link in `source`, in order.
#[cfg_attr(not(feature = "provider"), allow(dead_code))]
pub(crate) fn web_links(source: &str) -> Vec<WebLink> {
    let lines = Lines::new(source);
    let (mask, structure, _) = analyse(source, &lines, body_start(source));
    let mut links: Vec<WebLink> = Vec::new();
    for link in structure.web {
        // A url in a link's text is part of that link.
        let inside = links
            .last()
            .is_some_and(|outer| outer.span.end >= link.span.end);
        if !inside && !mask.hides(link.span.start) {
            links.push(link);
        }
    }
    links
}

/// Whether `url` is `http:` or `https:`.
pub(crate) fn is_web(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// The link at `span` whose destination is `url`, if it is a web link.
pub(super) fn classify(source: &str, span: Range<usize>, url: &str) -> Option<WebLink> {
    if !is_web(url) {
        return None;
    }
    let text = source.get(span.clone())?;
    let bare = if text.starts_with('[') {
        url_span(source, &span)?;
        let open = text.rfind("](")?;
        same_as_url(&text[1..open], url)
    } else if text.starts_with('<') || is_web(text) {
        true
    } else {
        // `www.example.com` is left as written.
        return None;
    };
    Some(WebLink {
        span,
        url: url.to_string(),
        bare,
    })
}

/// Whether a link's text is only its url (give or take the scheme or a
/// last `/`).
fn same_as_url(text: &str, url: &str) -> bool {
    fn plain(text: &str) -> &str {
        let text = text.trim();
        let text = text
            .strip_prefix('<')
            .and_then(|text| text.strip_suffix('>'))
            .unwrap_or(text);
        let text = text
            .strip_prefix("https://")
            .or_else(|| text.strip_prefix("http://"))
            .unwrap_or(text);
        text.strip_suffix('/').unwrap_or(text)
    }
    !text.trim().is_empty() && plain(text).eq_ignore_ascii_case(plain(url))
}
