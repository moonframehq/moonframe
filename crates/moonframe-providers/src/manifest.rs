//! `provider.toml`: what a provider is, how often it ticks, and what it may
//! ask the host for.
//!
//! ```toml
//! kind = "wasm"            # or "process" with `command = ["bash", "github.sh"]`,
//!                          # or "native": the Rust one the app built in
//!                          # under this folder's name
//! interval = 1.0           # seconds between ticks (wasm)
//! capabilities = ["system"]
//!
//! [settings]               # handed to the provider's `start`
//! history = 16
//! ```

use {
    moonframe_core::Error,
    serde::Deserialize,
    std::{fs, path::Path, time::Duration},
};

/// A wasm provider's prebuilt component, which ships in its folder.
pub const PREBUILT: &str = "provider.wasm";

const MIN_INTERVAL: f64 = 0.1;
const MAX_INTERVAL: f64 = 3600.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub kind: Kind,
    /// When it starts.
    pub start: Start,
    pub interval: Duration,
    pub capabilities: Vec<Capability>,
    /// The `[settings]` table, as JSON for the provider's `start`.
    pub settings: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A Rust crate in the folder, built to a WebAssembly component.
    Wasm,
    /// A command run in the folder.
    Process { command: Vec<String> },
    /// Rust the app was built with, registered under the folder's name.
    Native,
}

/// When a provider starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Start {
    /// Once something wants it: a view reads `app.<name>`, a mod hears its
    /// topic, or a mod sends it a message. The default, so an app pays for
    /// none it doesn't use.
    #[default]
    OnDemand,
    /// With the app, whether or not anything reads it yet.
    Always,
}

/// What a wasm provider may ask the host for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// This machine's CPU, memory, and network load.
    System,
}

/// The file as written, checked into a [`Manifest`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct File {
    kind: String,
    #[serde(default)]
    start: Start,
    #[serde(default)]
    command: Vec<String>,
    interval: Option<f64>,
    #[serde(default)]
    capabilities: Vec<Capability>,
    #[serde(default)]
    settings: toml::Table,
}

impl Manifest {
    pub fn load(name: &str, dir: &Path) -> Result<Self, Error> {
        let problem = Error::new;
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
        {
            return Err(problem(
                "use a-z, 0-9, - and _ in provider names".to_owned(),
            ));
        }
        let path = dir.join("provider.toml");
        let text = fs::read_to_string(&path)
            .map_err(|error| problem(format!("cannot read {}: {error}", path.display())))?;
        let file: File = toml::from_str(&text).map_err(|error| problem(error.to_string()))?;
        let kind = match (file.kind.as_str(), file.command.is_empty()) {
            ("wasm", true) => Kind::Wasm,
            ("wasm", false) => {
                return Err(problem("a wasm provider takes no `command`".to_owned()));
            },
            ("process", false) => Kind::Process {
                command: file.command,
            },
            ("process", true) => {
                return Err(problem("a process provider needs a `command`".to_owned()));
            },
            ("native", true) => Kind::Native,
            ("native", false) => {
                return Err(problem("a native provider takes no `command`".to_owned()));
            },
            (other, _) => {
                return Err(problem(format!(
                    "unknown kind `{other}`; expected wasm, process or native"
                )));
            },
        };
        if kind == Kind::Wasm && !dir.join(PREBUILT).is_file() && !dir.join("Cargo.toml").is_file()
        {
            return Err(problem(format!(
                "a wasm provider needs a {PREBUILT}, or a Cargo.toml to build one, beside provider.toml"
            )));
        }
        if kind != Kind::Wasm && !file.capabilities.is_empty() {
            return Err(problem(
                "capabilities are for wasm providers; the others have the user's own access"
                    .to_owned(),
            ));
        }
        let interval = file.interval.unwrap_or(1.0);
        if !(MIN_INTERVAL..=MAX_INTERVAL).contains(&interval) {
            return Err(problem(format!(
                "`interval` is {MIN_INTERVAL} to {MAX_INTERVAL} seconds"
            )));
        }
        let settings =
            serde_json::to_value(file.settings).map_err(|error| problem(error.to_string()))?;
        Ok(Self {
            kind,
            start: file.start,
            interval: Duration::from_secs_f64(interval),
            capabilities: file.capabilities,
            settings,
        })
    }
}
