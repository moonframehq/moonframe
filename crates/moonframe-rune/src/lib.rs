//! The Moonframe window drawn with Rune. It reads the same checked [`Node`]
//! trees as the GPUI renderer in `crates/moonframe`, so one Luau source, its
//! components and its stylesheet draw on either: this crate only turns those
//! trees into Rune elements and hands clicks and edits back to the
//! [`Workspace`].
//!
//! The app chooses with a Cargo feature; see `crates/moonframe-app`. What
//! isn't drawn here shows as a marker naming the element (see
//! `render::leaves`), `docs/rune-coverage.md` lists how each component fares,
//! and [`COMPILED`] tells Luau what this build has.
//!
//! [`Node`]: moonframe_core::Node

// keep-sorted start
#[cfg_attr(not(any(target_os = "macos", windows)), path = "browser_left_out.rs")]
mod browser;
#[cfg(all(test, unix))]
mod coverage;
#[path = "../../moonframe/src/dock.rs"]
#[allow(dead_code, reason = "shared with `moonframe`, which uses more of it")]
mod dock;
#[path = "../../moonframe/src/embed.rs"]
#[allow(dead_code, reason = "shared with `moonframe`, which uses more of it")]
mod embed;
mod icons;
mod inspect;
mod keys;
mod launch;
mod map;
mod menus;
mod natives;
#[path = "../../moonframe/src/persist.rs"]
mod persist;
mod render;
#[path = "../../moonframe/src/settings.rs"]
#[allow(dead_code, reason = "shared with `moonframe`, which uses more of it")]
mod settings;
mod style;
mod terminal;
#[cfg(all(test, unix))]
mod tests;
mod theme;
#[cfg_attr(not(unix), path = "video_left_out.rs")]
mod video;
mod watch;
// keep-sorted end

pub use {
    launch::{Moonframe, View},
    moonframe_core::{self, Data},
    moonframe_providers::{AsyncNative, Handle, Native},
    rune, rust_embed,
};
use {
    moonframe_core::{
        APP, Action, Appearance, Command, Error, ListWindow, PROVIDER, Persist, Pick, PickKind,
        Platform, Request, Roots, Titlebar, WindowState, Workspace,
    },
    moonframe_packages::{identity, paths},
    moonframe_providers::{Event, Natives, Providers},
    rune::{AppHandle, Cx, El, FileDialogOptions, ImageCache, TextEdit, Tx, Waker},
    std::{
        cell::{Cell, RefCell},
        collections::{BTreeMap, BTreeSet, HashMap},
        path::PathBuf,
        rc::Rc,
        sync::{
            Arc,
            mpsc::{Receiver, channel},
        },
        time::{Duration, Instant},
    },
};

/// Capabilities a mod reads as `app.capabilities`, by the names of the GPUI
/// build's features, and this backend's own name.
const COMPILED: &[(&str, bool)] = &[
    // keep-sorted start
    ("acp", true),
    ("alacritty", true),
    ("browser", cfg!(any(target_os = "macos", windows))),
    ("chat", true),
    ("docs", true),
    ("forge", true),
    ("git", true),
    ("map", true),
    ("markdown", true),
    ("rune", true),
    ("shell", true),
    ("speech", true),
    ("terminal", true),
    ("usage", true),
    ("video", cfg!(unix)),
    ("wasm", true),
    // keep-sorted end
];

/// Rows built beyond those showing, on each side, as the GPUI list does.
const OVERSCAN: usize = 16;

/// A text field's edit state, kept across renders by its key.
#[derive(Default)]
pub(crate) struct Field {
    pub(crate) edit: TextEdit,
    /// The last `value` the mod passed: only a new one replaces the text.
    pub(crate) passed: Option<String>,
    /// `focus` as last drawn: it takes the keyboard as it turns true.
    pub(crate) asked_focus: bool,
    /// It had the keyboard when last looked at.
    pub(crate) had_focus: bool,
    /// Who hears `blur`, and what they are sent: sender, mod and message.
    pub(crate) blur: Option<(String, String, Data)>,
}

