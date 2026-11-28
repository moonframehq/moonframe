//! What a fetched page says about itself: its title and its icon. The
//! page is untrusted, so it is only scanned for a few tags, never run or
//! rendered; anything odd is skipped.

/// Longest title kept, in characters.
const MAX_TITLE: usize = 200;

/// One start tag: its name and attributes, names lowercased, values
/// with their entities decoded.
pub(super) struct Tag {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    /// The byte just past the tag's `>`.
    pub end: usize,
}

impl Tag {
    pub(super) fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Where `needle` (lowercase ASCII) first is in `text` from `from`, in
/// any case.
pub(super) fn find(text: &str, from: usize, needle: &str) -> Option<usize> {
    let bytes = text.as_bytes().get(from..)?;
    let needle = needle.as_bytes();
    bytes
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
        .map(|at| from + at)
}

/// The page's head: up to `</head>` or `<body`, else all of it.
fn head(html: &str) -> &str {
    let end = [find(html, 0, "</head"), find(html, 0, "<body")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(html.len());
    &html[..end]
}

/// Every start tag in `html`, skipping comments and what `<script>`,
/// `<style>`, `<title>` and `<textarea>` hold.
pub(super) fn tags(html: &str) -> impl Iterator<Item = Tag> + '_ {
    let mut at = 0;
    std::iter::from_fn(move || {
        loop {
            let open = at + html.get(at..)?.find('<')?;
            let rest = &html[open..];
            if rest.starts_with("<!--") {
                at = rest.find("-->").map_or(html.len(), |end| open + end + 3);
                continue;
            }
            if !rest.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic) {
                at = open + 1;
                continue;
            }
            let mut tag = read_tag(rest);
            tag.end += open;
            at = tag.end;
            if matches!(tag.name.as_str(), "script" | "style" | "title" | "textarea") {
                at = find(html, at, &format!("</{}", tag.name)).unwrap_or(html.len());
            }
            return Some(tag);
        }
    })
}

/// The tag `text` starts with (`<name attr="value" ...>`).
fn read_tag(text: &str) -> Tag {
    let bytes = text.as_bytes();
    let mut at = 1;
    while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'-') {
        at += 1;
    }
    let name = text[1..at].to_ascii_lowercase();
    let mut attributes = Vec::new();
    loop {
        while at < bytes.len() && (bytes[at].is_ascii_whitespace() || bytes[at] == b'/') {
            at += 1;
        }
        if at >= bytes.len() {
            break;
        }
        if bytes[at] == b'>' {
            at += 1;
            break;
        }
        let start = at;
        while at < bytes.len()
            && !bytes[at].is_ascii_whitespace()
            && !matches!(bytes[at], b'=' | b'>' | b'/')
        {
            at += 1;
        }
        // A byte no rule above moves past (a lone `=`): step over it.
        if at == start {
            at += 1;
            continue;
        }
        let key = text[start..at].to_ascii_lowercase();
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let mut value = String::new();
        if bytes.get(at) == Some(&b'=') {
            at += 1;
            while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            let (from, to, next) = match bytes.get(at) {
                Some(&quote @ (b'"' | b'\'')) => {
                    let end = text[at + 1..]
                        .find(quote as char)
                        .map_or(text.len(), |end| at + 1 + end);
                    (at + 1, end, (end + 1).min(text.len()))
                },
                _ => {
                    let end = text[at..]
                        .find(|c: char| c.is_ascii_whitespace() || c == '>')
                        .map_or(text.len(), |end| at + end);
                    (at, end, end)
                },
            };
            value = decode(&text[from..to]);
            at = next;
        }
        attributes.push((key, value));
    }
    Tag {
        name,
        attributes,
        end: at.min(text.len()),
    }
}

