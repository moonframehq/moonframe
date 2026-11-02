//! `app.usage`, from what herdr-usage last read. Shares are fractions
//! (0 to 1), as Moonframe's meters take them; times are Unix seconds, with
//! the countdowns already written out so a view needs no clock.
//!
//! ```text
//! { available, host, busy, updated?, age? (minutes), error?,
//!   providers = { provider },       -- the sheet's tabs, in registry order
//!   headline  = { id } }            -- those with numbers, tightest first
//! provider = { id, name, icon?, dashboard?, status_page?, email?, plan?,
//!   error?, access? ("needed" | "denied"), stale, used?, severity?,
//!   windows = { window }, balances = { balance }, sections = { section },
//!   daily? = { { day, amount } }, accounts? = { account } }
//! window = { title, kind, used, left, severity, label, resets_at?,
//!   resets_in?, pace?, pace_note? }
//! ```

use {
    herdr_usage::{
        Access, Host, Reading, Usage, UsageConfig,
        details::{ClaudeAccount, OpenaiDaily},
        model::{Balance, Kind, Report, Section, Severity, Unit, Window, countdown},
    },
    moonframe_core::IconName,
    serde_json::{Value, json},
    std::time::{SystemTime, UNIX_EPOCH},
};

/// Providers whose logo Moonframe bundles under another name than
/// `agent-<id>`.
const ICON_ALIASES: &[(&str, &str)] = &[
    // keep-sorted start
    ("factory", "agent-droid"),
    ("openai", "agent-codex"),
    ("qwencloud", "agent-qwen"),
    // keep-sorted end
];

pub(crate) fn usage(usage: &Usage, config: &UsageConfig, host: Option<&Host>) -> Value {
    let now = SystemTime::now();
    let entry = usage.current();
    let providers: Vec<Value> = entry
        .map(|entry| entry.tabs(config))
        .unwrap_or_default()
        .into_iter()
        .map(|reading| provider(reading, now))
        .collect();
    let headline: Vec<&str> = entry
        .map(|entry| entry.headline(None))
        .unwrap_or_default()
        .into_iter()
        .map(|reading| reading.provider.id())
        .collect();
    json!({
        "available": true,
        "host": host.map(crate::host_name),
        "busy": usage.busy(),
        "updated": entry.and_then(|entry| entry.updated).map(unix),
        // Whole minutes since then, for "Updated 8m ago": views have no clock.
        "age": entry
            .and_then(|entry| entry.updated)
            .map(|at| now.duration_since(at).map_or(0, |since| since.as_secs() / 60)),
        "error": entry.and_then(|entry| entry.error.clone()),
        "providers": providers,
        "headline": headline,
    })
}

fn provider(reading: &Reading, now: SystemTime) -> Value {
    let meta = reading.provider.service().meta();
    let report = reading.report.as_ref();
    let tightest = report.and_then(Report::tightest);
    let mut out = json!({
        "id": meta.id,
        "name": meta.name,
        "icon": icon(meta.id, meta.icon),
        "dashboard": meta.dashboard,
        "status_page": meta.status_page,
        "email": report.and_then(|report| report.account.email.clone()),
        "plan": report.and_then(|report| report.account.plan.clone()),
        "error": reading.error,
        "access": reading.access.map(|access| match access {
            Access::Needed => "needed",
            Access::Denied => "denied",
        }),
        // The numbers are the last good ones; the latest read failed.
        "stale": report.is_some() && reading.error.is_some(),
        "used": tightest.map(|window| fraction(window.used)),
        "severity": tightest.map(|window| severity(window.used)),
        "windows": report.map_or_else(Vec::new, |report| {
            report.windows.iter().map(|w| window(w, now)).collect()
        }),
        "balances": report.map_or_else(Vec::new, |report| {
            report.balances.iter().map(balance).collect()
        }),
        "sections": report.map_or_else(Vec::new, |report| {
            report.sections.iter().map(|s| section(s, now)).collect()
        }),
    });
    if let Some(daily) = report.and_then(Report::detail::<OpenaiDaily>) {
        out["daily"] = daily
            .days
            .iter()
            .map(|(day, amount)| json!({ "day": day, "amount": amount }))
            .collect();
    }
    if let Some(others) = report.and_then(Report::detail::<Vec<ClaudeAccount>>) {
        out["accounts"] = others.iter().map(|other| account(other, now)).collect();
    }
    out
}

