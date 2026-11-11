//! Numbers and dates the way a language writes them, from the Unicode
//! data ICU4X builds in: `1,234.5` in English, `1 234,5` in French, and
//! `10 nov. 2026` where English writes `Nov 10, 2026`.

use {
    chrono::{Datelike as _, Local, TimeZone as _, Timelike as _},
    fluent_bundle::FluentValue,
    icu_datetime::{
        DateTimeFormatter,
        fieldsets::{
            builder::{DateFields, FieldSetBuilder},
            enums::CompositeDateTimeFieldSet,
        },
        input::{Date, DateTime, Time},
        options::{Length, TimePrecision},
    },
    icu_decimal::{DecimalFormatter, input::Decimal},
    intl_memoizer::{IntlLangMemoizer, Memoizable},
    std::{cell::RefCell, collections::BTreeMap},
    unic_langid::LanguageIdentifier,
};

/// The most digits after the point `ui.number` writes.
pub(crate) const MAX_DECIMALS: usize = 20;
/// What `ui.number` keeps after the point when not told: as many as there
/// are, up to this.
const DEFAULT_DECIMALS: usize = 3;

/// One language's formatters, made when first asked for.
pub(crate) struct Formats {
    locale: icu_locale_core::Locale,
    decimals: Option<DecimalFormatter>,
    dates: RefCell<BTreeMap<DateStyle, Option<DateTimeFormatter<CompositeDateTimeFieldSet>>>>,
}

/// What `ui.date` writes: the date, the time, or both, and how long.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DateStyle {
    pub(crate) length: DateLength,
    pub(crate) date: bool,
    pub(crate) time: Option<Clock>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DateLength {
    Short,
    #[default]
    Medium,
    Long,
}

/// How much of the time of day shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Clock {
    Minutes,
    Seconds,
}

impl Formats {
    pub(crate) fn new(language: &LanguageIdentifier) -> Self {
        let locale = icu_locale(language);
        let decimals = DecimalFormatter::try_new((&locale).into(), Default::default()).ok();
        Self {
            locale,
            decimals,
            dates: RefCell::default(),
        }
    }

    /// `value` with `decimals` digits after the point, or as many as it
    /// has up to three.
    pub(crate) fn number(&self, value: f64, decimals: Option<usize>) -> String {
        let digits = plain(value, decimals);
        self.decimal(&digits).unwrap_or(digits)
    }

    /// `digits`, a plain decimal such as `-1234.50`, the language's way.
    fn decimal(&self, digits: &str) -> Option<String> {
        let decimal = digits.parse::<Decimal>().ok()?;
        Some(self.decimals.as_ref()?.format(&decimal).to_string())
    }

    /// The moment `seconds` after 1970 (UTC), in the system's time zone.
    pub(crate) fn date(&self, seconds: f64, style: DateStyle) -> Option<String> {
        if !seconds.is_finite() {
            return None;
        }
        // Whole seconds: no format shows less.
        #[allow(clippy::cast_possible_truncation)]
        let local = Local.timestamp_opt(seconds.floor() as i64, 0).single()?;
        let date = Date::try_new_iso(
            local.year(),
            u8::try_from(local.month()).ok()?,
            u8::try_from(local.day()).ok()?,
        )
        .ok()?;
        let time = Time::try_new(
            u8::try_from(local.hour()).ok()?,
            u8::try_from(local.minute()).ok()?,
            u8::try_from(local.second()).ok()?,
            0,
        )
        .ok()?;
        let mut dates = self.dates.borrow_mut();
        let formatter = dates
            .entry(style)
            .or_insert_with(|| self.date_formatter(style))
            .as_ref()?;
        Some(formatter.format(&DateTime { date, time }).to_string())
    }

    fn date_formatter(
        &self,
        style: DateStyle,
    ) -> Option<DateTimeFormatter<CompositeDateTimeFieldSet>> {
        let mut builder = FieldSetBuilder::new();
        builder.length = Some(match style.length {
            DateLength::Short => Length::Short,
            DateLength::Medium => Length::Medium,
            DateLength::Long => Length::Long,
        });
        builder.date_fields = style.date.then_some(DateFields::YMD);
        builder.time_precision = style.time.map(|clock| match clock {
            Clock::Minutes => TimePrecision::Minute,
            Clock::Seconds => TimePrecision::Second,
        });
        let fields = builder.build_composite_datetime().ok()?;
        DateTimeFormatter::try_new((&self.locale).into(), fields).ok()
    }
}

/// `value` as a plain decimal, `-1234.5`, with `decimals` digits after
/// the point, or up to [`DEFAULT_DECIMALS`] with trailing zeros dropped.
fn plain(value: f64, decimals: Option<usize>) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    match decimals {
        Some(decimals) => format!("{value:.*}", decimals.min(MAX_DECIMALS)),
        None => {
            let fixed = format!("{value:.DEFAULT_DECIMALS$}");
            let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
            // Rounding `-0.0001` leaves `-0`.
            if trimmed == "-0" {
                "0"
            } else {
                trimmed
            }
            .to_owned()
        },
    }
}

fn icu_locale(language: &LanguageIdentifier) -> icu_locale_core::Locale {
    language
        .to_string()
        .parse()
        .unwrap_or(icu_locale_core::locale!("en-US"))
}

/// A number in a Fluent message, `{ $count }`, the language's way; what
/// `NUMBER($n, minimumFractionDigits: 2)` asks for stays.
pub(super) fn fluent_number(value: &FluentValue, memoizer: &IntlLangMemoizer) -> Option<String> {
    let FluentValue::Number(number) = value else {
        return None;
    };
    let digits = number.as_string();
    memoizer
        .with_try_get::<Decimals, _, _>((), |decimals| {
            let decimal = digits.parse::<Decimal>().ok()?;
            Some(decimals.0.as_ref()?.format(&decimal).to_string())
        })
        .ok()
        .flatten()
}

/// The decimal formatter a Fluent bundle keeps for its language.
struct Decimals(Option<DecimalFormatter>);

impl Memoizable for Decimals {
    type Args = ();
    type Error = std::convert::Infallible;

    fn construct(language: LanguageIdentifier, (): ()) -> Result<Self, Self::Error> {
        let locale = icu_locale(&language);
        Ok(Self(
            DecimalFormatter::try_new((&locale).into(), Default::default()).ok(),
        ))
    }
}
