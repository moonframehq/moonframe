//! A note's file read into what the index keeps: its properties, headings,
//! anchored blocks, links, tags and tasks. One pass of the markdown parser
//! the app also renders with (`markdown`) finds structure and the spans to
//! mask (code, math, HTML); Obsidian's own syntax is then read from the
//! raw text around them (see `inline`).

mod frontmatter;
mod inline;

pub(crate) use {frontmatter::with_id, inline::WikiSpan};

use {
    crate::text::{self, Lines},
    inline::Mask,
    markdown::{ParseOptions, mdast::Node, to_mdast},
    serde::Serialize,
    serde_json::{Map, Value},
    std::ops::Range,
};

/// Who may change a note (see VAULT.md, "Ownership").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Owner {
    Human,
    Llm,
    Source,
}

impl Owner {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Llm => "llm",
            Self::Source => "source",
        }
    }

    /// The owner a path has by its folder, before any `owner:` key.
    pub fn of_path(path: &str) -> Self {
        let lower = path.to_lowercase();
        if lower.starts_with("wiki/") || lower == "index.md" || lower == "log.md" {
            Self::Llm
        } else if lower.starts_with("raw/") {
            Self::Source
        } else {
            Self::Human
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: u32,
}

/// A block someone can link to: one with a `^id` at its end.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Block {
    pub anchor: String,
    pub text: String,
    pub line: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    /// `[[...]]`
    Wiki,
    /// `![[...]]`
    Embed,
    /// `[text](path.md)`
    Markdown,
}

impl LinkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wiki => "wiki",
            Self::Embed => "embed",
            Self::Markdown => "markdown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Link {
    /// As written, without `#anchor` and `|display`; empty for `[[#Local]]`.
    pub target: String,
    pub anchor: Option<String>,
    pub display: Option<String>,
    pub kind: LinkKind,
    pub line: u32,
    /// The line it is on, for backlinks to show.
    pub context: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Task {
    pub done: bool,
    pub text: String,
    pub line: u32,
}

/// Everything the index keeps about one file.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Note {
    /// Relative to the vault, `/`-separated.
    pub path: String,
    /// The file name without `.md`, which links resolve by.
    pub title: String,
    pub id: Option<String>,
    pub aliases: Vec<String>,
    /// From the frontmatter and the text, as first written, without `#`.
    pub tags: Vec<String>,
    /// The frontmatter's `type`.
    pub kind: Option<String>,
    pub owner: Owner,
    /// Every frontmatter key, in order.
    pub props: Map<String, Value>,
    pub headings: Vec<Heading>,
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
    pub tasks: Vec<Task>,
    /// The text without frontmatter, for full-text search.
    pub body: String,
    /// The first lines of prose, for lists (up to 200 characters).
    pub excerpt: String,
    pub words: u32,
    pub hash: String,
}

/// Longest line kept as a link's context.
const CONTEXT: usize = 240;

/// The byte where `source`'s body starts, after any frontmatter.
#[cfg_attr(not(feature = "provider"), allow(dead_code))]
pub(crate) fn body_start(source: &str) -> usize {
    frontmatter::find(source).map_or(0, |fenced| fenced.body_start)
}

/// Reads `source`, the file at vault path `path`.
pub fn parse(path: &str, source: &str) -> Note {
    let lines = Lines::new(source);
    let fenced = frontmatter::find(source);
    let props = fenced
        .as_ref()
        .map(|fenced| frontmatter::read(fenced.yaml))
        .unwrap_or_default();
    let body_start = fenced.as_ref().map_or(0, |fenced| fenced.body_start);

    let (mask, structure, wiki) = analyse(source, &lines, body_start);
    let mut links: Vec<Link> = wiki
        .iter()
        .map(|span| {
            let line = lines.line(span.span.start);
            Link {
                target: span.target.clone(),
                anchor: span.anchor.clone(),
                display: span.display.clone(),
                kind: if span.embed {
                    LinkKind::Embed
                } else {
                    LinkKind::Wiki
                },
                line,
                context: text::clip(lines.text(source, line), CONTEXT),
            }
        })
        .collect();
    for (span, url) in &structure.markdown_links {
        if let Some((target, anchor)) = local_target(url) {
            let line = lines.line(span.start);
            links.push(Link {
                target,
                anchor,
                display: None,
                kind: LinkKind::Markdown,
                line,
                context: text::clip(lines.text(source, line), CONTEXT),
            });
        }
    }
    links.sort_by_key(|link| link.line);

    let mut tags: Vec<String> = Vec::new();
    let mut push_tag = |tag: &str| {
        let tag = tag.trim().trim_start_matches('#');
        if !tag.is_empty() && !tags.iter().any(|seen| seen.eq_ignore_ascii_case(tag)) {
            tags.push(tag.to_string());
        }
    };
    for tag in frontmatter::strings(props.get("tags").or_else(|| props.get("tag"))) {
        push_tag(&tag);
    }
    for (_, tag) in inline::tags(source, &mask, &wiki) {
        push_tag(&tag);
    }

    let mut blocks = Vec::new();
    for line_number in 1..=lines.count() {
        let line = lines.text(source, line_number);
        let Some(anchor) = inline::anchor(line) else {
            continue;
        };
        if mask.hides(lines.start(line_number) + line.trim_end().len().saturating_sub(1)) {
            continue;
        }
        let text = line.trim_end();
        let text = text[..text.len() - anchor.len() - 1].trim();
        blocks.push(Block {
            anchor: anchor.to_string(),
            text: text::clip(strip_marker(text), CONTEXT),
            line: line_number,
        });
    }

    let owner = match props.get("owner").and_then(Value::as_str) {
        Some("llm") => Owner::Llm,
        Some("human") => Owner::Human,
        Some("source") => Owner::Source,
        _ => Owner::of_path(path),
    };
    let body = source[body_start.min(source.len())..].to_string();
    let words = u32::try_from(body.split_whitespace().count()).unwrap_or(u32::MAX);
    let excerpt = excerpt(&body);
    Note {
        path: path.to_string(),
        title: text::stem(path).to_string(),
        id: props.get("id").and_then(Value::as_str).map(str::to_string),
        aliases: frontmatter::strings(props.get("aliases").or_else(|| props.get("alias"))),
        tags,
        kind: props
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_string),
        owner,
        props,
        headings: structure.headings,
        blocks,
        links,
        tasks: structure.tasks,
        body,
        excerpt,
        words,
        hash: text::digest(source),
    }
}