/// A `ui.list` on screen: where it is scrolled, written as Rune lays it out.
pub(crate) struct Watched {
    pub(crate) owner: String,
    pub(crate) id: String,
    pub(crate) row_height: f32,
    pub(crate) count: usize,
    pub(crate) window: ListWindow,
    /// Scrolled offset and viewport height, in points.
    pub(crate) seen: Rc<Cell<(f32, f32)>>,
}

/// Everything the window keeps. Rune hands views `&Ui`, so what a view has to
/// record while it draws sits in cells.
pub struct Ui {
    pub(crate) workspace: Workspace,
    pub(crate) dark: bool,
    /// What the system asks for, read as the view runs and applied by `poll`.
    pub(crate) system_dark: Cell<bool>,
    pub(crate) fields: RefCell<HashMap<String, Field>>,
    pub(crate) lists: RefCell<HashMap<String, Watched>>,
    /// Pane sizes the user dragged to, by the pane's path.
    pub(crate) panes: HashMap<String, f32>,
    /// The same providers the GPUI app runs, started once Rune has a waker.
    providers: Option<Providers>,
    events: Option<Receiver<Event>>,
    links: BTreeMap<String, Data>,
    /// Decodes `ui.image`'s pictures; set once Rune has a waker.
    pub(crate) images: Option<ImageCache>,
    pub(crate) pictures: RefCell<HashMap<PathBuf, rune::Image>>,
    /// What the user chose: `system` follows it, light and dark stand.
    pub(crate) appearance: Appearance,
    /// Where that choice is kept.
    pub(crate) settings: Option<PathBuf>,
    /// Draw the component browser instead of the app (`MOONFRAME_RUNE_GALLERY`).
    pub(crate) gallery: bool,
    /// The native providers this build has, until they start.
    pub(crate) natives: Natives,
    /// The app's own `ui.view`s, by name.
    pub(crate) views: HashMap<String, View>,
    /// The component the browser shows.
    pub(crate) preview: Option<String>,
    pub(crate) terminals: terminal::Terminals,
    pub(crate) inspecting: inspect::Inspecting,
    /// What each element is, while picking, by its key.
    pub(crate) crumbs: RefCell<HashMap<String, inspect::Crumb>>,
    pub(crate) browsers: browser::Browsers,
    pub(crate) videos: video::Videos,
    pub(crate) map_tiles: map::Tiles,
    /// What each menu bar slot does; see `menus`.
    pub(crate) menu_actions: Vec<Action>,
    /// The menus last put in the menu bar.
    pub(crate) shown_menus: Option<Vec<moonframe_core::AppMenu>>,
    pub(crate) app: Option<AppHandle<Ui>>,
    /// The app's icon, for its About panel.
    pub(crate) icon: Option<PathBuf>,
    /// The system's own About has left the app menu.
    about_replaced: bool,
    /// Open overlays and popovers, lowest first, as last drawn.
    pub(crate) layers: RefCell<Vec<keys::Layer>>,
    watching: Option<watch::Watching>,
    /// How long a capture still waits for shells to answer.
    settle: Option<Duration>,
    /// A terminal cell's width in points, once the probe has been laid out.
    pub(crate) cell_width: Cell<Option<f32>>,
    /// Wakes the window from other threads; set once Rune has one.
    pub(crate) waker: Option<Waker>,
    /// The elements a view asked for that only GPUI draws, by `ui.` name.
    pub(crate) left_out: RefCell<BTreeSet<String>>,
}

impl Drop for Ui {
    fn drop(&mut self) {
        if let Some(providers) = &mut self.providers {
            providers.shutdown();
        }
    }
}

impl Ui {
    /// Makes the window light, dark or follow the system, and keeps the
    /// choice for the next run, as the GPUI app does.
    pub(crate) fn set_appearance(&mut self, appearance: Appearance) {
        self.show_appearance(appearance);
        if let Some(path) = &self.settings
            && let Err(problem) = settings::save(path, &[(settings::APPEARANCE, appearance.name())])
        {
            tracing::warn!("cannot save the appearance: {problem}");
        }
    }

