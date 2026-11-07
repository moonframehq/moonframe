//! The shells `ui.terminal` elements show. Each is kept by its mod and `id`,
//! so it outlives reloads and hidden panes. herdr-core or tmux, when this
//! machine has one, keeps it across launches too; a pty's ends with the
//! app. Output wakes the window from the session's thread, through a
//! channel.

#[path = "selection.rs"]
mod selection;
#[path = "shell_events.rs"]
mod shell_events;

pub(crate) use selection::{Selection, Spot};

use {
    crate::app::MoonframeApp,
    gpui_kit::*,
    moonframe_core::Data,
    moonframe_mux::{Backends, Preference},
    moonframe_terminal::{Ansi, Engine, Key, Open, Session, encode, paste},
    std::{
        cell::{Cell, OnceCell, RefCell},
        collections::{HashMap, HashSet},
        path::{Path, PathBuf},
        rc::Rc,
        sync::Arc,
        time::Instant,
    },
};

/// Shells one mod can have, and the whole app: each is a process, and a mod
/// drawing ids in a loop must not start them without end.
const PER_MOD: usize = 8;
const TOTAL: usize = 32;

/// What a terminal was opened with, to open it again after it exits.
#[derive(Clone)]
struct Spawn {
    engine: Engine,
    preference: Preference,
    open: Open,
}

/// A shell and what the window keeps for it.
pub(crate) struct TerminalView {
    pub(crate) session: Result<Session, String>,
    /// The backend that runs it, shown as a hint in its corner.
    pub(crate) backend: &'static str,
    pub(crate) focus: FocusHandle,
    spawn: Spawn,
    /// Scrolling not yet worth a whole line.
    pub(crate) scrolled: Cell<f32>,
    /// What its element sends its mod as the shell exits, as last drawn.
    exit: RefCell<Option<Data>>,
    /// What its element sends its mod as the shell reports something.
    shell: RefCell<Option<Data>>,
    /// That message went out: the mod handles the exit, so the terminal
    /// goes once it is no longer drawn.
    told: Cell<bool>,
    /// A window other than the main one drew it. Only the main window's
    /// frames are counted, so such a terminal is never taken for gone.
    elsewhere: Cell<bool>,
    /// The text chosen with the pointer, in the visible screen's cells.
    pub(crate) selection: RefCell<Option<Selection>>,
    /// The left button went down on the grid and is still held.
    pub(crate) selecting: Cell<bool>,
    /// Where the grid was last painted: its first cell's corner and a
    /// cell's size, to find the cell under the pointer.
    pub(crate) grid: Cell<Option<(Point<Pixels>, Size<Pixels>)>>,
}

impl TerminalView {
    fn new(
        session: Result<Session, String>,
        backend: &'static str,
        focus: FocusHandle,
        spawn: Spawn,
    ) -> Self {
        Self {
            session,
            backend,
            focus,
            spawn,
            scrolled: Cell::new(0.),
            exit: RefCell::new(None),
            shell: RefCell::new(None),
            told: Cell::new(false),
            elsewhere: Cell::new(false),
            selection: RefCell::new(None),
            selecting: Cell::new(false),
            grid: Cell::new(None),
        }
    }

    /// Keeps what the element asks to be sent as the shell exits.
    pub(crate) fn on_exit(&self, msg: Option<&Data>) {
        if self.exit.borrow().as_ref() != msg {
            *self.exit.borrow_mut() = msg.cloned();
        }
    }

    /// Keeps what the element asks to be sent as the shell reports.
    pub(crate) fn on_shell(&self, msg: Option<&Data>) {
        if self.shell.borrow().as_ref() != msg {
            *self.shell.borrow_mut() = msg.cloned();
        }
    }

    /// The mod sends itself a message as the shell exits, so Enter does
    /// not start another here.
    pub(crate) fn exit_handled(&self) -> bool {
        self.exit.borrow().is_some()
    }

    /// The selected text, when there is any.
    pub(crate) fn selected_text(&self) -> Option<String> {
        let session = self.session.as_ref().ok()?;
        let selection = (*self.selection.borrow()).filter(|selection| !selection.is_empty())?;
        Some(selection.text(&session.screen())).filter(|text| !text.is_empty())
    }

    /// Forgets the selection, as typing, pasting, scrolling or resizing
    /// moves the text under it. Returns whether there was one to redraw.
    pub(crate) fn clear_selection(&self) -> bool {
        self.selecting.set(false);
        self.selection.borrow_mut().take().is_some()
    }
}

