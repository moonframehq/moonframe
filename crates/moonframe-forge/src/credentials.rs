//! Where a sign-in comes from and where it is kept.
//!
//! A token is found, in order, in what this app saved (a device sign-in or
//! a pasted token, in the system keyring, or a private `0600` file where
//! the build has none), in the environment (`GH_TOKEN`, `GITLAB_TOKEN`),
//! and in the forge's CLI (`gh`, `glab`), but only once the user chose it:
//! that choice is saved instead of the token, which is read from the CLI
//! again each time. Tokens stay in [`SecretString`]s and never reach a
//! mod, the log or published data.

use {
    crate::{http::Failure, kind::Kind},
    herdr_credentials::{Entry, Store},
    secrecy::{ExposeSecret, SecretString},
    serde_json::{Map, Value, json},
    std::{
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    },
};

mod command;
use command::run_for;

/// Where a token came from, as `auth.source` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    // keep-sorted start
    /// The forge's CLI, which the user chose.
    Cli,
    /// `GH_TOKEN`, `GITHUB_TOKEN` or `GITLAB_TOKEN`.
    Env,
    /// A device sign-in or a pasted token this app saved.
    Saved,
    // keep-sorted end
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::Env => "env",
            Self::Cli => "cli",
        }
    }
}

/// A token, and how to renew it when it expires (GitLab's OAuth tokens
/// last two hours).
pub struct Credential {
    pub token: SecretString,
    pub refresh: Option<SecretString>,
    /// Unix seconds.
    pub expires: Option<u64>,
    pub source: Source,
}

impl Clone for Credential {
    fn clone(&self) -> Self {
        Self {
            token: self.token.clone(),
            refresh: self.refresh.clone(),
            expires: self.expires,
            source: self.source,
        }
    }
}

impl Credential {
    pub fn new(token: SecretString, source: Source) -> Self {
        Self {
            token,
            refresh: None,
            expires: None,
            source,
        }
    }