/// A Moonframe icon name for the provider's logo, when the app bundles it.
pub(crate) fn icon(id: &str, path: Option<&str>) -> Option<&'static str> {
    let named = path
        .and_then(|path| path.strip_prefix("icons/"))
        .and_then(|name| name.strip_suffix(".svg"))
        .and_then(IconName::new);
    let alias = || {
        ICON_ALIASES
            .iter()
            .find(|(provider, _)| *provider == id)
            .and_then(|(_, name)| IconName::new(name))
    };
    named
        .or_else(alias)
        .or_else(|| IconName::new(&format!("agent-{id}")))
        .map(|name| name.name())
}

pub(crate) fn window(limit: &Window, now: SystemTime) -> Value {
    let pace = limit.pace(now);
    json!({
        "title": limit.kind.title(),
        "kind": match limit.kind {
            Kind::Session => "session",
            Kind::Daily => "daily",
            Kind::Weekly => "weekly",
            Kind::Monthly => "monthly",
            Kind::Named(_) => "named",
        },
        "used": fraction(limit.used),
        "left": fraction(100. - limit.used),
        "severity": severity(limit.used),
        "label": limit.label(now),
        "resets_at": limit.resets_at.map(unix),
        "resets_in": limit.resets_in(now).map(countdown),
        // Where an even spend would have left the bar of what is left:
        // `usage-bar`'s `pace`.
        "pace": pace.map(|pace| fraction(100. - pace.expected)),
        "pace_note": pace.map(|pace| pace.describe(limit.used)),
    })
}

fn balance(balance: &Balance) -> Value {
    let (unit, currency) = match &balance.unit {
        Unit::Currency(code) => (code.as_str(), true),
        Unit::Count(name) => (name.as_str(), false),
    };
    let left = balance
        .total
        .filter(|total| *total > 0.)
        .map(|total| (balance.amount / total).clamp(0., 1.));
    json!({
        "label": balance.label,
        "amount": balance.amount,
        "total": balance.total,
        "unit": unit,
        "currency": currency,
        "text": balance.text(),
        "left": left,
    })
}

fn section(section: &Section, now: SystemTime) -> Value {
    match section {
        Section::Limit(limit) => json!({ "kind": "limit", "window": window(limit, now) }),
        Section::Facts { title, facts } => json!({
            "kind": "facts",
            "title": title,
            "facts": facts
                .iter()
                .map(|(label, value)| json!({ "label": label, "value": value }))
                .collect::<Vec<_>>(),
        }),
        Section::Shares { title, shares } => json!({
            "kind": "shares",
            "title": title,
            "shares": shares
                .iter()
                .map(|(label, share)| json!({ "label": label, "share": fraction(*share) }))
                .collect::<Vec<_>>(),
        }),
    }
}

fn account(other: &ClaudeAccount, now: SystemTime) -> Value {
    json!({
        "name": other.name(),
        "email": other.email,
        "disabled": other.disabled,
        "stale": other.stale,
        "note": other.note(),
        "windows": other.windows.iter().map(|w| window(w, now)).collect::<Vec<_>>(),
    })
}

/// herdr's percent as a fraction, rounded to what a bar can show.
fn fraction(percent: f32) -> f64 {
    (f64::from(percent.clamp(0., 100.)) * 10.).round() / 1000.
}

fn severity(used: f32) -> &'static str {
    match Severity::from(used) {
        Severity::Normal => "normal",
        Severity::Warning => "warning",
        Severity::Critical => "critical",
    }
}

fn unix(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}
