//! Obsidian's inline syntax found in the raw text: `[[wikilinks]]`,
//! `![[embeds]]`, `#tags`, `^block` anchors and `%% comments %%`. Spans
//! the markdown parser says are code, math, HTML or frontmatter are masked
//! first, so nothing inside them counts.

use std::ops::Range;

/// A `[[...]]` or `![[...]]` found in the text, with where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WikiSpan {
    /// The whole `[[...]]` (with the `!` of an embed).
    pub span: Range<usize>,
    /// Just the target part (before `#` and `|`), to rewrite on a rename.
    pub target_span: Range<usize>,
    pub target: String,
    /// What follows `#`: a heading, or `^id` for a block.
    pub anchor: Option<String>,
    pub display: Option<String>,
    pub embed: bool,
}

/// Byte ranges no inline syntax may come from.
#[derive(Default)]
pub(crate) struct Mask {
    ranges: Vec<Range<usize>>,
}

impl Mask {
    pub(crate) fn add(&mut self, range: Range<usize>) {
        if range.start < range.end {
            self.ranges.push(range);
        }
    }

    /// Sorts and merges the ranges; call once all are added.
    pub(crate) fn seal(&mut self) {
        self.ranges.sort_by_key(|range| range.start);
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(self.ranges.len());
        for range in self.ranges.drain(..) {
            match merged.last_mut() {
                Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
                _ => merged.push(range),
            }
        }
        self.ranges = merged;
    }

    pub(crate) fn hides(&self, at: usize) -> bool {
        let index = self.ranges.partition_point(|range| range.end <= at);
        self.ranges
            .get(index)
            .is_some_and(|range| range.start <= at)
    }

    /// Whether any of `range` is hidden.
    pub(crate) fn touches(&self, range: &Range<usize>) -> bool {
        let index = self
            .ranges
            .partition_point(|hidden| hidden.end <= range.start);
        self.ranges
            .get(index)
            .is_some_and(|hidden| hidden.start < range.end)
    }
}

/// `%% ... %%` comments, which hide what they hold.
pub(crate) fn comments(source: &str, mask: &Mask) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = source[from..].find("%%") {
        let start = from + found;
        if mask.hides(start) {
            from = start + 2;
            continue;
        }
        let end = source[start + 2..]
            .find("%%")
            .map_or(source.len(), |close| start + 2 + close + 2);
        out.push(start..end);
        from = end;
    }
    out
}

/// Every wikilink and embed outside the mask.
pub(crate) fn wikilinks(source: &str, mask: &Mask) -> Vec<WikiSpan> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = source[from..].find("[[") {
        let open = from + found;
        from = open + 2;
        if mask.hides(open) {
            continue;
        }
        // The closing `]]` on the same line, with no `[[` before it.
        let inner_start = open + 2;
        let line_end = source[inner_start..]
            .find('\n')
            .map_or(source.len(), |at| inner_start + at);
        let Some(close) = source[inner_start..line_end].find("]]") else {
            continue;
        };
        let inner_end = inner_start + close;
        let inner = &source[inner_start..inner_end];
        if inner.contains("[[") || inner.trim().is_empty() {
            continue;
        }
        let embed = open > 0 && bytes[open - 1] == b'!' && !(open > 1 && bytes[open - 2] == b'\\');
        let start = if embed {
            open - 1
        } else {
            open
        };
        let span = start..inner_end + 2;
        if mask.touches(&span) {
            continue;
        }
        let (body, display) = match inner.find('|') {
            Some(bar) => (&inner[..bar], Some(inner[bar + 1..].trim().to_string())),
            None => (inner, None),
        };
        let (target, anchor) = match body.find('#') {
            Some(hash) => (
                &body[..hash],
                Some(body[hash + 1..].trim().to_string()).filter(|anchor| !anchor.is_empty()),
            ),
            None => (body, None),
        };
        let leading = target.len() - target.trim_start().len();
        let target_start = inner_start + leading;
        let target = target.trim();
        out.push(WikiSpan {
            span,
            target_span: target_start..target_start + target.len(),
            target: target.to_string(),
            anchor,
            display: display.filter(|text| !text.is_empty()),
            embed,
        });
        from = inner_end + 2;
    }
    out
}

