//! The marks a writing field paints: markdown's spans as typed, and the
//! sentence or paragraph in focus. Plain text in, byte ranges out; every
//! range starts and ends at an ASCII mark or a line's start, so each is on
//! a character boundary.

use {moonframe_core::Dim, std::ops::Range};

/// What a span of text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mark {
    /// `#`, `**`, `>`, a list's bullet, a link's address: faded.
    Markup,
    /// A heading's line.
    Heading,
    Strong,
    Emphasis,
    /// A quote's text.
    Quote,
    /// Code, in a span or a fenced block.
    Code,
    /// A link's text.
    Link,
}

impl Mark {
    fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// The marks over one run of text, and whether it lies outside the focus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Look {
    marks: u8,
    pub(super) dimmed: bool,
}

impl Look {
    pub(super) fn has(self, mark: Mark) -> bool {
        self.marks & mark.bit() != 0
    }
}

/// The fenced code blocks of `text`, from the opening fence's line to the
/// end of the closing one's (or of the text, when one is never closed).
pub(super) fn fences(text: &str) -> Vec<Range<usize>> {
    let mut fences = Vec::new();
    let mut open = None;
    for line in lines(text, 0..text.len()) {
        let trimmed = text[line.clone()].trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            match open.take() {
                Some(start) => fences.push(start..line.end),
                None => open = Some(line.start),
            }
        }
    }
    if let Some(start) = open {
        fences.push(start..text.len());
    }
    fences
}

/// The lines of `text` that `range` touches, each without its newline.
fn lines(text: &str, range: Range<usize>) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut at = text[..range.start].rfind('\n').map_or(0, |i| i + 1);
    std::iter::from_fn(move || {
        if at > range.end || at > text.len() || (at == text.len() && at > range.start) {
            return None;
        }
        let end = text[at..].find('\n').map_or(text.len(), |i| at + i);
        let line = at..end;
        at = end + 1;
        Some(line)
    })
}

/// Adds the markdown marks of the lines `range` touches to `marks`.
pub(super) fn markdown(
    text: &str,
    range: Range<usize>,
    fences: &[Range<usize>],
    marks: &mut Vec<(Range<usize>, Mark)>,
) {
    for line in lines(text, range) {
        if let Some(fence) = fences.iter().find(|fence| fence.contains(&line.start)) {
            let trimmed = text[line.clone()].trim_start();
            let delimiter = (line.start == fence.start || line.end == fence.end)
                && (trimmed.starts_with("```") || trimmed.starts_with("~~~"));
            let mark = if delimiter {
                Mark::Markup
            } else {
                Mark::Code
            };
            marks.push((line, mark));
            continue;
        }
        block(text, line, marks);
    }
}

/// One line's marks: what starts it (a heading, a quote, a bullet), then its
/// inline spans.
fn block(text: &str, line: Range<usize>, marks: &mut Vec<(Range<usize>, Mark)>) {
    let bytes = text.as_bytes();
    let indent = text[line.clone()].len() - text[line.clone()].trim_start().len();
    let start = line.start + indent;
    let rest = &text[start..line.end];
    let hashes = rest.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && matches!(bytes.get(start + hashes), None | Some(b' ' | b'\n')) {
        marks.push((line.clone(), Mark::Heading));
        marks.push((start..start + hashes, Mark::Markup));
        return inline(text, start + hashes..line.end, marks);
    }
    if rule(rest) {
        marks.push((line, Mark::Markup));
        return;
    }
    if rest.starts_with('>') {
        marks.push((start..start + 1, Mark::Markup));
        marks.push((start + 1..line.end, Mark::Quote));
        return inline(text, start + 1..line.end, marks);
    }
    let bullet = match rest.as_bytes() {
        [b'-' | b'*' | b'+', b' ', ..] => 2,
        _ => {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            match rest.as_bytes().get(digits..digits + 2) {
                Some([b'.' | b')', b' ']) if digits > 0 => digits + 2,
                _ => 0,
            }
        },
    };
    let mut body = start + bullet;
    if bullet > 0 {
        let task = ["[ ] ", "[x] ", "[X] "]
            .iter()
            .any(|task| text[body..line.end].starts_with(task));
        if task {
            body += 4;
        }
        marks.push((start..body, Mark::Markup));
    }
    inline(text, body..line.end, marks);
}

/// Whether a line is a thematic break: three or more of one of `-`, `*`, `_`.
fn rule(line: &str) -> bool {
    let marks: Vec<u8> = line.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    marks.len() >= 3
        && matches!(marks[0], b'-' | b'*' | b'_')
        && marks.iter().all(|&b| b == marks[0])
}