    /// `appearance`, as it was saved or just chosen.
    pub(crate) fn show_appearance(&mut self, appearance: Appearance) {
        self.appearance = appearance;
        self.dark = appearance.is_dark(self.system_dark.get());
        self.workspace.set_dark(self.dark);
        self.workspace.set_appearance(appearance, BTreeMap::new());
    }

    /// Does what a mod asked of the app; true when the view has to run.
    fn request(&mut self, from: &str, request: Request, tx: &mut Tx) -> bool {
        match request {
            Request::Command(Command::Inspect) => {
                self.inspect_for(from);
                true
            },
            Request::Command(command) => {
                self.act(APP, &Action::Command(command), tx);
                true
            },
            Request::Open(url) => {
                if let Err(problem) = tx.open_url(&url) {
                    self.workspace
                        .refuse(from, Error::new(format!("{problem:?}")));
                }
                false
            },
            Request::Pick(pick) => {
                self.pick(from, pick, tx);
                false
            },
            Request::Appearance(appearance) => {
                self.set_appearance(appearance);
                true
            },
            other => {
                tracing::debug!("not on the Rune backend yet: {other:?} from {from}");
                false
            },
        }
    }

    /// Opens the panel `app:pick` asks for; Rune's return one path.
    fn pick(&self, from: &str, pick: Pick, tx: &mut Tx) {
        let mut options = FileDialogOptions {
            title: pick.prompt.clone().unwrap_or_default(),
            file_name: pick.name.clone(),
            directory: pick.folder.clone().map(PathBuf::from),
            ..FileDialogOptions::default()
        };
        if !pick.types.is_empty() {
            let types: Vec<&str> = pick.types.iter().map(String::as_str).collect();
            options = options.filter("Files", &types);
        }
        let owner = from.to_owned();
        let kind = pick.kind;
        let answer = move |ui: &mut Ui, picked: rune::FileDialogResult, _: &mut Tx| {
            let picked = picked.ok().flatten().map(|path| vec![path]);
            let answer = pick.answer(picked);
            ui.send(&owner, &owner, answer);
        };
        match kind {
            PickKind::Open => tx.open_file(options, answer),
            PickKind::Folder => tx.open_folder(options, answer),
            PickKind::Save => tx.save_file(options, answer),
        }
    }

    /// Runs what a control asks for, as `MoonframeApp::dispatch` does.
    pub(crate) fn act(&mut self, from: &str, action: &Action, tx: &mut Tx) {
        if let Action::Browser { to, id, command } = action {
            self.browser_command(to.as_deref().unwrap_or(from), id, *command, tx);
            return;
        }
        match self.workspace.dispatch(from, action) {
            Some(Command::Quit) => tx.quit(),
            Some(Command::Inspect) => self.inspect_for(from),
            Some(Command::CloseWindow) => tx.close_window(),
            Some(Command::Minimize) => tx.minimize(),
            Some(Command::Maximize) => tx.toggle_maximized(),
            Some(Command::Reload) => self.workspace.reload(),
            // The other of light and dark, whatever the system is.
            Some(Command::ToggleTheme) => self.set_appearance(if self.dark {
                Appearance::Light
            } else {
                Appearance::Dark
            }),
            Some(other) => tracing::debug!("not on the Rune backend yet: {other:?}"),
            None => {},
        }
    }

    /// Delivers a control's message to the mod `to`.
    pub(crate) fn send(&mut self, from: &str, to: &str, msg: Data) {
        self.workspace.send(from, to, msg);
    }

