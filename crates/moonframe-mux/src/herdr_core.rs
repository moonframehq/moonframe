//! Terminals kept by herdr-core (`hc`, Herdr 2), the runtime that keeps
//! programs running and streams their raw output to viewers. Each terminal
//! carries a `moonframe` label holding its key, so the app finds it again
//! after a restart. herdr-core emulates the terminal too and answers the
//! program's queries itself.
//!
//! This is the only module that knows herdr-core's client: the app sees a
//! terminal backend, so another runtime can take its place. Its document
//! store is available independently of terminal support.

use {
    crate::{document::Store, program},
    herdr_core_client::{Core, Endpoint, PROGRAM},
    std::{
        path::PathBuf,
        sync::{Arc, Mutex},
        time::Duration,
    },
};
#[cfg(feature = "terminal")]
use {
    herdr_core_client::{Attachment, End, wire},
    moonframe_terminal::{Backend, Open, Opened, Pane, Size},
    std::{
        collections::BTreeMap,
        sync::atomic::{AtomicBool, Ordering},
    },
};

/// The label that holds a terminal's key.
#[cfg(feature = "terminal")]
const LABEL: &str = "moonframe";
/// The label that holds the app a terminal belongs to; the starter's
/// terminals from before it existed have none.
#[cfg(feature = "terminal")]
const APP: &str = "moonframe-app";
/// Scrollback lines the core paints first when a terminal attaches.
#[cfg(feature = "terminal")]
const HISTORY: u32 = 2000;
/// How long a read waits before checking the stream again.
#[cfg(feature = "terminal")]
const POLL: Duration = Duration::from_millis(500);
/// How long starting a core may take.
const START: Duration = Duration::from_secs(5);

/// A herdr-core session, reached through its socket.
pub struct HerdrCore {
    endpoint: Endpoint,
    /// Whose terminals it opens; see [`HerdrCore::for_app`].
    #[cfg(feature = "terminal")]
    app: String,
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
            #[cfg(feature = "terminal")]
            app: crate::STARTER.to_owned(),
            core: Mutex::new(Some(Arc::new(core))),
        })
    }

    /// The same core, opening the terminals of the app `namespace` only.
    pub fn for_app(self, namespace: &str) -> Self {
        #[cfg(feature = "terminal")]
        {
            Self {
                app: namespace.to_owned(),
                ..self
            }
        }
        #[cfg(not(feature = "terminal"))]
        {
            let _ = namespace;
            self
        }
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

    fn load(&self) -> Result<Option<serde_json::Value>, String> {
        let doc = self
            .core
            .core()?
            .doc_get(&self.name)
            .map_err(|error| error.to_string())?;
        self.remember(doc.version);
        if doc.version == 0 || doc.value.is_null() {
            return Ok(None);
        }
        Ok(Some(doc.value))
    }

    fn save(&self, value: &serde_json::Value) -> Result<(), String> {
        let core = self.core.core()?;
        let version = self.version.lock().map(|version| *version).unwrap_or(0);
        let saved = core.doc_set(&self.name, value, version).or_else(|_| {
            let current = core.doc_get(&self.name)?.version;
            core.doc_set(&self.name, value, current)
        });
        let version = saved.map_err(|error| error.to_string())?;
        self.remember(version);
        Ok(())
    }
}

/// A terminal name herdr-core takes for `key` (1 to 32 of `a-z 0-9 - _ .`,
/// starting with a letter or digit), or `None` when none fits.
#[cfg(feature = "terminal")]
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

/// Whether a terminal labelled `labels` belongs to the app `app`: the
/// starter also owns the unlabelled ones it made before apps had labels.
#[cfg(feature = "terminal")]
pub(crate) fn owns(app: &str, labels: &BTreeMap<String, String>) -> bool {
    match labels.get(APP) {
        Some(owner) => owner == app,
        None => app == crate::STARTER,
    }
}

/// The installed `herdr-core`, if any: a candidate counts only if its
/// version says so, since some window managers ship an unrelated `hc`.
pub fn installed() -> Option<PathBuf> {
    program::find(&[PROGRAM, "hc"], |path| {
        program::first_line(path, &["--version"])
            .is_some_and(|version| version.starts_with("hc ") || version.starts_with("herdr-core"))
    })
}

