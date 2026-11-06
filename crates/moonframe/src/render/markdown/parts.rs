//! Reading the pieces of a document the renderer has no block for: GitHub
//! alerts, and `$` signs that are math rather than money. Plain text in, so
//! it is tested without a window.

/// A GitHub alert's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl AlertKind {
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Important => "Important",
            Self::Warning => "Warning",
            Self::Caution => "Caution",
        }
    }

    fn read(marker: &str) -> Option<Self> {
        match marker.to_ascii_uppercase().as_str() {
            "NOTE" => Some(Self::Note),
            "TIP" => Some(Self::Tip),
            "IMPORTANT" => Some(Self::Important),
            "WARNING" => Some(Self::Warning),
            "CAUTION" => Some(Self::Caution),
            _ => None,
        }
    }
}

/// A block quote's source read as an alert: `> [!NOTE]` on the first line,
/// and the markdown under it with the quote marks taken off.
pub(super) fn alert(quote: &str) -> Option<(AlertKind, String)> {
    let mut lines = quote.lines().map(unquoted);
    let first = lines.next()?.trim();
    let kind = AlertKind::read(first.strip_prefix("[!")?.strip_suffix(']')?)?;
    Some((kind, lines.collect::<Vec<_>>().join("\n")))
}

/// A line of a block quote without its `>` and the space after it.
fn unquoted(line: &str) -> &str {
    let line = line.trim_start();
    let line = line.strip_prefix('>').unwrap_or(line);
    line.strip_prefix(' ').unwrap_or(line)
}

/// Whether `$...$` is math and not prose that happens to hold two dollar
/// signs ("$5 and $10"): as in Pandoc, its text hugs the signs, and no digit
/// follows the closing one.
#[cfg_attr(not(feature = "math"), allow(dead_code))]
pub(super) fn inline_math(value: &str, after: Option<char>) -> bool {
    !value.is_empty()
        && !value.starts_with(char::is_whitespace)
        && !value.ends_with(char::is_whitespace)
        && !after.is_some_and(|c| c.is_ascii_digit())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_alert_and_its_body() {
        let (kind, body) = alert("> [!WARNING]\n> Mind the **gap**.\n>\n> Twice.").unwrap();
        assert_eq!(kind, AlertKind::Warning);
        assert_eq!(body, "Mind the **gap**.\n\nTwice.");
    }

    #[test]
    fn the_marker_may_be_in_lower_case() {
        assert_eq!(alert("> [!tip]\n> x").unwrap().0, AlertKind::Tip);
    }

    #[test]
    fn other_quotes_are_not_alerts() {
        assert!(alert("> Just a quote").is_none());
        assert!(alert("> [!UNKNOWN]\n> x").is_none());
        assert!(alert("> [!NOTE] text on the same line").is_none());
    }

    #[test]
    fn dollars_in_prose_are_not_math() {
        assert!(inline_math("E = mc^2", Some(' ')));
        assert!(inline_math("x", None));
        assert!(!inline_math("5 and ", Some('1')));
        assert!(!inline_math(" x", None));
        assert!(!inline_math("5", Some('0')));
    }
}