    /// Whether it runs out within a minute and can be renewed.
    pub fn expiring(&self) -> bool {
        self.refresh.is_some() && self.expires.is_some_and(|expires| expires <= now() + 60)
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Why a saved sign-in could not be read or written.
#[derive(Debug)]
pub struct StoreError(pub String);

impl From<herdr_credentials::Error> for StoreError {
    fn from(error: herdr_credentials::Error) -> Self {
        Self(error.to_string())
    }
}

/// The largest saved record.
const LIMIT: usize = 64 * 1024;

fn validate(record: &SecretString) -> Result<(), StoreError> {
    let text = record.expose_secret();
    if text.len() > LIMIT
        || !serde_json::from_str::<Value>(text).is_ok_and(|value| value.is_object())
    {
        return Err(StoreError(
            "the saved sign-in is not a record this app wrote".into(),
        ));
    }
    Ok(())
}

const GITHUB: Entry<StoreError> = Entry {
    service: "Moonframe",
    account: "github",
    label: "Moonframe GitHub sign-in",
    file: c"github-sign-in.json",
    validate,
};

const GITLAB: Entry<StoreError> = Entry {
    service: "Moonframe",
    account: "gitlab",
    label: "Moonframe GitLab sign-in",
    file: c"gitlab-sign-in.json",
    validate,
};

/// The sign-ins this app saved for one forge, one per host, in the
/// keyring (or a private file).
pub struct Saved {
    kind: Kind,
    store: Store,
    directory: PathBuf,
    /// A renewal this app holds but could not save: the store still has
    /// the pair it replaced, which must not be taken for another app's.
    unsaved: std::sync::Mutex<Option<Unsaved>>,
}

/// See [`Saved::unsaved`].
struct Unsaved {
    host: String,
    /// The refresh token the store still holds, spent by the renewal.
    spent: SecretString,
    fresh: Credential,
}

fn same_refresh(a: &Credential, b: &Credential) -> bool {
    match (&a.refresh, &b.refresh) {
        (Some(a), Some(b)) => a.expose_secret() == b.expose_secret(),
        _ => false,
    }
}

impl Saved {
    /// `plaintext` lets a Linux desktop without a Secret Service keep the
    /// sign-in in a private file; macOS builds that cannot use the
    /// Keychain always do.
    pub fn new(kind: Kind, directory: PathBuf, plaintext: bool) -> Self {
        Self {
            kind,
            store: Store::for_policy(plaintext),
            directory,
            unsaved: std::sync::Mutex::new(None),
        }
    }

    fn entry(&self) -> &'static Entry<StoreError> {
        match self.kind {
            Kind::GitHub => &GITHUB,
            Kind::GitLab => &GITLAB,
        }
    }

    fn records(&self) -> Result<Map<String, Value>, StoreError> {
        if self.store == Store::File {
            create_private(&self.directory)?;
        }
        let record = herdr_credentials::read_entry(self.store, self.entry(), &self.directory)?;
        Ok(record
            .and_then(|record| serde_json::from_str::<Value>(record.expose_secret()).ok())
            .and_then(|value| match value {
                Value::Object(records) => Some(records),
                _ => None,
            })
            .unwrap_or_default())
    }

    fn write(&self, records: &Map<String, Value>) -> Result<(), StoreError> {
        if self.store == Store::File {
            create_private(&self.directory)?;
        }
        let record = (!records.is_empty())
            .then(|| SecretString::from(Value::Object(records.clone()).to_string()));
        herdr_credentials::save_entry(self.store, self.entry(), &self.directory, record.as_ref())
    }

    /// What was saved for `host`: a token, or the choice of the CLI.
    pub fn load(&self, host: &str) -> Result<Option<Remembered>, StoreError> {
        let records = self.records()?;
        let Some(record) = records.get(host) else {
            return Ok(None);
        };
        if record.get("source").and_then(Value::as_str) == Some("cli") {
            return Ok(Some(Remembered::Cli));
        }
        let text = |key: &str| {
            record
                .get(key)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(|text| SecretString::from(text.to_owned()))
        };
        Ok(text("token").map(|token| {
            Remembered::Token(Credential {
                token,
                refresh: text("refresh"),
                expires: record.get("expires").and_then(Value::as_u64),
                source: Source::Saved,
            })
        }))
    }

    /// Saves `credential` for `host`, or the choice of the CLI when it
    /// came from there, in turn with renewals and sign-outs.
    pub fn save(&self, host: &str, credential: &Credential) -> Result<(), StoreError> {
        self.locked(|| {
            self.write_record(host, credential)?;
            self.forget_unsaved(host);
            Ok(())
        })
    }

    /// [`Saved::save`] for a caller already holding the lock.
    fn write_record(&self, host: &str, credential: &Credential) -> Result<(), StoreError> {
        let mut records = self.records()?;
        let record = match credential.source {
            Source::Cli => json!({ "source": "cli" }),
            Source::Saved | Source::Env => {
                let mut record = json!({ "token": credential.token.expose_secret() });
                if let Some(refresh) = &credential.refresh {
                    record["refresh"] = json!(refresh.expose_secret());
                }
                if let Some(expires) = credential.expires {
                    record["expires"] = json!(expires);
                }
                record
            },
        };
        records.insert(host.to_owned(), record);
        self.write(&records)
    }

    /// Forgets `host`'s sign-in, in turn with any renewal (see
    /// [`Saved::renew`]).
    pub fn forget(&self, host: &str) -> Result<(), StoreError> {
        self.locked(|| {
            let mut records = self.records()?;
            if records.remove(host).is_some() {
                self.write(&records)?;
            }
            self.forget_unsaved(host);
            Ok(())
        })
    }

    /// Runs `work` holding this forge's sign-in lock, a file beside the
    /// private one, so apps sharing a sign-in renew and forget it in turn.
    /// Without a lock to take (no folder), it runs anyway.
    fn locked<T>(&self, work: impl FnOnce() -> T) -> T {
        let file = create_private(&self.directory).ok().and_then(|()| {
            let name = format!("{}-sign-in.lock", self.kind.name());
            std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(self.directory.join(name))
                .ok()
        });
        let held = file.as_ref().is_some_and(|file| file.lock().is_ok());
        let result = work();
        if held && let Some(file) = &file {
            let _ = file.unlock();
        }
        result
    }

    /// Renews `current`, the sign-in this app holds for `host`, unless
    /// another app sharing it renewed it first (GitLab spends a refresh
    /// token as it hands out the next, so only one may use it) or it was
    /// signed out meanwhile. A renewal is saved before the lock is let go.
    pub fn renew(
        &self,
        host: &str,
        current: &Credential,
        renew: impl FnOnce(&Credential) -> Result<Credential, Failure>,
    ) -> Renewal {
        self.locked(|| {
            let mut saved = match self.load(host) {
                Ok(Some(Remembered::Token(saved))) => saved,
                Ok(Some(Remembered::Cli) | None) => return Renewal::Gone,
                Err(_) => {
                    // Keep the whole unsaved chain if storage is temporarily
                    // unreadable, not just the last pair it spent.
                    let spent = self
                        .unsaved
                        .lock()
                        .ok()
                        .and_then(|pending| {
                            pending
                                .as_ref()
                                .filter(|pending| {
                                    pending.host == host && same_refresh(&pending.fresh, current)
                                })
                                .map(|pending| pending.spent.clone())
                        })
                        .or_else(|| current.refresh.clone());
                    return self.rotate(host, current, spent, renew);
                },
            };
            // Ours: the pair the store holds, or a renewal of it this app
            // could not save, which it saves now.
            let ours =
                same_refresh(&saved, current) || self.retry_unsaved(host, &mut saved, current);
            let current = if ours {
                current
            } else {
                self.forget_unsaved(host);
                if !saved.expiring() {
                    return Renewal::Newer(saved);
                }
                // Newer does not mean usable: both apps may have slept
                // through this pair's lifetime. Renew it under the same lock.
                &saved
            };
            self.rotate(host, current, saved.refresh.clone(), renew)
        })
    }

    /// Rotate while holding the store lock, remembering the persisted
    /// predecessor if either reading or writing storage fails.
    fn rotate(
        &self,
        host: &str,
        current: &Credential,
        spent: Option<SecretString>,
        renew: impl FnOnce(&Credential) -> Result<Credential, Failure>,
    ) -> Renewal {
        let renewed = match renew(current) {
            Ok(renewed) => renewed,
            Err(failure) => return Renewal::Failed(failure),
        };
        if let Err(problem) = self.write_record(host, &renewed) {
            tracing::warn!(
                "{}: cannot save the renewed sign-in, trying again later: {}",
                self.kind.name(),
                problem.0
            );
            if let (Ok(mut unsaved), Some(spent)) = (self.unsaved.lock(), spent) {
                *unsaved = Some(Unsaved {
                    host: host.to_owned(),
                    spent,
                    fresh: renewed.clone(),
                });
            }
        } else {
            self.forget_unsaved(host);
        }
        Renewal::Renewed(renewed)
    }

    /// Whether `current` is this app's unsaved renewal of `saved`; if so,
    /// saves it now.
    fn retry_unsaved(&self, host: &str, saved: &mut Credential, current: &Credential) -> bool {
        let Ok(mut unsaved) = self.unsaved.lock() else {
            return false;
        };
        let pending = unsaved.as_ref().is_some_and(|pending| {
            pending.host == host
                && saved
                    .refresh
                    .as_ref()
                    .is_some_and(|refresh| refresh.expose_secret() == pending.spent.expose_secret())
                && same_refresh(&pending.fresh, current)
        });
        if pending
            && let Some(fresh) = unsaved.as_ref().map(|pending| pending.fresh.clone())
            && self.write_record(host, &fresh).is_ok()
        {
            *saved = fresh;
            *unsaved = None;
        }
        pending
    }

    fn forget_unsaved(&self, host: &str) {
        if let Ok(mut unsaved) = self.unsaved.lock()
            && unsaved.as_ref().is_some_and(|pending| pending.host == host)
        {
            *unsaved = None;
        }
    }
}

/// What [`Saved::renew`] did.
pub enum Renewal {
    // keep-sorted start
    /// A refusal or a retryable failure; only a refusal signs out.
    Failed(Failure),
    /// Signed out meanwhile: nothing to renew.
    Gone,
    /// Another app renewed it first: its token, as saved.
    Newer(Credential),
    /// Renewed, and saved (or kept to save at the next renewal).
    Renewed(Credential),
    // keep-sorted end
}

/// What [`Saved`] holds for a host.
pub enum Remembered {
    // keep-sorted start
    Cli,
    Token(Credential),
    // keep-sorted end
}

fn create_private(directory: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|error| StoreError(format!("cannot make {}: {error}", directory.display())))
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(directory)
            .map_err(|error| StoreError(format!("cannot make {}: {error}", directory.display())))
    }
}

