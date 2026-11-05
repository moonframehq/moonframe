//! Finding text in a rendered document: plain text in, byte ranges out.

use std::ops::Range;

/// Most matches marked in one document: past this a one-letter search
/// costs more to paint than it can help.
const MOST: usize = 5000;

/// Where `query` shows in `text`, ignoring case, without overlaps. Ranges
/// are bytes of `text`, on character boundaries.
pub(super) fn matches(text: &str, query: &str) -> Vec<Range<usize>> {
    let needle: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    // Each lower-cased character with the source character it came from:
    // lower-casing can change a character's length, and even their number.
    let mut folded: Vec<(char, usize, usize)> = Vec::with_capacity(text.len());
    for (start, c) in text.char_indices() {
        let end = start + c.len_utf8();
        folded.extend(c.to_lowercase().map(|lower| (lower, start, end)));
    }
    let mut found = Vec::new();
    let mut index = 0;
    while index + needle.len() <= folded.len() && found.len() < MOST {
        let window = &folded[index..index + needle.len()];
        if window.iter().map(|(c, ..)| c).eq(needle.iter()) {
            found.push(window[0].1..window[window.len() - 1].2);
            index += needle.len();
        } else {
            index += 1;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_case_and_keeps_to_the_source_bytes() {
        let text = "Zoë and ZOË";
        let found = matches(text, "zoë");
        let words: Vec<_> = found.iter().map(|range| &text[range.clone()]).collect();
        assert_eq!(words, ["Zoë", "ZOË"]);
    }

    #[test]
    fn matches_do_not_overlap() {
        assert_eq!(matches("aaaa", "aa"), [0..2, 2..4]);
    }

    #[test]
    fn nothing_to_find_finds_nothing() {
        assert!(matches("text", "").is_empty());
        assert!(matches("text", "xyz").is_empty());
    }

    #[test]
    fn a_character_that_lower_cases_to_two_stays_whole() {
        // `İ` lower-cases to `i` and a combining dot.
        let text = "İx";
        assert_eq!(matches(text, "x").first(), Some(&(2..3)));
        assert_eq!(matches(text, "i").first(), Some(&(0..2)));
    }

    #[test]
    fn stops_at_the_limit() {
        assert_eq!(matches(&"a".repeat(MOST + 10), "a").len(), MOST);
    }
}
