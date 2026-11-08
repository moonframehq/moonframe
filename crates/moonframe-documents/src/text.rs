//! What the list shows of a document: its title, excerpt, word count and
//! when it changed, read from the markdown as it is.

use {
    chrono::{DateTime, Datelike, Days, Local},
    std::time::SystemTime,
};

/// Characters of a document shown under its title.
const EXCERPT: usize = 140;

/// Characters of a title.
const TITLE: usize = 80;

/// `line` without the indent and marker that open it: a heading, a quote or
/// a bullet, and with `numbered` a list number too.
fn unmarked(line: &str, numbered: bool) -> &str {
    let indent = line.chars().take(3).take_while(|c| c.is_whitespace());
    let rest = &line[indent.map(char::len_utf8).sum::<usize>()..];
    let spaced = |after: &str| -> Option<usize> {
        let space = after.len() - after.trim_start().len();
        (space > 0).then_some(space)
    };
    let hashes = rest.chars().take_while(|c| *c == '#').count();
    let marker = match rest.chars().next() {
        Some('#') if hashes <= 6 => spaced(&rest[hashes..]).map(|space| hashes + space),
        Some('>') => Some(1 + rest[1..].len() - rest[1..].trim_start().len()),
        Some('-' | '*' | '+') => spaced(&rest[1..]).map(|space| 1 + space),
        Some(c) if numbered && c.is_numeric() => {
            let digits: usize = rest
                .chars()
                .take_while(|c| c.is_numeric())
                .map(char::len_utf8)
                .sum();
            rest[digits..]
                .strip_prefix('.')
                .and_then(spaced)
                .map(|space| digits + 1 + space)
        },
        _ => None,
    };
    marker.map_or(line, |length| &rest[length..])
}

/// The first line with text, without its markup, or `doc`.
pub(crate) fn title(body: &str, doc: &str) -> String {
    body.lines()
        .map(|line| unmarked(line, false).trim())
        .find(|line| !line.is_empty())
        .map_or_else(
            || doc.to_owned(),
            |line| {
                line.chars()
                    .filter(|c| !matches!(c, '*' | '_' | '`'))
                    .take(TITLE)
                    .collect()
            },
        )
}

/// `text` without inline markup: emphasis and heading marks go, and a link
/// leaves its words.
fn plain(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '['
            && let Some((words, after)) = rest[1..].split_once(']')
            && let Some(after) = after.strip_prefix('(')
            && let Some((_, after)) = after.split_once(')')
        {
            plain.push_str(words);
            rest = after;
            continue;
        }
        if !matches!(c, '*' | '_' | '`' | '#' | '>') {
            plain.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    plain
}

/// What follows the title, on one line, cut at [`EXCERPT`] characters.
pub(crate) fn excerpt(body: &str) -> String {
    let lines: Vec<&str> = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .skip(1)
        .map(|line| unmarked(line, true).trim())
        .collect();
    let rest = plain(&lines.join(" "));
    let cut: String = rest.chars().take(EXCERPT).collect();
    let cut = cut.trim_end();
    if rest.chars().count() > EXCERPT {
        format!("{cut}…")
    } else {
        cut.to_owned()
    }
}

/// How many words `body` has: runs of letters, digits and underscores.
pub(crate) fn words(body: &str) -> usize {
    body.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .count()
}

/// When a file changed, as the list says it.
pub(crate) fn when(stamp: SystemTime) -> String {
    when_at(stamp.into(), Local::now())
}

fn when_at(time: DateTime<Local>, now: DateTime<Local>) -> String {
    let (day, today) = (time.date_naive(), now.date_naive());
    if day == today {
        time.format("Today, %H:%M").to_string()
    } else if today.checked_sub_days(Days::new(1)) == Some(day) {
        time.format("Yesterday, %H:%M").to_string()
    } else if day.year() == today.year() {
        time.format("%b %-d").to_string()
    } else {
        time.format("%b %-d, %Y").to_string()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, chrono::TimeZone};

    #[test]
    fn a_title_is_the_first_line_without_its_markup() {
        assert_eq!(title("\n# Hello *there*\nmore", "doc"), "Hello there");
        assert_eq!(title("  - a `list`", "doc"), "a list");
        assert_eq!(title("> quoted", "doc"), "quoted");
        assert_eq!(title("####### seven", "doc"), "####### seven");
        assert_eq!(title("    # indented", "doc"), "# indented");
        assert_eq!(title("  \n\n", "doc"), "doc");
        assert_eq!(title(&"é".repeat(100), "doc").chars().count(), 80);
    }

    #[test]
    fn an_excerpt_is_what_follows_on_one_line() {
        assert_eq!(
            excerpt("# Title\n\n1. **first**\n- [a link](https://x) and [not one]\n> quote"),
            "first a link and [not one] quote"
        );
        assert_eq!(excerpt("only a title"), "");
        let long = excerpt(&format!("t\n{}", "é ".repeat(100)));
        assert!(long.ends_with('…'));
        assert_eq!(long.chars().count(), 140);
    }

    #[test]
    fn words_are_counted_across_scripts() {
        assert_eq!(words("one two_three, café 4"), 4);
        assert_eq!(words(""), 0);
    }

    #[test]
    fn dates_read_relative_to_today() {
        let at = |y, m, d, h| Local.with_ymd_and_hms(y, m, d, h, 2, 0).unwrap();
        let now = at(2026, 3, 9, 12);
        assert_eq!(when_at(at(2026, 3, 9, 8), now), "Today, 08:02");
        assert_eq!(when_at(at(2026, 3, 8, 8), now), "Yesterday, 08:02");
        assert_eq!(when_at(at(2026, 1, 2, 8), now), "Jan 2");
        assert_eq!(when_at(at(2025, 1, 2, 8), now), "Jan 2, 2025");
    }
}
