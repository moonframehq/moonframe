//! The `usage` native provider: the plan usage of the AI services signed in
//! on this machine (Claude, Codex, Copilot, Cursor, Gemini, OpenRouter and
//! the rest sheprd-usage knows), shared as `app.usage`, for a status bar
//! meter and a usage sheet.
//!
//! sheprd-usage does the reading: it finds each agent's own sign-in, asks
//! its service on a thread of its own, keeps the last good numbers when a
//! refresh fails, and spaces reads out (every 10 minutes, longer when
//! rate limited). This crate drives it from the provider's ticks and
//! publishes what it has in the shape `publish` documents.
//!
//! Messages:
//! - `{ allow = "<provider id>" }`: the user pressed Allow for a sign-in
//!   behind a macOS Keychain prompt; it reads again at once so macOS asks,
//!   and the grant is kept across launches (denying it there takes it back);
//! - `{ host = "local" | "ssh:<target>" | "wsl:<distro>" }` reads another
//!   machine's sign-ins, over SSH or through `wsl.exe`, with its own
//!   credentials; only `local`, `[settings] host`, and the machines listed
//!   in `[settings] hosts`, so a mod cannot point SSH anywhere else;
//! - anything else refreshes, at most once every 10 seconds.
//!
//! Only the mods named in provider.toml's `writers` setting may send
//! `allow` or `host`: one makes macOS ask for a Keychain item, the other
//! connects over SSH with the user's keys. Any mod may refresh.
//!
//! provider.toml's `[settings]` are herdr's `[usage]` table:
//! `show_providers`, `hide_providers`, `browser_cookies`, and
//! `providers.<id>.<name>` for a service's key or cookie (each also read
//! from its environment variable, which keeps secrets out of the file).

mod grants;
mod publish;
mod words;

#[cfg(test)]
mod tests;

use {
    grants::Grants,
    moonframe_core::Data,
    moonframe_providers::{Handle, Mods, Native},
    serde_json::{Map, Value},
    sheprd_usage::{Host, Usage, UsageConfig, registry},
    std::time::Instant,
};

/// The keys of `[settings]` that are herdr's `[usage]` table; the rest are
/// this provider's own (`host`, `hosts`, `grants`, `writers`).
const USAGE_KEYS: &[&str] = &[
    "browser_cookies",
    "hide_providers",
    "providers",
    "show_providers",
];

/// The keys of `[settings]` this provider reads itself.
const OWN_KEYS: &[&str] = &[
    // keep-sorted start
    "grants", "host", "hosts", "writers",
    // keep-sorted end
];

/// Makes the provider, for the app's registry.
pub fn native() -> Box<dyn Native> {
    Box::new(UsageProvider::default())
}

#[derive(Default)]
pub struct UsageProvider {
    usage: Usage,
    config: UsageConfig,
    host: Option<Host>,
    grants: Grants,
    /// The mods that may send `allow` and `host` (`[settings] writers`).
    writers: Mods,
    /// The machines a `host` message may name besides `local`: `[settings]
    /// host` and `hosts`.
    hosts: Vec<Host>,
    /// What was published last, so a tick that changed nothing sends
    /// nothing.
    last: Option<Value>,
}

impl Native for UsageProvider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let settings = moonframe_providers::to_json(host.settings());
        self.config = config(&settings)?;
        self.host = Some(match settings.get("host").and_then(Value::as_str) {
            Some(name) => parse_host(name)?,
            None => Host::Local,
        });
        self.grants = Grants::load(grants::path(&settings));
        self.writers = Mods::from_settings(host.settings(), "writers");
        self.hosts = settings
            .get("hosts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(parse_host)
            .collect::<Result<_, _>>()?;
        self.hosts.extend(self.host.clone());
        self.tick(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        self.usage.poll(
            self.host.clone(),
            &self.config,
            self.grants.granted(),
            0,
            true,
            Instant::now(),
        );
        for provider in self.usage.take_denied() {
            self.grants.revoke(provider);
        }
        let data = publish::usage(&self.usage, &self.config, self.host.as_ref());
        if self.last.as_ref() != Some(&data) {
            host.publish_json(&data);
            self.last = Some(data);
        }
    }

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        let message = moonframe_providers::to_json(&message);
        let now = Instant::now();
        let asks = ["allow", "host"]
            .into_iter()
            .find(|key| message.get(*key).is_some());
        if let Some(key) = asks
            && !self.writers.allows(from)
        {
            host.fail(format!(
                "`{from}` may not send `{key}`: only the mods in providers/usage's `writers` may"
            ));
            return;
        }
        if let Some(id) = message.get("allow").and_then(Value::as_str) {
            let Some(provider) = registry::find(id) else {
                host.fail(format!(
                    "{from} asked to allow `{id}`, which is no provider"
                ));
                return;
            };
            self.grants.grant(provider);
            self.usage.allow(provider, now);
        } else if let Some(name) = message.get("host").and_then(Value::as_str) {
            match parse_host(name) {
                Ok(next) if next == Host::Local || self.hosts.contains(&next) => {
                    self.host = Some(next);
                },
                Ok(_) => {
                    host.fail(format!(
                        "{from}: host `{name}` is not in providers/usage's `hosts`"
                    ));
                    return;
                },
                Err(problem) => {
                    host.fail(format!("{from}: {problem}"));
                    return;
                },
            }
        } else {
            self.usage.refresh(now);
        }
        self.tick(host);
    }
}

/// sheprd-usage's config from `[settings]`, refusing what it does not
/// know so a typo says so instead of showing nothing.
fn config(settings: &Value) -> Result<UsageConfig, String> {
    let entries = settings.as_object().into_iter().flatten();
    if let Some((key, _)) = entries
        .clone()
        .find(|(key, _)| !USAGE_KEYS.contains(&key.as_str()) && !OWN_KEYS.contains(&key.as_str()))
    {
        return Err(format!(
            "settings: unknown key `{key}`; expected one of {}",
            [USAGE_KEYS, OWN_KEYS].concat().join(", ")
        ));
    }
    let table: Map<String, Value> = entries
        .filter(|(key, _)| USAGE_KEYS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let mut config: UsageConfig = serde_json::from_value(Value::Object(table))
        .map_err(|error| format!("settings: {error}"))?;
    for unknown in config.retain_known() {
        tracing::warn!(setting = %unknown, "usage: no such provider or setting");
    }
    Ok(config)
}

/// `local`, `ssh:<target>` or `wsl:<distro>`. sheprd-client checks the
/// target again before running anything; this refuses early what could
/// read as an option to `ssh` or `wsl.exe`, so the mod hears why.
pub(crate) fn parse_host(name: &str) -> Result<Host, String> {
    let plain = |word: &str| {
        !word.is_empty()
            && !word.starts_with('-')
            && word.len() <= 255
            && !word.chars().any(|c| c.is_whitespace() || c.is_control())
    };
    match name.split_once(':') {
        None if name == "local" => Ok(Host::Local),
        Some(("ssh", target)) if plain(target) => Ok(Host::Ssh(target.to_owned())),
        Some(("wsl", distro)) if plain(distro) => Ok(Host::Wsl(distro.to_owned())),
        _ => Err(format!(
            "host `{name}`: expected `local`, `ssh:<target>` or `wsl:<distro>`"
        )),
    }
}

/// How `host` is named in `app.usage.host`, as `parse_host` reads it.
pub(crate) fn host_name(host: &Host) -> String {
    match host {
        Host::Local => "local".to_owned(),
        Host::Ssh(target) => format!("ssh:{target}"),
        Host::Wsl(distro) => format!("wsl:{distro}"),
    }
}