/// The spans of a stretch of one line: code, strong, emphasis and links.
fn inline(text: &str, range: Range<usize>, marks: &mut Vec<(Range<usize>, Mark)>) {
    let bytes = text.as_bytes();
    let end = range.end;
    let mut at = range.start;
    let find = |from: usize, needle: &str| text[from..end].find(needle).map(|i| from + i);
    let word = |i: Option<&u8>| i.is_some_and(u8::is_ascii_alphanumeric);
    while at < end {
        match bytes[at] {
            b'`' => {
                if let Some(close) = find(at + 1, "`") {
                    marks.push((at..at + 1, Mark::Markup));
                    marks.push((at + 1..close, Mark::Code));
                    marks.push((close..close + 1, Mark::Markup));
                    at = close + 1;
                    continue;
                }
            },
            b'*' | b'_' if bytes.get(at + 1) == Some(&bytes[at]) => {
                let delimiter = &text[at..at + 2];
                if let Some(close) = find(at + 2, delimiter).filter(|&close| close > at + 2) {
                    marks.push((at..at + 2, Mark::Markup));
                    marks.push((at + 2..close, Mark::Strong));
                    marks.push((close..close + 2, Mark::Markup));
                    at = close + 2;
                    continue;
                }
            },
            mark @ (b'*' | b'_') => {
                // `snake_case` and `2*3*4` are words, not emphasis.
                let opens = !(mark == b'_' && at > 0 && word(bytes.get(at - 1)))
                    && !bytes.get(at + 1).is_some_and(u8::is_ascii_whitespace);
                let delimiter = &text[at..at + 1];
                let close = find(at + 1, delimiter).filter(|&close| {
                    close > at + 1
                        && !(mark == b'_' && word(bytes.get(close + 1)))
                        && !bytes[close - 1].is_ascii_whitespace()
                });
                if let Some(close) = close.filter(|_| opens) {
                    marks.push((at..at + 1, Mark::Markup));
                    marks.push((at + 1..close, Mark::Emphasis));
                    marks.push((close..close + 1, Mark::Markup));
                    at = close + 1;
                    continue;
                }
            },
            b'[' => {
                let link = find(at + 1, "](").and_then(|middle| {
                    let close = find(middle + 2, ")")?;
                    Some((middle, close))
                });
                if let Some((middle, close)) = link {
                    marks.push((at..at + 1, Mark::Markup));
                    marks.push((at + 1..middle, Mark::Link));
                    marks.push((middle..close + 1, Mark::Markup));
                    at = close + 1;
                    continue;
                }
            },
            _ => {},
        }
        at += 1;
    }
}

/// The part of `text` in focus with the caret at `cursor`: its sentence, or
/// its paragraph (the lines between blank ones).
pub(super) fn focus(text: &str, cursor: usize, dim: Dim) -> Range<usize> {
    let start = text[..cursor].rfind("\n\n").map_or(0, |i| i + 2);
    let end = text[cursor..]
        .find("\n\n")
        .map_or(text.len(), |i| cursor + i);
    let paragraph = start..end;
    match dim {
        Dim::Paragraph => paragraph,
        Dim::Sentence => sentence(text, cursor, paragraph),
    }
}

/// The sentence around `cursor` inside `paragraph`. A sentence ends after
/// `.`, `!` or `?` (and any closing quotes or brackets) before a space, or
/// at a line's end, so a heading or a list item is a sentence of its own.
fn sentence(text: &str, cursor: usize, paragraph: Range<usize>) -> Range<usize> {
    let bytes = text.as_bytes();
    let closes = |b: u8| matches!(b, b'"' | b'\'' | b')' | b']' | b'*' | b'_');
    // Where the sentence that ends at `i` (exclusive) stops: past its
    // closing marks, if `i` ends one.
    let ends_at = |i: usize| -> Option<usize> {
        if !matches!(bytes[i], b'.' | b'!' | b'?') {
            return None;
        }
        let mut after = i + 1;
        while after < paragraph.end && closes(bytes[after]) {
            after += 1;
        }
        (after == paragraph.end || bytes[after].is_ascii_whitespace()).then_some(after)
    };
    let mut start = paragraph.start;
    for i in (paragraph.start..cursor).rev() {
        if bytes[i] == b'\n' {
            start = i + 1;
            break;
        }
        if let Some(after) = ends_at(i).filter(|&after| after < cursor) {
            start = after;
            break;
        }
    }
    while start < cursor && bytes[start] == b' ' {
        start += 1;
    }
    let mut end = paragraph.end;
    for (i, &byte) in bytes.iter().enumerate().take(paragraph.end).skip(cursor) {
        if byte == b'\n' {
            end = i;
            break;
        }
        if let Some(after) = ends_at(i) {
            end = after;
            break;
        }
    }
    start..end
}

/// Runs covering `range` with the marks over each, and those outside
/// `focus` dimmed; neighbours that look the same are joined.
pub(super) fn paint(
    range: Range<usize>,
    marks: &[(Range<usize>, Mark)],
    focus: Option<Range<usize>>,
) -> Vec<(Range<usize>, Look)> {
    // Each mark opens and closes; a sweep over the points keeps a count
    // per kind, since marks of one kind may overlap.
    let clamp = |at: usize| at.clamp(range.start, range.end);
    let mut points: Vec<(usize, Option<(Mark, bool)>)> =
        vec![(range.start, None), (range.end, None)];
    for (span, mark) in marks {
        let (start, end) = (clamp(span.start), clamp(span.end));
        if start < end {
            points.push((start, Some((*mark, true))));
            points.push((end, Some((*mark, false))));
        }
    }
    if let Some(focus) = &focus {
        points.push((clamp(focus.start), None));
        points.push((clamp(focus.end), None));
    }
    points.sort_by_key(|(at, _)| *at);
    let mut counts = [0u32; 8];
    let mut runs: Vec<(Range<usize>, Look)> = Vec::new();
    let mut last = range.start;
    for (at, change) in points {
        if at > last {
            let marks = counts
                .iter()
                .enumerate()
                .filter(|(_, count)| **count > 0)
                .fold(0u8, |bits, (kind, _)| bits | 1 << kind);
            let dimmed = focus
                .as_ref()
                .is_some_and(|focus| last >= focus.end || at <= focus.start);
            let look = Look { marks, dimmed };
            match runs.last_mut() {
                Some((run, previous)) if *previous == look => run.end = at,
                _ => runs.push((last..at, look)),
            }
            last = at;
        }
        if let Some((mark, opens)) = change {
            let count = &mut counts[mark as usize];
            *count = if opens {
                *count + 1
            } else {
                count.saturating_sub(1)
            };
        }
    }
    runs
}

// Named in full, so Rune, which shares this file, finds them too.
#[cfg(test)]
#[path = "marks/tests.rs"]
mod tests;
