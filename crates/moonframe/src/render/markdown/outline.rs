//! What a document's structure says, read from its source: its headings,
//! their anchors, and where its footnotes are defined. Plain text in, so it
//! is tested without a window, and cheap enough to run off the UI thread.

use {
    ::markdown::{ParseOptions, mdast::Node, to_mdast},
    moonframe_core::Data,
    std::collections::{BTreeMap, HashSet},
};

/// One heading of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Heading {
    pub(super) level: u8,
    pub(super) text: String,
    /// Bytes into the source, for `reveal`.
    pub(super) at: usize,
    /// What a `#anchor` link to it says, as GitHub writes them.
    pub(super) slug: String,
    /// Which top-level block it is, for telling where a scroll has got to.
    pub(super) block: usize,
}

impl Heading {
    pub(super) fn data(&self) -> Data {
        Data::Map(
            [
                ("level".to_owned(), Data::Integer(i64::from(self.level))),
                ("text".to_owned(), Data::String(self.text.clone())),
                ("at".to_owned(), Data::Integer(self.at as i64)),
                ("slug".to_owned(), Data::String(self.slug.clone())),
            ]
            .into(),
        )
    }
}

/// A document's headings and footnotes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Outline {
    pub(super) headings: Vec<Heading>,
    /// Bytes into the source of each footnote's definition, by identifier.
    pub(super) footnotes: BTreeMap<String, usize>,
}

impl Outline {
    pub(super) fn parse(source: &str) -> Self {
        let mut options = ParseOptions::gfm();
        options.constructs.frontmatter = true;
        options.constructs.math_flow = true;
        options.constructs.math_text = true;
        let Ok(Node::Root(root)) = to_mdast(source, &options) else {
            return Self::default();
        };
        let mut outline = Self::default();
        let mut seen = HashSet::new();
        for (block, node) in root.children.iter().enumerate() {
            let at = node.position().map_or(0, |position| position.start.offset);
            match node {
                Node::Heading(heading) => {
                    let text = node.to_string();
                    outline.headings.push(Heading {
                        level: heading.depth,
                        slug: unique(slug(&text), &mut seen),
                        text,
                        at,
                        block,
                    });
                },
                Node::FootnoteDefinition(definition) => {
                    outline.footnotes.insert(definition.identifier.clone(), at);
                },
                _ => {},
            }
        }
        outline
    }

    /// The heading a `#anchor` link names, which may be spelled in any case.
    pub(super) fn anchor(&self, fragment: &str) -> Option<&Heading> {
        let fragment = fragment.to_lowercase();
        self.headings
            .iter()
            .find(|heading| heading.slug == fragment)
    }

    /// Which heading a reader at top-level block `block` is under, from 0.
    pub(super) fn section(&self, block: usize) -> Option<usize> {
        match self
            .headings
            .iter()
            .rposition(|heading| heading.block <= block)
        {
            Some(index) => Some(index),
            None if self.headings.is_empty() => None,
            None => Some(0),
        }
    }
}

/// A heading's text as an anchor: lower case, words joined by `-`, and
/// punctuation left out, though not letters of any script.
fn slug(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// `slug`, numbered if an earlier heading took it, as `intro-1`.
fn unique(slug: String, seen: &mut HashSet<String>) -> String {
    let mut candidate = slug.clone();
    let mut number = 0;
    while !seen.insert(candidate.clone()) {
        number += 1;
        candidate = format!("{slug}-{number}");
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_headings_with_their_places() {
        let source = "intro\n\n# One *two*\n\ntext\n\n## Three\n";
        let outline = Outline::parse(source);
        let found: Vec<_> = outline
            .headings
            .iter()
            .map(|heading| (heading.level, heading.text.as_str(), heading.slug.as_str()))
            .collect();
        assert_eq!(found, [(1, "One two", "one-two"), (2, "Three", "three")]);
        assert_eq!(
            source[outline.headings[1].at..].lines().next(),
            Some("## Three")
        );
    }

    #[test]
    fn a_hash_in_code_is_no_heading() {
        let outline = Outline::parse("```sh\n# a comment\n```\n");
        assert!(outline.headings.is_empty());
    }

    #[test]
    fn repeated_headings_get_their_own_anchors() {
        let outline = Outline::parse("# Notes\n\n# Notes\n\n# Notes\n");
        let slugs: Vec<_> = outline.headings.iter().map(|h| h.slug.as_str()).collect();
        assert_eq!(slugs, ["notes", "notes-1", "notes-2"]);
        assert_eq!(outline.anchor("Notes-1").map(|h| h.block), Some(1));
    }

    #[test]
    fn anchors_keep_letters_of_any_script() {
        assert_eq!(slug("Crème brûlée: 2 ways!"), "crème-brûlée-2-ways");
        assert_eq!(slug("日本語 の見出し"), "日本語-の見出し");
    }

    #[test]
    fn finds_footnote_definitions() {
        let source = "A note[^one].\n\n[^one]: The body.\n";
        let outline = Outline::parse(source);
        let at = outline.footnotes["one"];
        assert!(source[at..].starts_with("[^one]:"));
    }

    #[test]
    fn frontmatter_is_not_a_heading() {
        let outline = Outline::parse("---\ntitle: x\n---\n\n# Real\n");
        let texts: Vec<_> = outline.headings.iter().map(|h| h.text.as_str()).collect();
        assert_eq!(texts, ["Real"]);
    }

    #[test]
    fn the_section_is_the_last_heading_at_or_above() {
        let outline = Outline::parse("# A\n\ntext\n\n# B\n\ntext\n");
        assert_eq!(outline.section(0), Some(0));
        assert_eq!(outline.section(1), Some(0));
        assert_eq!(outline.section(2), Some(1));
        assert_eq!(Outline::parse("no headings").section(3), None);
    }
}