/// The mask, the markdown structure and the wikilinks of `source`.
fn analyse(source: &str, lines: &Lines, body_start: usize) -> (Mask, Structure, Vec<WikiSpan>) {
    let mut mask = Mask::default();
    mask.add(0..body_start);
    let mut structure = Structure::default();
    let mut options = ParseOptions::gfm();
    options.constructs.frontmatter = true;
    options.constructs.math_flow = true;
    options.constructs.math_text = true;
    if let Ok(root) = to_mdast(source, &options) {
        walk(&root, source, lines, &mut mask, &mut structure);
    }
    // Comments are found with code already masked (`%%` in code is text),
    // then masked themselves.
    mask.seal();
    for comment in inline::comments(source, &mask) {
        mask.add(comment);
    }
    mask.seal();
    // Links in properties count, as in Obsidian (`source: "[[Book]]"`).
    let mut wiki = inline::wikilinks(&source[..body_start.min(source.len())], &Mask::default());
    wiki.extend(inline::wikilinks(source, &mask));
    (mask, structure, wiki)
}

/// Where links are in a file, to rewrite them on a rename: each wikilink,
/// and each markdown link's url with the target and anchor it means.
pub(crate) struct Spans {
    pub wiki: Vec<WikiSpan>,
    pub markdown: Vec<(Range<usize>, String)>,
}

pub(crate) fn spans(source: &str) -> Spans {
    let lines = Lines::new(source);
    let body_start = frontmatter::find(source).map_or(0, |fenced| fenced.body_start);
    let (_, structure, wiki) = analyse(source, &lines, body_start);
    let markdown = structure
        .markdown_links
        .into_iter()
        .filter_map(|(span, url)| local_target(&url).map(|(target, _)| (span, target)))
        .collect();
    Spans { wiki, markdown }
}

/// The url's bytes inside a markdown link or image at `span`:
/// `[text](url "title")` or `[text](<url with spaces>)`. None for a
/// reference link, which has no url in place.
fn url_span(source: &str, span: &Range<usize>) -> Option<Range<usize>> {
    let text = source.get(span.clone())?;
    if !text.ends_with(')') {
        return None;
    }
    let open = text.rfind("](")? + 2;
    let inner = &text[open..text.len() - 1];
    let leading = inner.len() - inner.trim_start().len();
    let inner_trimmed = inner.trim_start();
    let (start, length) = if let Some(rest) = inner_trimmed.strip_prefix('<') {
        (open + leading + 1, rest.find('>')?)
    } else {
        (
            open + leading,
            inner_trimmed
                .find(char::is_whitespace)
                .unwrap_or(inner_trimmed.len()),
        )
    };
    Some(span.start + start..span.start + start + length)
}

/// What the markdown tree gives besides the mask.
#[derive(Default)]
struct Structure {
    headings: Vec<Heading>,
    tasks: Vec<Task>,
    /// `[text](url)` and `![alt](url)`: where the url is, and the url.
    markdown_links: Vec<(Range<usize>, String)>,
}

