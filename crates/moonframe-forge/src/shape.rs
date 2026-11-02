//! What both forges' answers become: the `Forge` shape mods read (see
//! `types/moonframe.d.luau`), shared so a view draws GitHub and GitLab
//! alike. Only short strings and numbers go out.

use {
    serde_json::{Value, json},
    std::path::Path,
};

/// The longest title, name or label kept.
const TEXT: usize = 300;

/// `text` without control characters, cut to [`TEXT`] characters.
pub fn text(value: Option<&Value>) -> Value {
    match value.and_then(Value::as_str) {
        Some(text) => json!(
            text.chars()
                .map(|c| if c.is_control() {
                    ' '
                } else {
                    c
                })
                .take(TEXT)
                .collect::<String>()
        ),
        None => Value::Null,
    }
}

/// An `https` address, or nothing.
pub fn url(value: Option<&Value>) -> Value {
    match value.and_then(Value::as_str) {
        Some(url) if url.starts_with("https://") && url.len() <= 2048 => json!(url),
        _ => Value::Null,
    }
}

/// An ISO 8601 time (`2026-11-01T09:30:00Z`, `...+02:00`, fractions
/// allowed) as Unix seconds.
pub fn seconds(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_str)
        .and_then(parse_time)
        .map_or(Value::Null, |seconds| json!(seconds))
}

pub fn parse_time(text: &str) -> Option<i64> {
    let (date, time) = text.split_once('T')?;
    let mut date = date.split('-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let zone_at = time.find(['Z', 'z', '+', '-']).unwrap_or(time.len());
    let (clock, zone) = time.split_at(zone_at);
    let clock = clock.split('.').next()?;
    let mut clock = clock.split(':').map(str::parse::<i64>);
    let (hour, minute, second) = (
        clock.next()?.ok()?,
        clock.next()?.ok()?,
        clock.next().unwrap_or(Ok(0)).ok()?,
    );
    let offset = match zone.as_bytes().first() {
        Some(sign @ (b'+' | b'-')) => {
            let mut parts = zone[1..].split(':').map(str::parse::<i64>);
            let (hours, minutes) = (parts.next()?.ok()?, parts.next().unwrap_or(Ok(0)).ok()?);
            let offset = hours * 3600 + minutes * 60;
            if *sign == b'+' {
                offset
            } else {
                -offset
            }
        },
        _ => 0,
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days from the civil date, as in Howard Hinnant's algorithm.
    let year = if month <= 2 {
        year - 1
    } else {
        year
    };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    let days = era * 146_097 + of_cycle - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// A person: `{ login, name, bot, avatar_url, avatar_path }`.
pub fn person(
    login: &Value,
    name: &Value,
    bot: bool,
    avatar_url: &Value,
    avatars: &Avatars,
) -> Value {
    let Some(login_text) = login.as_str() else {
        return Value::Null;
    };
    json!({
        "login": login,
        "name": name,
        "bot": bot,
        "avatar_url": avatar_url,
        "avatar_path": avatars.path(login_text),
    })
}

/// Counts of checks by outcome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Checks {
    pub passed: u64,
    pub failed: u64,
    pub pending: u64,
    pub skipped: u64,
}

impl Checks {
    pub fn add(&mut self, outcome: &str) {
        self.add_count(outcome, 1);
    }

    pub fn add_count(&mut self, outcome: &str, count: u64) {
        match outcome {
            "passed" => self.passed += count,
            "failed" => self.failed += count,
            "skipped" => self.skipped += count,
            _ => self.pending += count,
        }
    }

    pub fn json(self) -> Value {
        json!({
            "total": self.passed + self.failed + self.pending + self.skipped,
            "passed": self.passed,
            "failed": self.failed,
            "pending": self.pending,
            "skipped": self.skipped,
        })
    }
}

/// What a pull request's badge says, from its state, review and checks:
/// the `tone` the `pr-badge` component colors.
pub struct Standing<'a> {
    pub state: &'a str,
    pub draft: bool,
    pub conflicts: bool,
    pub changes_requested: bool,
    pub review_required: bool,
    pub ready: bool,
    pub held: bool,
    pub checks: Checks,
}

impl Standing<'_> {
    pub fn tone(&self) -> &'static str {
        if self.state == "merged" {
            "merged"
        } else if self.state == "closed" {
            "closed"
        } else if self.draft {
            "draft"
        } else if self.conflicts || self.changes_requested || self.checks.failed > 0 {
            "attention"
        } else if self.checks.pending > 0 || self.review_required {
            "waiting"
        } else if self.ready {
            "ready"
        } else if self.held {
            "held"
        } else {
            "waiting"
        }
    }
}

/// The avatars downloaded so far, in the folder `ui.image` may read, and
/// which ones to fetch.
pub struct Avatars<'a> {
    pub folder: &'a Path,
    /// Prefixes the file name: `""` for github.com (as the old provider
    /// named them), `gitlab.com-` and so on for the others.
    pub prefix: String,
}

impl Avatars<'_> {
    /// Where `login`'s picture is (or would be), if `login` is a name
    /// that can be a file's.
    pub fn file(&self, login: &str) -> Option<std::path::PathBuf> {
        let safe = !login.is_empty()
            && login.len() <= 100
            && login
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            && !login.starts_with('.');
        safe.then(|| self.folder.join(format!("{}{login}.png", self.prefix)))
    }

    /// The picture's path once it is downloaded.
    pub fn path(&self, login: &str) -> Value {
        self.file(login)
            .filter(|file| file.metadata().is_ok_and(|meta| meta.len() > 0))
            .map_or(Value::Null, |file| json!(file.to_string_lossy()))
    }
}

impl Avatars<'_> {
    /// Downloads up to ten of `wanted` (login, picture address) that are
    /// not here yet; the next publish shows them.
    pub fn fetch(&self, agent: &ureq::Agent, wanted: &[(String, String)]) {
        let missing = wanted
            .iter()
            .filter(|(login, url)| {
                url.starts_with("https://") && self.file(login).is_some_and(|file| !file.exists())
            })
            .take(10);
        let mut made = false;
        for (login, url) in missing {
            let Some(file) = self.file(login) else {
                continue;
            };
            if !made {
                if std::fs::create_dir_all(self.folder).is_err() {
                    return;
                }
                made = true;
            }
            match crate::http::picture(agent, url) {
                Ok(bytes) if !bytes.is_empty() => {
                    let part = file.with_extension("png.part");
                    if std::fs::write(&part, bytes).is_ok()
                        && std::fs::rename(&part, &file).is_err()
                    {
                        let _ = std::fs::remove_file(&part);
                    }
                },
                Ok(_) => {},
                Err(failure) => tracing::debug!("avatar for {login}: {failure}"),
            }
        }
    }
}
