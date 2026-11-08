//! Dates as `YYYY-MM-DD` text, the way the views get them, with days and
//! months out of range rolling over as C's `mktime` rolls them.

use chrono::{Datelike, Local, NaiveDate, TimeDelta, TimeZone};

/// A date's year, month and day, from `YYYY-MM-DD` (a final newline allowed,
/// as Python's `\n?\Z` allowed it). Each part may be out of range.
pub(crate) fn split(date: &str) -> Option<(i64, i64, i64)> {
    let date = date.strip_suffix('\n').unwrap_or(date);
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let number = |part: &str| {
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())
            .flatten()
    };
    Some((
        number(&date[0..4])?,
        number(&date[5..7])?,
        number(&date[8..10])?,
    ))
}

/// The day `y-m-d` names, months and days rolling over. One past what a
/// four-digit year holds is the day of Unix time 0, as Python's overflow
/// was.
pub(crate) fn civil(y: i64, m: i64, d: i64) -> NaiveDate {
    let date = || {
        let months = y.checked_mul(12)?.checked_add(m.checked_sub(1)?)?;
        let year = i32::try_from(months.div_euclid(12)).ok()?;
        let month = u32::try_from(months.rem_euclid(12) + 1).ok()?;
        let first = NaiveDate::from_ymd_opt(year, month, 1)?;
        let date = first.checked_add_signed(TimeDelta::try_days(d.checked_sub(1)?)?)?;
        (0..=9999).contains(&date.year()).then_some(date)
    };
    date().unwrap_or_else(epoch)
}

fn epoch() -> NaiveDate {
    Local
        .timestamp_opt(0, 0)
        .single()
        .map_or_else(NaiveDate::default, |time| time.date_naive())
}

pub(crate) fn format(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// `YYYY-MM-DD` of the day `offset` days after `date`, or after `today`
/// when `date` is none or no date.
pub(crate) fn day(today: NaiveDate, date: Option<&str>, offset: i64) -> String {
    let (y, m, d) = date.and_then(split).unwrap_or((
        i64::from(today.year()),
        i64::from(today.month()),
        i64::from(today.day()),
    ));
    match d.checked_add(offset) {
        Some(d) => format(civil(y, m, d)),
        None => format(epoch()),
    }
}

/// The date, rolled over, of `YYYY-MM-DD` text.
pub(crate) fn parts(date: &str) -> Option<NaiveDate> {
    let (y, m, d) = split(date)?;
    Some(civil(y, m, d))
}

/// The date in `pattern` (strftime's), or "" for text that is no date.
pub(crate) fn label(date: &str, pattern: &str) -> String {
    parts(date).map_or_else(String::new, |date| date.format(pattern).to_string())
}

/// The last day of the month `y-m`.
pub(crate) fn last_day(y: i64, m: i64) -> i64 {
    i64::from(civil(y, m + 1, 0).day())
}
