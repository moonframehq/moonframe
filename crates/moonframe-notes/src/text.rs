//! Small text helpers shared by the parser, the index and the writer:
//! line numbers from byte offsets, the keys names are matched by, hashes,
//! and note ids.

use {
    sha2::{Digest, Sha256},
    std::time::{SystemTime, UNIX_EPOCH},
};

/// Byte offsets of each line's start, to turn an offset into a line number.
pub(crate) struct Lines {
    starts: Vec<usize>,
}

impl Lines {
    pub(crate) fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
        Self { starts }
    }

    /// The 1-based line holding byte `offset`.
    pub(crate) fn line(&self, offset: usize) -> u32 {
        let index = match self.starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        u32::try_from(index + 1).unwrap_or(u32::MAX)
    }

    /// How many lines there are (a final line break starts no new one).
    pub(crate) fn count(&self) -> u32 {
        u32::try_from(self.starts.len()).unwrap_or(u32::MAX)
    }

    /// The byte where 1-based line `line` starts.
    pub(crate) fn start(&self, line: u32) -> usize {
        self.starts
            .get((line as usize).saturating_sub(1))
            .copied()
            .unwrap_or(0)
    }

    /// The text of 1-based line `line`, without its line break.
    pub(crate) fn text<'a>(&self, source: &'a str, line: u32) -> &'a str {
        let index = (line as usize).saturating_sub(1);
        let Some(&start) = self.starts.get(index) else {
            return "";
        };
        let end = self
            .starts
            .get(index + 1)
            .map_or(source.len(), |next| next - 1);
        source.get(start..end).unwrap_or("").trim_end_matches('\r')
    }
}

/// The key a name is matched by: lowercase, `/` separators, no `.md`, no
/// surrounding spaces. Obsidian matches note names without regard to case.
pub fn key(name: &str) -> String {
    let name = name.trim().replace('\\', "/");
    let name = name.trim_start_matches("./").trim_start_matches('/');
    let name = strip_md(name);
    name.to_lowercase()
}

/// `name` without a trailing `.md` (any case).
pub(crate) fn strip_md(name: &str) -> &str {
    if name.len() >= 3
        && name.is_char_boundary(name.len() - 3)
        && name[name.len() - 3..].eq_ignore_ascii_case(".md")
    {
        &name[..name.len() - 3]
    } else {
        name
    }
}

/// The file name of a vault path without folders or `.md`: a note's title.
pub fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    strip_md(name)
}

/// The hex SHA-256 of `text`, identifying a version of a file.
pub fn digest(text: &str) -> String {
    digest_bytes(text.as_bytes())
}

/// The hex SHA-256 of `bytes`.
pub(crate) fn digest_bytes(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in hash {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// A new ULID: 48 bits of milliseconds then 80 random bits, in Crockford's
/// base 32, so ids sort by creation time.
pub fn ulid() -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let random = u128::from_be_bytes(*uuid::Uuid::new_v4().as_bytes()) & ((1u128 << 80) - 1);
    let mut value = ((millis & ((1u128 << 48) - 1)) << 80) | random;
    let mut out = [0u8; 26];
    for slot in out.iter_mut().rev() {
        *slot = ALPHABET[(value & 31) as usize];
        value >>= 5;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// An ISO 8601 time (`2026-08-03T02:16:56.000Z`, `2026-08-03T04:16:56+02:00`,
/// `2026-08-03`) as a system time; None when it does not read as one.
pub fn parse_time(text: &str) -> Option<SystemTime> {
    let text = text.trim();
    let number =
        |range: std::ops::Range<usize>| text.get(range).and_then(|part| part.parse::<i64>().ok());
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    if text.get(4..5) != Some("-")
        || text.get(7..8) != Some("-")
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
    {
        return None;
    }
    let (mut seconds, mut rest) = (0, "");
    if text.len() > 10 {
        if !matches!(text.get(10..11), Some("T" | " ")) || text.get(13..14) != Some(":") {
            return None;
        }
        let (hour, minute) = (number(11..13)?, number(14..16)?);
        let second = if text.get(16..17) == Some(":") {
            number(17..19)?
        } else {
            0
        };
        seconds = hour * 3600 + minute * 60 + second;
        let after = if text.get(16..17) == Some(":") {
            19
        } else {
            16
        };
        rest = text.get(after..).unwrap_or("");
        // Fractions of a second do not move a file's date.
        if let Some(fraction) = rest.strip_prefix('.') {
            rest = fraction.trim_start_matches(|c: char| c.is_ascii_digit());
        }
    }
    let offset = match rest {
        "" | "Z" | "z" => 0,
        zone => {
            let sign = if zone.starts_with('-') {
                -1
            } else if zone.starts_with('+') {
                1
            } else {
                return None;
            };
            let digits: String = zone[1..].chars().filter(char::is_ascii_digit).collect();
            let hours = digits.get(0..2)?.parse::<i64>().ok()?;
            let minutes = digits
                .get(2..4)
                .map_or(Some(0), |part| part.parse::<i64>().ok())?;
            sign * (hours * 3600 + minutes * 60)
        },
    };
    // Days from 1970-01-01 to the date (Howard Hinnant's civil-from-days, reversed).
    let y = if month <= 2 {
        year - 1
    } else {
        year
    };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let unix = days * 86_400 + seconds - offset;
    let unix = u64::try_from(unix).ok()?;
    Some(UNIX_EPOCH + std::time::Duration::from_secs(unix))
}

/// Up to `max` characters of `text`, cut at a character boundary, with an
/// ellipsis when cut.
pub(crate) fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    match text.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", text[..at].trim_end()),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn lines_map_offsets() {
        let text = "one\ntwo\r\nthree";
        let lines = Lines::new(text);
        assert_eq!(lines.line(0), 1);
        assert_eq!(lines.line(4), 2);
        assert_eq!(lines.line(text.len()), 3);
        assert_eq!(lines.text(text, 2), "two");
        assert_eq!(lines.text(text, 3), "three");
        assert_eq!(lines.text(text, 9), "");
    }

    #[test]
    fn keys_ignore_case_and_extension() {
        assert_eq!(key("Notes/Spaced Repetition.md"), "notes/spaced repetition");
        assert_eq!(key("  ./Café  "), "café");
        assert_eq!(stem("wiki/Forgetting curve.md"), "Forgetting curve");
        assert_eq!(stem("x.MD"), "x");
    }

    #[test]
    fn ulids_are_26_sortable_characters() {
        let first = ulid();
        let second = ulid();
        assert_eq!(first.len(), 26);
        assert!(first[..10] <= second[..10]);
        assert_ne!(first, second);
    }

    #[test]
    fn times_parse_with_zones() {
        let secs = |text: &str| {
            parse_time(text).map(|time| time.duration_since(UNIX_EPOCH).unwrap().as_secs())
        };
        assert_eq!(secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(secs("2026-08-03T02:16:56.000Z"), Some(1_785_723_416));
        assert_eq!(secs("2026-08-03T04:16:56+02:00"), Some(1_785_723_416));
        assert_eq!(secs("2026-08-03"), Some(1_785_715_200));
        assert_eq!(secs("2000-02-29T12:00:00Z"), Some(951_825_600));
        assert_eq!(secs("not a date"), None);
        assert_eq!(secs("2026-13-03"), None);
    }

    #[test]
    fn clip_cuts_on_characters() {
        assert_eq!(clip("héllo world", 5), "héllo…");
        assert_eq!(clip("short", 10), "short");
    }
}
