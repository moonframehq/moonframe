//! Cutting text to a size, and writing durations and token counts.

/// `text`'s first `max` bytes, ending with `…` when cut.
pub(crate) fn head(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut at = max;
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    format!("{}…", &text[..at])
}

/// `text`'s last `max` bytes, starting with `…` when cut.
pub(crate) fn tail(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut at = text.len() - max;
    while !text.is_char_boundary(at) {
        at += 1;
    }
    format!("…{}", &text[at..])
}

/// The last `lines` lines of `text` that say something, at most `max`
/// bytes; `None` when nothing is left.
pub(crate) fn last_lines(text: &str, lines: usize, max: usize) -> Option<String> {
    if lines == 0 || max == 0 {
        return None;
    }
    let kept: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect();
    let from = kept.len().saturating_sub(lines);
    let shown = kept[from..].join("\n");
    (!shown.is_empty()).then(|| tail(&shown, max))
}

/// The first line of `text` that says something, at most `max` bytes.
pub(crate) fn first_line(text: &str, max: usize) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| head(line, max))
}

/// A duration as a person reads it: `850ms`, `12s`, `3m 05s`, `1h 02m`.
pub fn duration(ms: u64) -> String {
    let seconds = ms / 1000;
    match seconds {
        0 => format!("{ms}ms"),
        1..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h {:02}m", seconds / 3600, seconds % 3600 / 60),
    }
}

/// A token count, short: `950`, `12.4k`, `1.2M`.
pub fn tokens(count: u64) -> String {
    match count {
        0..1000 => count.to_string(),
        1000..1_000_000 => short(count, 1000, "k"),
        _ => short(count, 1_000_000, "M"),
    }
}

fn short(count: u64, unit: u64, suffix: &str) -> String {
    let tenths = count.saturating_mul(10) / unit;
    if tenths >= 1000 || tenths.is_multiple_of(10) {
        format!("{}{suffix}", tenths / 10)
    } else {
        format!("{}.{}{suffix}", tenths / 10, tenths % 10)
    }
}