    /// Shares what providers reported, as `start_providers` does for GPUI.
    /// True when anything arrived.
    fn take_events(&mut self) -> bool {
        let events: Vec<Event> = self
            .events
            .as_ref()
            .map(|events| events.try_iter().collect())
            .unwrap_or_default();
        let changed = !events.is_empty();
        for event in events {
            match event {
                Event::Published { provider, data } => {
                    self.workspace.set_host(&provider, data);
                },
                // The newest there is by now, not what was first sent.
                Event::Newest { provider, newest } => {
                    if let Some(data) = newest.take() {
                        self.workspace.set_host(&provider, data);
                    }
                },
                Event::Link { provider, link } => {
                    self.links.insert(provider, link.data());
                    self.workspace
                        .set_host("links", Data::Map(self.links.clone()));
                },
                Event::Failed { provider, problem } => {
                    tracing::warn!("provider {provider}: {problem}");
                    let problem = Error::new(format!("provider failed: {problem}"));
                    self.workspace.clear_host(&provider, Some(problem));
                },
                Event::Sent { provider, to, msg } => {
                    self.workspace
                        .send(&format!("{PROVIDER}{provider}"), &to, msg);
                },
            }
        }
        changed
    }

    /// Starts what views now want, and passes on what mods sent providers.
    fn flush_outbox(&mut self) {
        let wanted = self.workspace.wanted_providers();
        let outbox = self.workspace.take_outbox();
        let Some(providers) = &mut self.providers else {
            return;
        };
        providers.want(wanted);
        for (provider, sender, message) in outbox {
            if let Err(problem) = providers.send(&provider, &sender, message) {
                self.workspace.refuse(&provider, problem);
            }
        }
    }

    /// Asks mods for the rows of each list that have scrolled into view.
    fn refill_lists(&mut self) -> bool {
        let mut changed = false;
        for list in self.lists.get_mut().values() {
            let (offset, height) = list.seen.get();
            let first = (offset / list.row_height).floor().max(0.) as usize;
            let showing = (height / list.row_height).ceil() as usize + 1;
            let shown = first..(first + showing).min(list.count);
            let built = list.window.start..list.window.start + list.window.len;
            if shown.start < built.start || shown.end > built.end {
                let start = first.saturating_sub(OVERSCAN);
                let window = ListWindow {
                    start,
                    len: (showing + 2 * OVERSCAN).min(list.count - start),
                };
                changed |= self
                    .workspace
                    .set_list_window(&list.owner, &list.id, window);
            }
        }
        changed
    }
}

/// `AppSpec` wants a plain `fn`; `App::init` hands it the real one.
fn unused_init() -> Ui {
    Ui::open(PathBuf::new(), None)
}

/// What [`COMPILED`] tells Luau.
fn compiled() -> BTreeMap<String, bool> {
    COMPILED
        .iter()
        .map(|(name, on)| ((*name).to_owned(), *on))
        .collect()
}

impl Ui {
    fn open(roots: impl Into<Roots>, store: Option<Box<dyn Persist>>) -> Self {
        let mut workspace = Workspace::open_with_capabilities(roots, true, store, compiled());
        // Rune's default window has macOS's buttons over its content, and a
        // native frame elsewhere: a view has to leave them room only there.
        let gallery = std::env::var_os("MOONFRAME_RUNE_GALLERY").is_some();
        workspace.show_demos(gallery);
        let platform = Platform::current();
        workspace.set_window(WindowState {
            titlebar: match platform {
                Platform::Macos => workspace.chrome().titlebar,
                Platform::Linux | Platform::Windows => Titlebar::System,
            },
            ..WindowState::default()
        });
        Self {
            workspace,
            dark: true,
            system_dark: Cell::new(true),
            fields: RefCell::default(),
            lists: RefCell::default(),
            panes: HashMap::new(),
            providers: None,
            events: None,
            links: BTreeMap::new(),
            images: None,
            pictures: RefCell::default(),
            gallery,
            appearance: Appearance::System,
            settings: None,
            natives: natives::natives(),
            views: HashMap::new(),
            preview: std::env::var("MOONFRAME_RUNE_GALLERY")
                .ok()
                .filter(|name| !name.is_empty() && name != "1"),
            terminals: terminal::Terminals::default(),
            inspecting: inspect::Inspecting::default(),
            crumbs: RefCell::default(),
            browsers: browser::Browsers::default(),
            videos: video::Videos::default(),
            map_tiles: map::Tiles::default(),
            menu_actions: Vec::new(),
            shown_menus: None,
            app: None,
            icon: None,
            about_replaced: false,
            layers: RefCell::default(),
            watching: None,
            settle: None,
            cell_width: Cell::new(None),
            waker: None,
            left_out: RefCell::default(),
        }
    }
}