/// A command line waiting for the terminal of a tab `moonframe pane new`
/// opened.
struct Typed {
    bytes: Vec<u8>,
    /// The folder that terminal must start in. The tab's id is published
    /// on a topic any mod can hear, so a mod could draw a terminal of that
    /// id first; it gets the command only if it starts where it was asked
    /// to, never in a folder of the mod's choosing.
    cwd: Option<PathBuf>,
    asked: Instant,
}

/// Command lines waiting at once. A tab's terminal starts only once it is
/// drawn, which can be long after (the window hidden behind others), so
/// they wait without a time limit; past this many, the oldest goes.
const TYPED_MAX: usize = 16;

pub(crate) struct Terminals {
    views: HashMap<String, Rc<TerminalView>>,
    backends: Arc<Backends>,
    wake: Arc<dyn Fn() + Send + Sync>,
    /// The theme's 16 colors; xterm's when `None`.
    palette: Option<Ansi>,
    /// What new shells start with so they report what they do; written
    /// when the first one starts.
    shell_env: OnceCell<Vec<(String, String)>>,
    /// The terminals the main window drew in its last frame, and in the
    /// one being drawn: one that appears (a new tab, or a tab selected
    /// again) takes the keyboard, ready to type in.
    last_drawn: HashSet<String>,
    drawn: HashSet<String>,
    /// The last terminal that appeared since the window last looked.
    appeared: Option<FocusHandle>,
    /// The main window's frame is being drawn: only its terminals count
    /// as drawn, so one in a mod window that missed a frame of the main
    /// window's is not taken to have appeared again.
    in_main_frame: bool,
    /// The terminals refused for being over a limit, warned of once: they
    /// are asked for again every frame.
    refused: HashSet<String>,
    /// What to type into a terminal as it starts, by its element `id`: a
    /// command line `moonframe pane new` asked for. The app sets it, never
    /// Luau, so a mod still picks where a terminal starts but not what runs.
    typed: HashMap<String, Typed>,
}

