//! Terminals kept by herdr-core (`hc`, Herdr 2), the runtime that keeps
//! programs running and streams their raw output to viewers. Each terminal
//! carries a `moonframe` label holding its key, so the app finds it again
//! after a restart. herdr-core emulates the terminal too and answers the
//! program's queries itself.
//!
//! This is the only module that knows herdr-core's client: the app sees a
//! [`Backend`], so another runtime can take its place.

use {
    crate::{
        document::{Document, Store},
        program,
    },
    herdr_core_client::{Attachment, Core, Endpoint, PROGRAM, wire},
    moonframe_terminal::{Backend, Open, Opened, Pane, Size},
    std::{
        collections::BTreeMap,
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    },
};

/// The label that holds a terminal's key.
const LABEL: &str = "moonframe";
/// Scrollback lines the core paints first when a terminal attaches.
const HISTORY: u32 = 2000;
/// How long a read waits before checking the stream again.
const POLL: Duration = Duration::from_millis(500);
/// How long starting a core may take.
const START: Duration = Duration::from_secs(5);

/// A herdr-core session, reached through its socket.
pub struct HerdrCore {
    endpoint: Endpoint,
    /// The connection, made again when it is lost.
    core: Mutex<Option<Arc<Core>>>,
}

impl HerdrCore {
    /// The running core, or one started from the installed `herdr-core`.
    /// `None` when herdr-core is not installed or will not start.
    pub fn connect() -> Option<Self> {
        Self::connect_session(None)
    }

    /// The same, on core session `session` (`HC_SESSION`) rather than the
    /// user's default one, as tests use.
    pub fn connect_session(session: Option<&str>) -> Option<Self> {
        let endpoint = Endpoint::resolve(|name| match (name, session) {
            ("HC_SESSION", Some(session)) => Some(session.into()),
            _ => std::env::var_os(name),
        })
        .ok()?;
        let core = match Core::connect(&endpoint) {
            Ok(core) => core,
            Err(_) => {
                let binary = installed()?;
                let configure = |command: &mut std::process::Command| {
                    if let Some(session) = session {
                        command.env("HC_SESSION", session);
                    }
                };
                if let Err(error) = herdr_core_client::start(&binary, configure, START) {
                    tracing::warn!("herdr-core did not start: {error}");
                    return None;
                }
                Core::connect(&endpoint)
                    .inspect_err(|error| tracing::warn!("{error}"))
                    .ok()?
            },
        };
        Some(Self {
            endpoint,
            core: Mutex::new(Some(Arc::new(core))),
        })
    }

    fn core(&self) -> Result<Arc<Core>, String> {
        let mut core = self.core.lock().map_err(|_| "herdr-core is unusable")?;
        if let Some(live) = core.as_ref().filter(|live| !live.is_lost()) {
            return Ok(live.clone());
        }
        let live = Arc::new(Core::connect(&self.endpoint).map_err(|error| error.to_string())?);
        *core = Some(live.clone());
        Ok(live)
    }
}

/// The app's document in herdr-core's document store, under the app's
/// name. Writes carry the version last read, as the core asks; one that
/// lost a race reads the version again and writes over it, since one app
/// writes its own document.
pub struct CoreStore {
    core: Arc<HerdrCore>,
    name: String,
    version: Mutex<u64>,
}

impl CoreStore {
    pub fn new(core: Arc<HerdrCore>, name: &str) -> Self {
        Self {
            core,
            name: name.to_owned(),
            version: Mutex::new(0),
        }
    }

    fn remember(&self, version: u64) {
        if let Ok(mut current) = self.version.lock() {
            *current = version;
        }
    }
}

impl Store for CoreStore {
    fn name(&self) -> &'static str {
        "herdr-core"
    }

    fn load(&self) -> Result<Option<Document>, String> {
        let doc = self
            .core
            .core()?
            .doc_get(&self.name)
            .map_err(|error| error.to_string())?;
        self.remember(doc.version);
        if doc.version == 0 || doc.value.is_null() {
            return Ok(None);
        }
        Document::from_json(&doc.value).map(Some)
    }

    fn save(&self, document: &Document) -> Result<(), String> {
        let core = self.core.core()?;
        let value = document.to_json();
        let version = self.version.lock().map(|version| *version).unwrap_or(0);
        let saved = core.doc_set(&self.name, &value, version).or_else(|_| {
            let current = core.doc_get(&self.name)?.version;
            core.doc_set(&self.name, &value, current)
        });
        let version = saved.map_err(|error| error.to_string())?;
        self.remember(version);
        Ok(())
    }
}