/// Starts the providers: their events wake the window from their threads.
fn setup(ui: &mut Ui, waker: Waker) {
    ui.images = Some(ImageCache::new(Arc::clone(&waker)));
    ui.waker = Some(Arc::clone(&waker));
    let provider_wake = Arc::clone(&waker);
    let (sender, events) = channel();
    let providers = Providers::start_wanting(
        ui.workspace.roots().clone(),
        paths::builds(),
        Arc::new(move |event| {
            // Only fails once the window, and the receiver with it, has gone.
            if sender.send(event).is_ok() {
                provider_wake();
            }
        }),
        std::mem::take(&mut ui.natives),
        ui.workspace.wanted_providers(),
    );
    ui.providers = Some(providers);
    ui.watching = watch::watching(ui, Arc::clone(&waker));
    ui.events = Some(events);
    ui.settle = capture_delay();
    if let Some(delay) = ui.settle {
        std::thread::sleep(delay);
    }
}

/// As for the GPUI `capture` feature: a headless `--snap` runs on a virtual
/// clock, so it waits for what providers and shells take real time to say.
fn capture_delay() -> Option<Duration> {
    let delay = std::env::var("MOONFRAME_CAPTURE_DELAY").ok()?;
    Some(Duration::from_millis(delay.parse().ok()?))
}

fn view(ui: &Ui, cx: &mut Cx<Ui>) -> El {
    ui.system_dark.set(cx.system_dark());
    render::window(ui)
}

/// Each frame's housekeeping: the system's appearance, what providers said
/// and were sent, lists to refill, and what mods asked of a host this backend
/// does not have yet.
fn poll(ui: &mut Ui, _: f64, tx: &mut Tx) -> bool {
    let mut changed = false;
    // The first frames started what the view shows: let it answer, once.
    if (!ui.terminals.views.get_mut().is_empty() || ui.map_tiles.fetching())
        && let Some(delay) = ui.settle.take()
    {
        std::thread::sleep(delay);
    }
    // Only `system` follows the system: a choice of light or dark stands.
    let dark = ui.appearance.is_dark(ui.system_dark.get());
    if dark != ui.dark {
        ui.dark = dark;
        ui.workspace.set_dark(dark);
        changed = true;
    }
    changed |= ui.reload_changed();
    changed |= ui.settle_browsers();
    if let Some(cache) = &ui.images {
        changed |= ui.map_tiles.take_ready(cache);
    }
    // A failed tile comes up for another try: the view asks for what shows.
    if let Some(due) = ui.map_tiles.next_retry() {
        let now = Instant::now();
        if now >= due {
            changed = true;
        } else {
            tx.wake_after(due - now);
        }
    }
    ui.settle_maps(tx);
    ui.videos.retire();
    ui.sync_menus();
    if ui.app.is_some() && !ui.about_replaced {
        ui.about_replaced = menus::drop_system_about();
    }
    changed |= ui.take_events();
    changed |= ui.terminal_changed();
    ui.tell_exits();
    ui.settle_pick(tx);
    ui.tell_blurs();
    changed |= ui.refill_lists();
    ui.flush_outbox();
    for (sender, request) in ui.workspace.take_requests() {
        changed |= ui.request(&sender, request, tx);
    }
    changed
}

/// A key no control took.
fn on_key(ui: &mut Ui, event: &rune::KeyEvent, tx: &mut Tx) -> bool {
    ui.key(event, tx)
}