#[cfg(feature = "terminal")]
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
                    && owns(&self.app, &terminal.labels)
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
                // Another app's names start with its own, in `hc ls`.
                let name = if self.app == crate::STARTER {
                    terminal_name(&open.key)
                } else {
                    terminal_name(&format!("{}/{}", self.app, open.key))
                };
                let mut env = BTreeMap::from([("COLORTERM", "truecolor")]);
                env.extend(
                    open.env
                        .iter()
                        .map(|(name, value)| (name.as_str(), value.as_str())),
                );
                let mut run = wire::Run {
                    cwd: cwd.as_deref(),
                    env,
                    labels: BTreeMap::from([(LABEL, open.key.as_str()), (APP, self.app.as_str())]),
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
        let exited = Arc::new(AtomicBool::new(false));
        Ok(Opened {
            pane: Box::new(CorePane {
                attachment: attachment.clone(),
                detached: detached.clone(),
                exited: exited.clone(),
                address,
                core,
            }),
            output: Box::new(Reading {
                attachment,
                detached,
                exited,
                done: false,
            }),
        })
    }
}

#[cfg(feature = "terminal")]
struct CorePane {
    attachment: Arc<Attachment>,
    /// Set when the pane goes, so the reader lets go of the stream too.
    detached: Arc<AtomicBool>,
    exited: Arc<AtomicBool>,
    /// The terminal, to find it in the core's list.
    address: String,
    /// Kept so the connection outlives every stream from it.
    core: Arc<Core>,
}

#[cfg(feature = "terminal")]
impl Drop for CorePane {
    /// Detaches once the reader notices: the terminal keeps running.
    fn drop(&mut self) {
        self.detached.store(true, Ordering::Release);
    }
}

#[cfg(feature = "terminal")]
impl Pane for CorePane {
    fn exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    fn write(&self, bytes: &[u8]) {
        let _ = self.attachment.write(bytes);
    }

    fn resize(&self, size: Size) {
        let _ = self.attachment.resize(size.cols, size.rows);
    }

    fn answers_queries(&self) -> bool {
        true
    }

    /// The folder of what runs in the foreground (a program the shell
    /// started, else the shell), as the core reports it.
    fn cwd(&self) -> Option<PathBuf> {
        let terminal = self
            .core
            .list(false)
            .ok()?
            .terminals
            .into_iter()
            .find(|terminal| terminal.address == self.address)?;
        let foreground = terminal.foreground.map(|foreground| foreground.cwd);
        [foreground.unwrap_or_default(), terminal.cwd]
            .into_iter()
            .find(|cwd| !cwd.is_empty())
            .map(PathBuf::from)
    }
}

/// The attachment's bytes until its stream ends: the program ended, or the
/// connection dropped (the program most likely still runs; Enter attaches
/// again).
#[cfg(feature = "terminal")]
struct Reading {
    attachment: Arc<Attachment>,
    detached: Arc<AtomicBool>,
    exited: Arc<AtomicBool>,
    done: bool,
}

#[cfg(feature = "terminal")]
impl Iterator for Reading {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Vec<u8>> {
        while !self.done && !self.detached.load(Ordering::Acquire) {
            match self.attachment.read(POLL) {
                Ok(chunk) => {
                    self.done = chunk.end.is_some();
                    if let Some(end) = &chunk.end {
                        self.exited.store(shell_exited(end), Ordering::Release);
                    }
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

/// A detached stream has an empty CLOSE; a lost connection has no exit
/// information. Only the core's explicit process outcome closes a tab.
#[cfg(feature = "terminal")]
fn shell_exited(end: &End) -> bool {
    matches!(end, End::Closed(closed) if closed.exit_code.is_some()
        || matches!(closed.reason.as_deref(), Some("exited" | "killed")))
}

#[cfg(all(test, feature = "terminal"))]
mod tests {
    use super::{shell_exited, terminal_name};

    #[test]
    fn lost_and_detached_streams_are_not_shell_exits() {
        use herdr_core_client::{Closed, End};
        assert!(!shell_exited(&End::Lost));
        assert!(!shell_exited(&End::Closed(Closed::default())));
        assert!(!shell_exited(&End::Closed(Closed {
            reason: Some("unknown".into()),
            exit_code: None,
        })));
        for reason in ["exited", "killed"] {
            assert!(shell_exited(&End::Closed(Closed {
                reason: Some(reason.into()),
                exit_code: None,
            })));
        }
        assert!(shell_exited(&End::Closed(Closed {
            reason: None,
            exit_code: Some(0),
        })));
    }

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