/// A terminal name herdr-core takes for `key` (1 to 32 of `a-z 0-9 - _ .`,
/// starting with a letter or digit), or `None` when none fits.
fn terminal_name(key: &str) -> Option<String> {
    let name: String = key
        .chars()
        .map(|char| match char.to_ascii_lowercase() {
            char @ ('a'..='z' | '0'..='9' | '-' | '_') => char,
            '/' => '.',
            _ => '-',
        })
        .take(32)
        .collect();
    let first = name.chars().next()?;
    // `t7` is shaped like a terminal's address.
    let like_address =
        first == 't' && name.len() > 1 && name[1..].chars().all(|char| char.is_ascii_digit());
    (first.is_ascii_alphanumeric() && !like_address).then_some(name)
}

/// The installed `herdr-core`, if any: a candidate counts only if its
/// version says so, since some window managers ship an unrelated `hc`.
pub fn installed() -> Option<PathBuf> {
    program::find(&[PROGRAM, "hc"], |path| {
        program::first_line(path, &["--version"])
            .is_some_and(|version| version.starts_with("hc ") || version.starts_with("herdr-core"))
    })
}

impl Backend for HerdrCore {
    fn name(&self) -> &'static str {
        "herdr-core"
    }

    fn open(&self, open: &Open) -> Result<Opened, String> {
        let core = self.core()?;
        let running = core
            .list(false)
            .map_err(|error| error.to_string())?
            .terminals
            .into_iter()
            .find(|terminal| {
                terminal.state == wire::TerminalState::Live
                    && terminal.labels.get(LABEL) == Some(&open.key)
            });
        let address = match running {
            Some(terminal) => terminal.address,
            None => {
                let cwd = open
                    .cwd
                    .as_ref()
                    .filter(|cwd| cwd.is_dir())
                    .map(|cwd| cwd.to_string_lossy().into_owned())
                    .or_else(|| std::env::var("HOME").ok());
                let name = terminal_name(&open.key);
                let mut run = wire::Run {
                    cwd: cwd.as_deref(),
                    env: BTreeMap::from([("COLORTERM", "truecolor")]),
                    labels: BTreeMap::from([(LABEL, open.key.as_str())]),
                    name: name.as_deref(),
                    ..wire::Run::default()
                };
                // The label finds it again; the name only reads better in
                // `hc ls`, so one the core refuses (taken) is left out.
                let started = core.run(&run).or_else(|_| {
                    run.name = None;
                    core.run(&run)
                });
                started.map_err(|error| error.to_string())?.address
            },
        };
        let attachment = core
            .attach(&wire::Attach {
                cols: open.size.cols.max(1),
                rows: open.size.rows.max(1),
                history: HISTORY,
                terminal: &address,
                ..wire::Attach::default()
            })
            .map_err(|error| error.to_string())?;
        let attachment = Arc::new(attachment);
        let detached = Arc::new(AtomicBool::new(false));
        Ok(Opened {
            pane: Box::new(CorePane {
                attachment: attachment.clone(),
                detached: detached.clone(),
                _core: core,
            }),
            output: Box::new(Reading {
                attachment,
                detached,
                done: false,
            }),
        })
    }
}

struct CorePane {
    attachment: Arc<Attachment>,
    /// Set when the pane goes, so the reader lets go of the stream too.
    detached: Arc<AtomicBool>,
    /// Kept so the connection outlives every stream from it.
    _core: Arc<Core>,
}

impl Drop for CorePane {
    /// Detaches once the reader notices: the terminal keeps running.
    fn drop(&mut self) {
        self.detached.store(true, Ordering::Release);
    }
}

impl Pane for CorePane {
    fn write(&self, bytes: &[u8]) {
        let _ = self.attachment.write(bytes);
    }

    fn resize(&self, size: Size) {
        let _ = self.attachment.resize(size.cols, size.rows);
    }

    fn answers_queries(&self) -> bool {
        true
    }
}

/// The attachment's bytes until its stream ends: the program ended, or the
/// connection dropped (the program most likely still runs; Enter attaches
/// again).
struct Reading {
    attachment: Arc<Attachment>,
    detached: Arc<AtomicBool>,
    done: bool,
}

impl Iterator for Reading {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Vec<u8>> {
        while !self.done && !self.detached.load(Ordering::Acquire) {
            match self.attachment.read(POLL) {
                Ok(chunk) => {
                    self.done = chunk.end.is_some();
                    if !chunk.bytes.is_empty() {
                        return Some(chunk.bytes);
                    }
                },
                Err(_) => self.done = true,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::terminal_name;

    #[test]
    fn keys_become_names_the_core_takes() {
        assert_eq!(terminal_name("editor/main").as_deref(), Some("editor.main"));
        assert_eq!(terminal_name("My Mod/X").as_deref(), Some("my-mod.x"));
        assert_eq!(terminal_name("t7"), None);
        assert_eq!(terminal_name("/x"), None);
        assert_eq!(
            terminal_name(&"a".repeat(40)).map(|name| name.len()),
            Some(32)
        );
    }
}