/// A token in the environment. Use it only for a host
/// [`Kind::env_serves`] allows.
pub fn environment(kind: Kind) -> Option<Credential> {
    kind.env().iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|token| token.trim().to_owned())
            .filter(|token| plausible(token))
            .map(|token| Credential::new(SecretString::from(token), Source::Env))
    })
}

/// The token the forge's CLI is signed in with on `host`, if it is
/// installed and signed in. Runs a program: call it off the provider's
/// thread.
pub fn cli(kind: Kind, host: &str, path: &std::ffi::OsStr) -> Result<Option<Credential>, Failure> {
    let (program, args) = kind.cli(host);
    let program = moonframe_providers::program::find(program, path)
        .ok_or_else(|| Failure::Network("the forge CLI is not installed".into()))?;
    let mut command = cli_command(&program, kind, host, path);
    command.args(args);
    let (succeeded, stdout) = run_within(command, true)?;
    if !succeeded {
        return Err(Failure::Network(
            "the forge CLI could not read its sign-in".into(),
        ));
    }
    let token = String::from_utf8(stdout)
        .map_err(|_| Failure::Body("the forge CLI returned an invalid token".into()))?;
    let token = token.trim();
    if token.is_empty() {
        return Ok(None);
    }
    if !plausible(token) {
        return Err(Failure::Body(
            "the forge CLI returned an invalid token".into(),
        ));
    }
    Ok(Some(Credential::new(
        SecretString::from(token.to_owned()),
        Source::Cli,
    )))
}