impl Terminals {
    pub(crate) fn new(backends: Arc<Backends>, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            views: HashMap::new(),
            backends,
            wake,
            palette: None,
            shell_env: OnceCell::new(),
            last_drawn: HashSet::new(),
            drawn: HashSet::new(),
            appeared: None,
            in_main_frame: false,
            refused: HashSet::new(),
            typed: HashMap::new(),
        }
    }

    /// The shell integration's environment; empty when it cannot be
    /// written, and shells then run without it.
    fn shell_env(&self) -> &[(String, String)] {
        self.shell_env.get_or_init(|| {
            let folder = moonframe_packages::paths::cache().join("shell");
            moonframe_terminal::install_integration(&folder)
                .inspect_err(|error| tracing::warn!("no shell integration: {error}"))
                .unwrap_or_default()
        })
    }

    /// Paints every terminal, and those started later, in `palette`.
    pub(crate) fn set_palette(&mut self, palette: Option<Ansi>) {
        if self.palette == palette {
            return;
        }
        self.palette = palette;
        for view in self.views.values() {
            if let Ok(session) = &view.session {
                session.set_palette(palette);
            }
        }
    }

    /// A shell for `spawn` on the best backend there is, in the theme's
    /// colors, and the backend's name.
    fn spawn(&self, spawn: &Spawn) -> (Result<Session, String>, &'static str) {
        // Check before opening a backend: a missing engine must not start
        // a shell (especially one kept by tmux or herdr-core).
        if !spawn.engine.available() {
            return (
                Err(format!(
                    "this build has no `{}` terminal; enable the `{}` feature",
                    spawn.engine.name(),
                    spawn.engine.name(),
                )),
                "",
            );
        }
        let (opened, backend) = match self.backends.open(spawn.preference, &spawn.open) {
            Ok(opened) => opened,
            Err(problem) => return (Err(problem), ""),
        };
        let session = Session::start(opened, spawn.engine, spawn.open.size, self.wake.clone());
        if let Ok(session) = &session
            && self.palette.is_some()
        {
            session.set_palette(self.palette);
        }
        (session, backend)
    }

    /// Types `bytes` into the terminal drawn with the element id `id` once
    /// it starts, however much later, and its shell is ready for a line.
    pub(crate) fn type_on_start(&mut self, id: &str, cwd: Option<PathBuf>, bytes: Vec<u8>) {
        while self.typed.len() >= TYPED_MAX {
            let Some(oldest) = self
                .typed
                .iter()
                .min_by_key(|(_, typed)| typed.asked)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            tracing::warn!("terminal {oldest}: never drawn; its command is dropped");
            self.typed.remove(&oldest);
        }
        self.typed.insert(id.to_owned(), Typed {
            bytes,
            cwd,
            asked: Instant::now(),
        });
    }

    /// The command line waiting for a terminal `key` that starts in `cwd`.
    fn take_typed(&mut self, key: &str, cwd: Option<&Path>) -> Option<Vec<u8>> {
        let id = key.rsplit_once('/').map_or(key, |(_, id)| id);
        let typed = self.typed.get(id)?;
        if typed.cwd.as_deref().map(canonical) != cwd.map(canonical) {
            tracing::warn!(
                "terminal {key}: not in the folder its pane was opened for; nothing typed"
            );
            return None;
        }
        self.typed.remove(id).map(|typed| typed.bytes)
    }

    /// The terminal at `key`, started now if it is new.
    pub(crate) fn open(
        &mut self,
        key: &str,
        engine: Engine,
        preference: Preference,
        cwd: Option<&str>,
        cx: &App,
    ) -> Rc<TerminalView> {
        if let Some(view) = self.views.get(key) {
            return view.clone();
        }
        let owner = key.split_once('/').map_or(key, |(owner, _)| owner);
        let mine = self
            .views
            .keys()
            .filter(|other| other.split_once('/').is_some_and(|(of, _)| of == owner))
            .count();
        let limit = if mine >= PER_MOD {
            Some(format!("`{owner}` already has {PER_MOD} terminals"))
        } else if self.views.len() >= TOTAL {
            Some(format!("the app already has {TOTAL} terminals"))
        } else {
            None
        };
        let spawn = Spawn {
            engine,
            preference,
            open: Open {
                key: key.to_owned(),
                // Unnamed, the folder the app was started from; the
                // backend falls back to home without one.
                cwd: cwd
                    .map(expand)
                    .or_else(|| moonframe_providers::launch_dir().map(Path::to_owned)),
                size: moonframe_terminal::Size::default(),
                env: self.shell_env().to_vec(),
            },
        };
        let refused = limit.is_some();
        let (session, backend) = match limit {
            Some(limit) => (Err(limit), ""),
            None => self.spawn(&spawn),
        };
        let view = Rc::new(TerminalView::new(
            session,
            backend,
            cx.focus_handle(),
            spawn,
        ));
        if let Err(problem) = &view.session
            && (!refused || self.refused.insert(key.to_owned()))
        {
            tracing::warn!("terminal {key}: {problem}");
        }
        // A refused one is asked for again next frame, not kept.
        if !refused {
            self.refused.remove(key);
            self.views.insert(key.to_owned(), view.clone());
            if let Ok(session) = &view.session
                && let Some(bytes) = self.take_typed(key, view.spawn.open.cwd.as_deref())
            {
                session.type_when_ready(bytes);
            }
        }
        view
    }

    /// Starts the main window's frame: what it drew last is what the new
    /// frame compares against.
    pub(crate) fn begin_frame(&mut self) {
        self.last_drawn = std::mem::take(&mut self.drawn);
        self.in_main_frame = true;
    }

    /// The terminal at `key` is drawn in the main window's frame; if it was
    /// not in the last one, it is the one to focus. Other windows' terminals
    /// are not counted, only marked as drawn elsewhere.
    pub(crate) fn drawn(&mut self, key: &str, view: &TerminalView) {
        if !self.in_main_frame {
            view.elsewhere.set(true);
        }
        if self.in_main_frame && self.drawn.insert(key.to_owned()) && !self.last_drawn.contains(key)
        {
            self.appeared = Some(view.focus.clone());
        }
    }

    /// Ends the main window's frame: the terminal that appeared in it, to
    /// focus.
    pub(crate) fn end_frame(&mut self) -> Option<FocusHandle> {
        self.in_main_frame = false;
        self.appeared.take()
    }

    /// What each shell reported, as `(mod, message)`, oldest first. Nothing
    /// is kept for a terminal whose mod did not ask.
    pub(crate) fn take_shell_events(&self) -> Vec<(String, Data)> {
        let mut events = Vec::new();
        for (key, view) in &self.views {
            let Ok(session) = &view.session else {
                continue;
            };
            let signals = session.take_signals();
            if let Some(msg) = view.shell.borrow().as_ref() {
                let owner = key
                    .rsplit_once('/')
                    .map_or(key.as_str(), |(owner, _)| owner);
                events.extend(
                    signals
                        .into_iter()
                        .map(|signal| (owner.to_owned(), shell_events::message(msg, signal))),
                );
            }
        }
        events
    }

    /// The exit messages of shells that just exited, as `(mod, message)`
    /// with `exited = true` added, each sent once. A told one that the main
    /// window no longer draws (its tab closed) goes, so it holds no slot;
    /// drawn again, it starts anew. One another window drew stays: that
    /// window may still show it, and drawing it would start another shell.
    pub(crate) fn take_exits(&mut self) -> Vec<(String, Data)> {
        let mut exits = Vec::new();
        for (key, view) in &self.views {
            let Ok(session) = &view.session else {
                continue;
            };
            if view.told.get() || !session.exited() {
                continue;
            }
            if let Some(msg) = view.exit.borrow().clone() {
                view.told.set(true);
                let owner = key
                    .rsplit_once('/')
                    .map_or(key.as_str(), |(owner, _)| owner);
                let msg = crate::browser::with(&msg, [("exited", Data::Bool(true))]);
                exits.push((owner.to_owned(), msg));
            }
        }
        let drawn = &self.drawn;
        self.views
            .retain(|key, view| !view.told.get() || view.elsewhere.get() || drawn.contains(key));
        exits
    }

    /// Starts the shell at `key` again, after it exited.
    fn restart(&mut self, key: &str) {
        if let Some(old) = self.views.get(key) {
            let (session, backend) = self.spawn(&old.spawn);
            let view = Rc::new(TerminalView::new(
                session,
                backend,
                old.focus.clone(),
                old.spawn.clone(),
            ));
            view.on_exit(old.exit.borrow().as_ref());
            view.on_shell(old.shell.borrow().as_ref());
            view.elsewhere.set(old.elsewhere.get());
            self.views.insert(key.to_owned(), view);
        }
    }
}

