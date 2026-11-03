//! `provider.toml`: what a provider is, how often it ticks, and what it may
//! ask the host for.
//!
//! ```toml
//! kind = "wasm"            # or "process" with `command = ["bash", "herdr.sh"]`,
//!                          # or "native": the Rust one the app built in
//!                          # under this folder's name, or "static" with
//!                          # `file = "sample.json"`: that file, published
//! interval = 1.0           # seconds between ticks (wasm)
//! capabilities = ["system"]
//! publish = "latest"       # keep only the newest, for readings that repeat
//! urgent = ["stop"]        # messages that jump the queue (process, socket)
//! latest = ["exposure"]    # messages the next of that name replaces (same)
//! stop-grace = 2.0         # seconds a process has once stdin closes
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
/// How long a process has once its stdin closes, unless it says.
const STOP_GRACE: f64 = 0.25;
const MAX_STOP_GRACE: f64 = 30.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub kind: Kind,
    /// When it starts.
    pub start: Start,
    pub interval: Duration,
    pub capabilities: Vec<Capability>,
    /// The `[settings]` table, as JSON for the provider's `start`.
    pub settings: serde_json::Value,
    /// What becomes of a publish the app has not got to yet.
    pub publish: Publish,
    /// Messages that jump the queue, by name (process and socket).
    pub urgent: Vec<String>,
    /// Messages the next of that name replaces while waiting (the same).
    pub latest: Vec<String>,
    /// How long a process has to leave once its stdin closes.
    pub stop_grace: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A Rust crate in the folder, built to a WebAssembly component.
    Wasm,
    /// A command run in the folder.
    Process { command: Vec<String> },
    /// A program that runs on its own, reached over a Unix socket.
    Socket { path: String },
    /// Rust the app was built with, registered under the folder's name.
    Native,
    /// A JSON file in the folder, published as it is: no code runs.
    Static { file: String },
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

/// What becomes of a publish the app has not got to yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Publish {
    /// It is delivered, each in turn: mods that hear the provider's topic
    /// see every one.
    #[default]
    All,
    /// The next replaces it: however fast the provider publishes, the app
    /// only ever has the newest to handle. For readings, where only the
    /// last one matters.
    Latest,
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
    file: Option<String>,
    socket: Option<String>,
    interval: Option<f64>,
    #[serde(default)]
    publish: Publish,
    #[serde(default)]
    urgent: Vec<String>,
    #[serde(default)]
    latest: Vec<String>,
    stop_grace: Option<f64>,
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
            ("static", true) => Kind::Static {
                file: file
                    .file
                    .clone()
                    .ok_or_else(|| problem("a static provider needs a `file`".to_owned()))?,
            },
            ("static", false) => {
                return Err(problem("a static provider takes no `command`".to_owned()));
            },
            ("socket", true) => Kind::Socket {
                path: file
                    .socket
                    .clone()
                    .filter(|path| !path.is_empty())
                    .ok_or_else(|| problem("a socket provider needs a `socket`".to_owned()))?,
            },
            ("socket", false) => {
                return Err(problem("a socket provider takes no `command`".to_owned()));
            },
            (other, _) => {
                return Err(problem(format!(
                    "unknown kind `{other}`; expected wasm, process, socket, native or static"
                )));
            },
        };
        match &kind {
            Kind::Static { file: name } => {
                // A name in the folder: never a path out of it.
                if name.is_empty()
                    || name.starts_with('.')
                    || name.contains(['/', '\\'])
                    || Path::new(name).is_absolute()
                {
                    return Err(problem(format!(
                        "`file` is a file name in the provider's folder, not `{name}`"
                    )));
                }
                // Never a link: it could reach any file the user can read, and
                // a static provider is installed without asking.
                match fs::symlink_metadata(dir.join(name)) {
                    Ok(meta) if meta.is_file() => {},
                    Ok(meta) if meta.file_type().is_symlink() => {
                        return Err(problem(format!(
                            "{name} is a link; a static provider publishes a file of its own"
                        )));
                    },
                    _ => {
                        return Err(problem(format!("there is no {name} beside provider.toml")));
                    },
                }
                if file.interval.is_some() {
                    return Err(problem(
                        "a static provider publishes its file as it changes; it takes no `interval`"
                            .to_owned(),
                    ));
                }
            },
            _ if file.file.is_some() => {
                return Err(problem("only a static provider takes a `file`".to_owned()));
            },
            _ => {},
        }
        if file.socket.is_some() && !matches!(kind, Kind::Socket { .. }) {
            return Err(problem(
                "only a socket provider takes a `socket`".to_owned(),
            ));
        }
        if !matches!(kind, Kind::Process { .. } | Kind::Socket { .. })
            && !(file.urgent.is_empty() && file.latest.is_empty())
        {
            return Err(problem(
                "`urgent` and `latest` are for process and socket providers".to_owned(),
            ));
        }
        if matches!(kind, Kind::Static { .. }) && file.publish != Publish::All {
            return Err(problem(
                "a static provider publishes its file; it takes no `publish`".to_owned(),
            ));
        }
        if file.stop_grace.is_some() && !matches!(kind, Kind::Process { .. }) {
            return Err(problem(
                "only a process provider takes a `stop-grace`".to_owned(),
            ));
        }
        let stop_grace = file.stop_grace.unwrap_or(STOP_GRACE);
        if !(0.0..=MAX_STOP_GRACE).contains(&stop_grace) {
            return Err(problem(format!(
                "`stop-grace` is 0 to {MAX_STOP_GRACE} seconds"
            )));
        }
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
            publish: file.publish,
            urgent: file.urgent,
            latest: file.latest,
            stop_grace: Duration::from_secs_f64(stop_grace),
        })
    }
}