/// The page's title: its `og:title` when it has one, else its `<title>`,
/// decoded and on one line.
pub(super) fn title(html: &str) -> Option<String> {
    let head = head(html);
    let mut graph = None;
    let mut plain = None;
    for tag in tags(head) {
        match tag.name.as_str() {
            "meta" if graph.is_none() => {
                let property = tag.attribute("property").or_else(|| tag.attribute("name"));
                if property.is_some_and(|property| property.eq_ignore_ascii_case("og:title")) {
                    graph = tag.attribute("content").and_then(clean);
                }
            },
            "title" if plain.is_none() => {
                let end = find(head, tag.end, "</title").unwrap_or(head.len());
                plain = clean(&decode(&head[tag.end..end]));
            },
            _ => {},
        }
    }
    graph.or(plain)
}

/// `text` on one line, its spaces collapsed, clipped; None when empty.
fn clean(text: &str) -> Option<String> {
    let line = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TITLE)
        .collect::<String>();
    let line = line.trim().to_string();
    (!line.is_empty()).then_some(line)
}

/// The page's best icon link, as written: an `apple-touch-icon` (the
/// largest), then a PNG or SVG `icon`, then any `icon`.
pub(super) fn icon(html: &str) -> Option<String> {
    let mut best: Option<(u8, u32, String)> = None;
    for tag in tags(head(html)).filter(|tag| tag.name == "link") {
        let Some(href) = tag.attribute("href").map(str::trim) else {
            continue;
        };
        if href.is_empty() || href.to_ascii_lowercase().starts_with("data:") {
            continue;
        }
        let rel = tag.attribute("rel").unwrap_or("").to_ascii_lowercase();
        let rel: Vec<&str> = rel.split_ascii_whitespace().collect();
        let kind = tag.attribute("type").unwrap_or("").to_ascii_lowercase();
        let path = href
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let rank = if rel
            .iter()
            .any(|rel| matches!(*rel, "apple-touch-icon" | "apple-touch-icon-precomposed"))
        {
            0
        } else if !rel.contains(&"icon") {
            continue;
        } else if kind == "image/png"
            || kind == "image/svg+xml"
            || path.ends_with(".png")
            || path.ends_with(".svg")
        {
            1
        } else {
            2
        };
        let size = size(tag.attribute("sizes").unwrap_or(""));
        // Lower rank first, then the larger.
        let better = best.as_ref().is_none_or(|(best_rank, best_size, _)| {
            rank < *best_rank || rank == *best_rank && size > *best_size
        });
        if better {
            best = Some((rank, size, href.to_string()));
        }
    }
    best.map(|(_, _, href)| href)
}

/// The largest side `sizes` names (`"32x32 180x180"`); `any` (an SVG)
/// beats them all.
fn size(sizes: &str) -> u32 {
    sizes
        .split_ascii_whitespace()
        .filter_map(|size| {
            if size.eq_ignore_ascii_case("any") {
                return Some(u32::MAX);
            }
            let (width, _) = size.split_once(['x', 'X'])?;
            width.parse().ok()
        })
        .max()
        .unwrap_or(0)
}

/// `text` with its HTML entities (`&amp;`, `&#39;`, `&#x2014;`) decoded;
/// one it does not know stays as written.
pub(super) fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest[1..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '#'))
            .map_or(rest.len(), |end| end + 1);
        match entity(&rest[1..end]) {
            Some(decoded) => {
                out.push(decoded);
                let semicolon = rest[end..].starts_with(';');
                rest = &rest[end + usize::from(semicolon)..];
            },
            None => {
                out.push('&');
                rest = &rest[1..];
            },
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if name.len() > 10 {
        return None;
    }
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        return char::from_u32(code).filter(|c| *c != '\0');
    }
    Some(match name {
        // keep-sorted start
        "amp" => '&',
        "apos" => '\'',
        "bull" => '•',
        "copy" => '©',
        "gt" => '>',
        "hellip" => '…',
        "laquo" => '«',
        "ldquo" => '“',
        "lsquo" => '‘',
        "lt" => '<',
        "mdash" => '—',
        "middot" => '·',
        "nbsp" => '\u{a0}',
        "ndash" => '–',
        "quot" => '"',
        "raquo" => '»',
        "rdquo" => '”',
        "reg" => '®',
        "rsquo" => '’',
        "times" => '×',
        "trade" => '™',
        // keep-sorted end
        _ => return None,
    })
}
