//! Urls taken apart (host, origin, a link resolved against a page) and a
//! note's bare links given their titles.

use {
    crate::parse::{WebLink, is_web},
    std::collections::HashMap,
};

/// The url's authority (`user@host:port`).
fn authority(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    Some(&rest[..end])
}

/// The url's host, lowercase, without user or port, if it is one a file
/// may be named after (see [`safe_host`]).
pub(super) fn host(url: &str) -> Option<String> {
    if !is_web(url) {
        return None;
    }
    let authority = authority(url)?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split(':').next().unwrap_or("");
    safe_host(&host.trim_end_matches('.').to_ascii_lowercase())
}

/// `host` when it is only `a-z`, `0-9`, `.` and `-`, not empty, without
/// `..` and not starting with a dot or a dash: safe as a file name in the
/// favicons folder.
pub(super) fn safe_host(host: &str) -> Option<String> {
    let safe = !host.is_empty()
        && host.len() <= 253
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
        && !host.contains("..")
        && !host.starts_with(['.', '-']);
    safe.then(|| host.to_string())
}

/// `scheme://authority` of `url`.
pub(super) fn origin(url: &str) -> Option<String> {
    let (scheme, _) = url.split_once("://")?;
    Some(format!("{scheme}://{}", authority(url)?))
}

/// `href`, found in the page at `base`, as a whole url; None unless it is
/// `http:` or `https:` (no `data:`, `javascript:` and the like).
pub(super) fn resolve(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || !is_web(base) {
        return None;
    }
    if is_web(href) {
        return Some(href.to_string());
    }
    let (scheme, rest) = base.split_once("://")?;
    if let Some(net) = href.strip_prefix("//") {
        return Some(format!("{scheme}://{net}"));
    }
    // Another scheme: a colon before any `/`, `?` or `#`.
    let first = href.find(['/', '?', '#']).unwrap_or(href.len());
    if href[..first].contains(':') {
        return None;
    }
    let origin = origin(base)?;
    let path = &rest[rest.find(['/', '?', '#']).unwrap_or(rest.len())..];
    let path = path.split(['?', '#']).next().unwrap_or("");
    Some(if href.starts_with('/') {
        format!("{origin}{href}")
    } else if href.starts_with(['?', '#']) {
        format!(
            "{origin}{}{href}",
            if path.is_empty() {
                "/"
            } else {
                path
            }
        )
    } else {
        let folder = path.rfind('/').map_or("/", |end| &path[..=end]);
        format!("{origin}{folder}{href}")
    })
}

/// `source` with each bare link that has a title in `titles` (by url)
/// written `[Title](url)`, and how many were.
pub(super) fn rewrite(
    source: &str,
    links: &[WebLink],
    titles: &HashMap<String, String>,
) -> (String, usize) {
    let mut out = String::with_capacity(source.len());
    let mut done = 0;
    let mut last = 0;
    for link in links.iter().filter(|link| link.bare) {
        let Some(title) = titles.get(&link.url) else {
            continue;
        };
        let Some(written) = source.get(link.span.clone()) else {
            continue;
        };
        if link.span.start < last {
            continue;
        }
        let title = escape(title);
        let replacement = if written.starts_with('[') {
            // Keep the destination as written: `](url "title")`.
            let Some(open) = written.rfind("](") else {
                continue;
            };
            format!("[{title}{}", &written[open..])
        } else {
            let url = written
                .strip_prefix('<')
                .and_then(|url| url.strip_suffix('>'))
                .unwrap_or(written);
            format!("[{title}]({})", destination(url))
        };
        out.push_str(&source[last..link.span.start]);
        out.push_str(&replacement);
        last = link.span.end;
        done += 1;
    }
    out.push_str(&source[last..]);
    (out, done)
}

/// A title as link text: what markdown would read as syntax escaped.
fn escape(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for c in title.chars() {
        if matches!(c, '\\' | '[' | ']' | '|' | '*' | '`' | '<') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `url` as a link destination: in `<...>` when spaces or unbalanced
/// parentheses would end it early.
fn destination(url: &str) -> String {
    let mut depth = 0i32;
    let mut balanced = true;
    for c in url.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                balanced &= depth >= 0;
            },
            _ => {},
        }
    }
    if balanced && depth == 0 && !url.contains(char::is_whitespace) {
        url.to_string()
    } else {
        format!("<{url}>")
    }
}
