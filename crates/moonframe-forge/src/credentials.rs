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
    crate::kind::Kind,
    herdr_credentials::{Entry, Store},
    secrecy::{ExposeSecret, SecretString},
    serde_json::{Map, Value, json},
    std::{
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    },
};

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
    /// came from there.
    pub fn save(&self, host: &str, credential: &Credential) -> Result<(), StoreError> {
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

    /// Forgets `host`'s sign-in.
    pub fn forget(&self, host: &str) -> Result<(), StoreError> {
        let mut records = self.records()?;
        if records.remove(host).is_some() {
            self.write(&records)?;
        }
        Ok(())
    }
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
pub fn cli(kind: Kind, host: &str, path: &std::ffi::OsStr) -> Option<Credential> {
    let (program, args) = kind.cli(host);
    let program = moonframe_providers::program::find(program, path)?;
    let output = cli_command(&program, kind, host, path)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.len() > 4096 {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?;
    let token = token.trim();
    plausible(token).then(|| Credential::new(SecretString::from(token.to_owned()), Source::Cli))
}

/// Resolve the chosen CLI account immediately before use. glab refreshes
/// OAuth credentials through its API transport, not `config get token`, so
/// it is asked once first. That call failing (offline, GitLab down) is not
/// a sign-out: the CLI's token is still used, and only the forge refusing
/// it signs the user out. `None` means the CLI has no sign-in at all.
pub fn cli_ready(kind: Kind, host: &str, path: &std::ffi::OsStr) -> Option<Credential> {
    if kind == Kind::GitLab {
        let program = moonframe_providers::program::find("glab", path)?;
        let renewed = cli_command(&program, kind, host, path)
            .args(["api", "user", "--hostname", host])
            .stdout(Stdio::null())
            .status();
        if !renewed.is_ok_and(|status| status.success()) {
            tracing::debug!("gitlab: glab could not reach {host}; using its saved token");
        }
    }
    cli(kind, host, path)
}

/// A CLI choice is resolved afresh rather than reusing a captured token.
pub fn for_fetch(
    kind: Kind,
    host: &str,
    path: &std::ffi::OsStr,
    credential: Credential,
) -> Option<Credential> {
    if credential.source == Source::Cli {
        cli_ready(kind, host, path)
    } else {
        Some(credential)
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