/// Resolve the chosen CLI account immediately before use. glab refreshes
/// OAuth credentials through its API transport, not `config get token`, so
/// it is asked once first. A failed call is retryable, not a sign-out:
/// its saved token may have expired while OAuth is temporarily unavailable.
pub fn cli_ready(
    kind: Kind,
    host: &str,
    path: &std::ffi::OsStr,
) -> Result<Option<Credential>, Failure> {
    if kind == Kind::GitLab {
        let program = moonframe_providers::program::find("glab", path)
            .ok_or_else(|| Failure::Network("glab is not installed".into()))?;
        let mut command = cli_command(&program, kind, host, path);
        command.args(["api", "user", "--hostname", host]);
        if !run_within(command, false)?.0 {
            return Err(Failure::Network("glab could not check its sign-in".into()));
        }
    }
    cli(kind, host, path)
}

/// How long a CLI may take: one stalled on the network or on its
/// credential store must not leave a sign-in checking, or a refresh
/// running, for good.
const CLI_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Runs `command` for at most [`CLI_TIMEOUT`] (`run_for`: `timeout`): whether it succeeded, and
/// what it printed (at most 8 KiB) when `capture`, or a retryable error.
fn run_within(command: Command, capture: bool) -> Result<(bool, Vec<u8>), Failure> {
    run_for(command, capture, CLI_TIMEOUT)
}

/// A CLI choice is resolved afresh rather than reusing a captured token.
pub fn for_fetch(
    kind: Kind,
    host: &str,
    path: &std::ffi::OsStr,
    credential: Credential,
) -> Result<Credential, Failure> {
    if credential.source == Source::Cli {
        cli_ready(kind, host, path)?.ok_or(Failure::Unauthorized)
    } else {
        Ok(credential)
    }
}

fn cli_command(program: &Path, kind: Kind, host: &str, path: &std::ffi::OsStr) -> Command {
    let mut command = Command::new(program);
    command
        .env("PATH", path)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GITLAB_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    // glab's environment overrides can otherwise bypass our host scoping.
    if kind == Kind::GitLab && !kind.env_serves(host) {
        for name in ["GITLAB_TOKEN", "GITLAB_ACCESS_TOKEN", "OAUTH_TOKEN"] {
            command.env_remove(name);
        }
    }
    command
}

/// Whether `token` could be a token: printable, no spaces, not absurdly
/// long. The forge has the last word.
pub fn plausible(token: &str) -> bool {
    (8..=1024).contains(&token.len()) && token.bytes().all(|byte| byte.is_ascii_graphic())
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