impl Terminals {
    /// Where to ask the folder of the terminal with keyboard focus in
    /// `window`, if one has it.
    pub(crate) fn focused_folder(&self, window: &Window) -> Option<moonframe_terminal::Folder> {
        let (_, view) = self.focused(window)?;
        view.session.as_ref().ok().map(Session::folder)
    }

    /// Whether a terminal has keyboard focus in `window`.
    pub(crate) fn any_focused(&self, window: &Window) -> bool {
        self.focused(window).is_some()
    }

    fn focused(&self, window: &Window) -> Option<(String, Rc<TerminalView>)> {
        self.views
            .iter()
            .find(|(_, view)| view.focus.is_focused(window))
            .map(|(key, view)| (key.clone(), view.clone()))
    }
}

impl MoonframeApp {
    /// Every keystroke, before any key binding: one for a focused terminal
    /// goes to its shell, Tab, Esc and Ctrl keys included, so completion
    /// and the like work. ⌘ combinations stay the app's, except ⌘V, which
    /// pastes, and ⌘C while text is selected, which copies it. Returns
    /// whether the terminal took it.
    pub(crate) fn terminal_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((key, view)) = self.terminals.focused(window) else {
            return false;
        };
        let Ok(session) = &view.session else {
            return false;
        };
        let modifiers = keystroke.modifiers;
        let screen = session.screen();
        if modifiers.platform {
            if keystroke.key == "c"
                && let Some(text) = view.selected_text()
            {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                return true;
            }
            if keystroke.key == "v"
                && let Some(text) = cx.read_from_clipboard().and_then(|item| item.text())
            {
                if view.clear_selection() {
                    cx.notify();
                }
                session.write(&paste(&text, screen.bracketed_paste));
                return true;
            }
            return false;
        }
        if screen.exited || screen.disconnected {
            if keystroke.key == "enter" && (screen.disconnected || !view.exit_handled()) {
                self.terminals.restart(&key);
                cx.notify();
            }
            return true;
        }
        let pressed = Key {
            name: &keystroke.key,
            text: keystroke.key_char.as_deref(),
            ctrl: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
        };
        // A key it has no bytes for (F13) does nothing, and still does not
        // reach the app while the terminal has focus.
        if let Some(bytes) = encode(pressed, screen.app_cursor) {
            if view.clear_selection() {
                cx.notify();
            }
            session.write(&bytes);
        }
        true
    }
}

/// `~` and `~/…` are the home folder.
/// `path` with links resolved, to compare two folders.
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

fn expand(cwd: &str) -> PathBuf {
    match (cwd.strip_prefix('~'), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            PathBuf::from(home).join(rest.trim_start_matches('/'))
        },
        _ => PathBuf::from(cwd),
    }
}
