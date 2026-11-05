//! The engines that turn a take into text, as `[settings.engines]` names
//! them. Each is a kind with its own module; adding one means a variant
//! here and a `check` and `transcribe` of its own.

// keep-sorted start
#[cfg_attr(not(target_os = "macos"), path = "engine/apple_left_out.rs")]
mod apple;
mod command;
mod remote;
// keep-sorted end

use {
    moonframe_core::Data,
    moonframe_providers::to_json,
    serde_json::{Value, json},
    std::{
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    },
};

/// Set when the user cancels, so a slow engine stops waiting.
#[derive(Clone, Default)]
pub(crate) struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub(crate) fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub(crate) fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug)]
enum Kind {
    Apple { language: Option<String> },
    Command(command::Command),
    Remote(remote::Remote),
}

#[derive(Clone, Debug)]
pub(crate) struct Engine {
    pub(crate) name: String,
    title: String,
    kind: Kind,
}

/// Whether an engine can run now, and if not, what the user can do.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Status {
    pub(crate) name: String,
    pub(crate) ready: bool,
    pub(crate) reason: Option<String>,
}

impl Engine {
    fn read(name: &str, value: &Value) -> Result<Self, String> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let kind = match text("kind").as_deref() {
            Some("apple") => Kind::Apple {
                language: text("language"),
            },
            Some("command") => Kind::Command(command::Command::read(value)?),
            Some("remote") => Kind::Remote(remote::Remote::read(value)?),
            Some(other) => return Err(format!("unknown kind `{other}`")),
            None => return Err("needs a `kind`".into()),
        };
        Ok(Self {
            name: name.to_owned(),
            title: text("title").unwrap_or_else(|| name.to_owned()),
            kind,
        })
    }

    /// Whether this engine sends the user's voice off the machine.
    fn remote(&self) -> bool {
        matches!(self.kind, Kind::Remote(_))
    }

    fn kind_name(&self) -> &'static str {
        match self.kind {
            Kind::Apple { .. } => "apple",
            Kind::Command(_) => "command",
            Kind::Remote(_) => "remote",
        }
    }

    /// May take a moment: it can ask the login shell for a key.
    pub(crate) fn check(&self) -> Status {
        let problem = match &self.kind {
            Kind::Apple { language } => apple::check(language.as_deref()),
            Kind::Command(command) => command.check(),
            Kind::Remote(remote) => remote.check(),
        };
        Status {
            name: self.name.clone(),
            ready: problem.is_none(),
            reason: problem,
        }
    }

    /// The words in the WAV file at `file`.
    pub(crate) fn transcribe(&self, file: &Path, cancel: &Cancel) -> Result<String, String> {
        let text = match &self.kind {
            Kind::Apple { language } => apple::transcribe(file, language.as_deref(), cancel),
            Kind::Command(command) => command.transcribe(file, cancel),
            Kind::Remote(remote) => remote.transcribe(file),
        }?;
        Ok(text.trim().to_owned())
    }

    pub(crate) fn publish(&self, status: Option<&Status>) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "kind": self.kind_name(),
            "remote": self.remote(),
            "ready": status.is_some_and(|status| status.ready),
            "reason": status.and_then(|status| status.reason.clone()),
        })
    }
}

/// What provider.toml says about engines.
#[derive(Clone, Debug, Default)]
pub(crate) struct Engines {
    pub(crate) all: Vec<Engine>,
    /// The engines a start without `engine` may use, best first. Never
    /// filled in by itself: sending the voice elsewhere is the user's call.
    prefer: Vec<String>,
    /// Entries that could not be read, for the log.
    pub(crate) problems: Vec<String>,
}

impl Engines {
    pub(crate) fn read(settings: &Data) -> Self {
        let settings = to_json(settings);
        let mut engines = Self::default();
        if let Some(table) = settings.get("engines").and_then(Value::as_object) {
            for (name, value) in table {
                match Engine::read(name, value) {
                    Ok(engine) => engines.all.push(engine),
                    Err(problem) => engines.problems.push(format!("engine {name}: {problem}")),
                }
            }
        }
        if let Some(list) = settings.get("prefer").and_then(Value::as_array) {
            engines.prefer = list
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
        }
        engines
    }

    pub(crate) fn named(&self, name: &str) -> Option<&Engine> {
        self.all.iter().find(|engine| engine.name == name)
    }

    /// The first preferred engine that is ready.
    pub(crate) fn default_engine(&self, statuses: &[Status]) -> Option<&Engine> {
        self.prefer
            .iter()
            .filter(|name| {
                statuses
                    .iter()
                    .any(|status| status.ready && status.name == **name)
            })
            .find_map(|name| self.named(name))
    }
}

/// `~/x` under the home folder; anything else as it is.
fn expand(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => path.to_owned(),
    }
}