/// Every `#tag` outside the mask and outside the given wikilinks: `#` at the
/// start or after whitespace or `(`, then letters, digits, `_`, `-`, `/`,
/// at least one of them not a digit.
pub(crate) fn tags(source: &str, mask: &Mask, links: &[WikiSpan]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (at, _) in source.match_indices('#') {
        if mask.hides(at) || links.iter().any(|link| link.span.contains(&at)) {
            continue;
        }
        let before = source[..at].chars().next_back();
        if before.is_some_and(|c| !(c.is_whitespace() || c == '(' || c == ',')) {
            continue;
        }
        let name: String = source[at + 1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))
            .collect();
        let name = name.trim_end_matches('/');
        if name.is_empty() || name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        out.push((at, name.to_string()));
    }
    out
}

/// A block anchor at the end of a line, ` ^id`: the id, if this line has one.
pub(crate) fn anchor(line: &str) -> Option<&str> {
    let line = line.trim_end();
    let caret = line.rfind('^')?;
    let id = &line[caret + 1..];
    let valid = !id.is_empty()
        && id.len() <= 32
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let spaced = caret == 0 || line[..caret].ends_with(char::is_whitespace);
    (valid && spaced).then_some(id)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn links(source: &str) -> Vec<WikiSpan> {
        wikilinks(source, &Mask::default())
    }

    #[test]
    fn reads_every_wikilink_form() {
        let found = links(
            "See [[Note]], [[Folder/Other.md|shown]] and ![[Pic.png]] or [[Page#Head|h]] [[#Local]] [[A#^b1]].",
        );
        let targets: Vec<_> = found
            .iter()
            .map(|link| {
                (
                    link.target.as_str(),
                    link.anchor.as_deref(),
                    link.display.as_deref(),
                    link.embed,
                )
            })
            .collect();
        assert_eq!(targets, vec![
            ("Note", None, None, false),
            ("Folder/Other.md", None, Some("shown"), false),
            ("Pic.png", None, None, true),
            ("Page", Some("Head"), Some("h"), false),
            ("", Some("Local"), None, false),
            ("A", Some("^b1"), None, false),
        ]);
    }

    #[test]
    fn target_spans_point_at_the_name() {
        let source = "x [[ Spaced repetition |s]] y";
        let link = &links(source)[0];
        assert_eq!(&source[link.target_span.clone()], "Spaced repetition");
    }

    #[test]
    fn ignores_unclosed_and_multiline_links() {
        assert!(links("[[open\nclosed]]").is_empty());
        assert!(links("[[]]").is_empty());
        assert_eq!(links("[[a [[b]]").len(), 1);
    }

    #[test]
    fn masked_spans_hide_links_and_tags() {
        let source = "`[[code]]` [[real]] #tag `#nottag`";
        let mut mask = Mask::default();
        mask.add(0..10);
        mask.add(25..34);
        mask.seal();
        let found = wikilinks(source, &mask);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].target, "real");
        let tags: Vec<_> = tags(source, &mask, &found)
            .into_iter()
            .map(|(_, tag)| tag)
            .collect();
        assert_eq!(tags, vec!["tag"]);
    }

    #[test]
    fn tags_need_a_boundary_and_a_letter() {
        let source =
            "#start mid#no (#paren) #2026 #y2026 # heading #a/b/ [[N#head]] https://x.io/#frag";
        let found = links(source);
        let tags: Vec<_> = tags(source, &Mask::default(), &found)
            .into_iter()
            .map(|(_, tag)| tag)
            .collect();
        assert_eq!(tags, vec!["start", "paren", "y2026", "a/b"]);
    }

    #[test]
    fn comments_run_to_their_close_or_the_end() {
        assert_eq!(comments("a %%x%% b %%y", &Mask::default()), vec![
            2..7,
            10..13
        ]);
    }

    #[test]
    fn anchors_end_a_line() {
        assert_eq!(anchor("- an idea ^g7k2"), Some("g7k2"));
        assert_eq!(anchor("x^not"), None);
        assert_eq!(anchor("text ^bad_id"), None);
        assert_eq!(anchor("^solo  "), Some("solo"));
    }
}
