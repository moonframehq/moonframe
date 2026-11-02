//! The shells `ui.terminal` elements show. Each is kept by its mod and `id`,
//! so it outlives reloads and hidden panes. herdr-core or tmux, when this
//! machine has one, keeps it across launches too; a pty's ends with the
//! app. Output wakes the window from the session's thread, through a
//! channel.

use {
    crate::{app::MoonframeApp, keys::Taken},
    gpui_kit::*,
    moonframe_core::Data,
    moonframe_mux::{Backends, Preference},
    moonframe_terminal::{Ansi, Engine, Key, Open, Session, encode, paste},
    std::{
        cell::{Cell, RefCell},
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
    /// That message went out: the mod handles the exit, so the terminal
    /// goes once it is no longer drawn.
    told: Cell<bool>,
}

impl TerminalView {
    /// Keeps what the element asks to be sent as the shell exits.
    pub(crate) fn on_exit(&self, msg: Option<&Data>) {
        if self.exit.borrow().as_ref() != msg {
            *self.exit.borrow_mut() = msg.cloned();
        }
    }

    /// The mod sends itself a message as the shell exits, so Enter does
    /// not start another here.
    pub(crate) fn exit_handled(&self) -> bool {
        self.exit.borrow().is_some()
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
            last_drawn: HashSet::new(),
            drawn: HashSet::new(),
            appeared: None,
            in_main_frame: false,
            refused: HashSet::new(),
            typed: HashMap::new(),
        }
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
            },
        };
        let refused = limit.is_some();
        let (session, backend) = match limit {
            Some(limit) => (Err(limit), ""),
            None => self.spawn(&spawn),
        };
        let view = Rc::new(TerminalView {
            session,
            backend,
            focus: cx.focus_handle(),
            spawn,
            scrolled: Cell::new(0.),
            exit: RefCell::new(None),
            told: Cell::new(false),
        });
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
    /// are not counted.
    pub(crate) fn drawn(&mut self, key: &str, view: &TerminalView) {
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

    /// The exit messages of shells that just exited, as `(mod, message)`
    /// with `exited = true` added, each sent once. A told one that the main window no longer draws (its
    /// tab closed) goes, so it holds no slot; drawn again, it starts anew.
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
            .retain(|key, view| !view.told.get() || drawn.contains(key));
        exits
    }

    /// Starts the shell at `key` again, after it exited.
    fn restart(&mut self, key: &str) {
        if let Some(old) = self.views.get(key) {
            let (session, backend) = self.spawn(&old.spawn);
            let view = Rc::new(TerminalView {
                session,
                backend,
                focus: old.focus.clone(),
                spawn: old.spawn.clone(),
                scrolled: Cell::new(0.),
                exit: RefCell::new(old.exit.borrow().clone()),
                told: Cell::new(false),
            });
            self.views.insert(key.to_owned(), view);
        }
    }
}

impl Terminals {
    /// The terminal with keyboard focus in `window`, by key.
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
    /// pastes. Returns whether the terminal took it.
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
            if keystroke.key == "v"
                && let Some(text) = cx.read_from_clipboard().and_then(|item| item.text())
            {
                session.write(&paste(&text, screen.bracketed_paste));
                return true;
            }
            return false;
        }
        if screen.exited {
            if keystroke.key == "enter" && !view.exit_handled() {
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
            session.write(&bytes);
        }
        true
    }
}

/// Hands keystrokes to the topmost open overlay or popover that takes them,
/// then to the focused text field (its `keys`), then to the focused terminal
/// (unless an overlay with a backdrop covers it), then to a shell menu item
/// whose shortcut is a single typing key (`j`), before key bindings see
/// them.
pub(crate) fn intercept_keys(cx: &mut Context<MoonframeApp>) -> Subscription {
    let app = cx.entity().downgrade();
    cx.intercept_keystrokes(move |event, window, cx| {
        let taken = app
            .update(cx, |app, cx| {
                match app.layer_keystroke(&event.keystroke, window, cx) {
                    Taken::Sent => true,
                    // A field inside the covering overlay keeps its keys.
                    Taken::Blocked => app.input_keystroke(&event.keystroke, window, cx),
                    Taken::Free => {
                        app.input_keystroke(&event.keystroke, window, cx)
                            || app.terminal_keystroke(&event.keystroke, window, cx)
                            || app.mod_keystroke(&event.keystroke, window, cx)
                            || app.menu_keystroke(&event.keystroke, window, cx)
                    },
                }
            })
            .unwrap_or(false);
        if taken {
            cx.stop_propagation();
        }
    })
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