fn walk(node: &Node, source: &str, lines: &Lines, mask: &mut Mask, out: &mut Structure) {
    let span = node
        .position()
        .map(|position| position.start.offset..position.end.offset);
    match node {
        Node::Code(_)
        | Node::InlineCode(_)
        | Node::Math(_)
        | Node::InlineMath(_)
        | Node::Html(_)
        | Node::Yaml(_)
        | Node::Toml(_) => {
            if let Some(span) = span {
                mask.add(span);
            }
            return;
        },
        Node::Heading(heading) => {
            if let Some(span) = &span {
                let line = lines.line(span.start);
                let raw = lines.text(source, line);
                let text = raw.trim_start_matches('#').trim();
                let text = match inline::anchor(text) {
                    Some(anchor) => text[..text.len() - anchor.len() - 1].trim(),
                    None => text,
                };
                let text = if text.is_empty() {
                    node.to_string()
                } else {
                    text.to_string()
                };
                out.headings.push(Heading {
                    level: heading.depth,
                    text,
                    line,
                });
            }
        },
        Node::ListItem(item) => {
            if let (Some(done), Some(span)) = (item.checked, &span) {
                let line = lines.line(span.start);
                let raw = lines.text(source, line).trim();
                let text = strip_marker(raw);
                let text = text
                    .strip_prefix("[ ]")
                    .or_else(|| text.strip_prefix("[x]"))
                    .or_else(|| text.strip_prefix("[X]"))
                    .unwrap_or(text);
                out.tasks.push(Task {
                    done,
                    text: text::clip(text.trim(), CONTEXT),
                    line,
                });
            }
        },
        Node::Link(link) => {
            if let Some(url) = span.as_ref().and_then(|span| url_span(source, span)) {
                out.markdown_links.push((url, link.url.clone()));
            }
        },
        Node::Image(image) => {
            if let Some(url) = span.as_ref().and_then(|span| url_span(source, span)) {
                out.markdown_links.push((url, image.url.clone()));
            }
        },
        _ => {},
    }
    if let Some(children) = node.children() {
        for child in children {
            walk(child, source, lines, mask, out);
        }
    }
}

/// A markdown link's url as a vault target and anchor, unless it leaves the
/// vault (`https:`, `mailto:`) or only jumps in the page (`#x`).
fn local_target(url: &str) -> Option<(String, Option<String>)> {
    let url = url.trim();
    // A url with a scheme (`https:`, `mailto:`, `tel:`, `applenotes:`)
    // leaves the vault; a Windows drive letter (`C:`) is a path.
    let scheme = url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1
            && scheme
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    if url.is_empty() || url.starts_with('#') || scheme {
        return None;
    }
    let decoded = percent_decode(url);
    let (target, anchor) = match decoded.split_once('#') {
        Some((target, anchor)) => (
            target.to_string(),
            Some(anchor.to_string()).filter(|anchor| !anchor.is_empty()),
        ),
        None => (decoded.clone(), None),
    };
    Some((target, anchor))
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Some(byte) = text
                .get(index + 1..index + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A list line without its `- `, `* `, `+ ` or `1. ` marker.
fn strip_marker(line: &str) -> &str {
    let line = line.trim_start();
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return rest;
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0
        && let Some(rest) = line[digits..]
            .strip_prefix(". ")
            .or_else(|| line[digits..].strip_prefix(") "))
    {
        return rest;
    }
    line
}

/// Longest excerpt kept for lists.
const EXCERPT: usize = 200;

/// The first prose of `body`: headings, fences, quotes' markers, list
/// markers, comments and markup left out, `[[a|b]]` as `b`.
fn excerpt(body: &str) -> String {
    let mut out = String::new();
    let mut fenced = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced
            || trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('<')
            || trimmed.starts_with("%%")
            || trimmed.starts_with("---")
            || trimmed.starts_with('|')
        {
            continue;
        }
        let trimmed = trimmed.trim_start_matches('>').trim();
        let trimmed = strip_marker(trimmed);
        let trimmed = trimmed
            .strip_prefix("[ ] ")
            .or_else(|| trimmed.strip_prefix("[x] "))
            .unwrap_or(trimmed);
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&plain(trimmed));
        if out.chars().count() > EXCERPT {
            break;
        }
    }
    text::clip(&out, EXCERPT)
}

/// Inline markdown as plain words: `[[a|b]]` as b, `[[a]]` as a, `[t](u)`
/// as t, and `*`, `_`, `` ` ``, `==`, `~~` dropped.
fn plain(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find("[[") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        match after.find("]]") {
            Some(close) => {
                let inner = &after[..close];
                let shown = inner.rsplit('|').next().unwrap_or(inner);
                out.push_str(
                    shown
                        .split('#')
                        .next()
                        .filter(|name| !name.is_empty())
                        .unwrap_or(shown),
                );
                rest = &after[close + 2..];
            },
            None => {
                out.push_str("[[");
                rest = after;
            },
        }
    }
    out.push_str(rest);
    let out = out.replace("==", "").replace("~~", "");
    out.chars()
        .filter(|c| !matches!(c, '*' | '`'))
        .collect::<String>()
        .replace(" _", " ")
        .replace("_ ", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_with_a_scheme_leave_the_vault() {
        for url in [
            "https://x.io",
            "mailto:a@b.c",
            "tel:+14155550100",
            "applenotes:note/204eb979",
            "data:image/png;base64,AA",
        ] {
            assert_eq!(local_target(url), None, "{url}");
        }
        assert_eq!(
            local_target("My%20Note.md#Part"),
            Some(("My Note.md".to_string(), Some("Part".to_string())))
        );
        assert_eq!(
            local_target("notes/A: B.md").map(|(target, _)| target),
            Some("notes/A: B.md".to_string())
        );
    }
}
