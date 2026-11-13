//! Mods in windows of their own (`app:window`, or `window = "<mod>"` on a
//! control), as the GPUI app opens them.
//!
//! Rune gives each window a state of its own, and a handler only the state of
//! the window it was drawn in, so each window has its own [`Ui`]: what it
//! draws (fields, lists, popovers, terminals) is its own, and the workspace
//! every window draws from is [`Shared`]. The main window keeps the
//! providers, the menus and what mods ask of the app; a mod's window hands
//! the main one what only it does.
//!
//! Every window runs on the one event loop, so they share through `Rc` and
//! `RefCell`: a borrow of the workspace never outlives the call that took it.

use {
    crate::Ui,
    moonframe_core::{
        Action, Appearance, Command, Edit, ModWindowSize, Platform, SHELL, Titlebar, WindowPlace,
        WindowState, Workspace,
    },
    rune::{App, AppHandle, AppSpec, Chrome, Tx, WindowHandle, WindowOptions, WindowSpec},
    std::{
        cell::{Cell, Ref, RefCell, RefMut},
        collections::HashMap,
        rc::Rc,
        sync::Arc,
    },
};

/// The size a mod's window opens at, and its smallest, when its `window`
/// key does not say: the GPUI app's.
const SIZE: (f32, f32) = (420., 560.);
const MIN_SIZE: (f32, f32) = (280., 240.);

/// What every window of the app draws from.
pub(crate) struct Shared {
    workspace: RefCell<Workspace>,
    /// What the user chose: `system` follows it, light and dark stand.
    pub(crate) appearance: Cell<Appearance>,
    /// What each menu bar slot does; see `menus`. Any window's menu bar
    /// fires them.
    pub(crate) menu_actions: RefCell<Vec<Action>>,
    /// Counts the changes one window made that the others have to draw.
    generation: Cell<u64>,
    /// The text fields mods' windows draw, by their key (`<mod>:<id>`), and
    /// the mod whose window holds each: `app:edit` goes there.
    field_windows: RefCell<HashMap<String, String>>,
    /// Every window's handle, to wake it for such a change.
    handles: RefCell<Vec<AppHandle<Ui>>>,
    /// The main window's, which does what mods ask of the app.
    main: RefCell<Option<AppHandle<Ui>>>,
}

impl Shared {
    pub(crate) fn new(workspace: Workspace) -> Self {
        Self {
            workspace: RefCell::new(workspace),
            appearance: Cell::new(Appearance::System),
            menu_actions: RefCell::default(),
            generation: Cell::new(0),
            field_windows: RefCell::default(),
            handles: RefCell::default(),
            main: RefCell::default(),
        }
    }

    /// The workspace, to change, for a window that holds another of its
    /// own fields; it marks itself touched.
    pub(crate) fn workspace_mut(&self) -> RefMut<'_, Workspace> {
        self.workspace.borrow_mut()
    }

    /// A mod's window, `window`, drew the field `key`.
    pub(crate) fn drew_field(&self, key: &str, window: &str) {
        let mut fields = self.field_windows.borrow_mut();
        if fields.get(key).is_none_or(|known| known != window) {
            fields.insert(key.to_owned(), window.to_owned());
        }
    }

    /// The mod whose window last drew the field `key`, if a mod's window did.
    pub(crate) fn field_window(&self, key: &str) -> Option<String> {
        self.field_windows.borrow().get(key).cloned()
    }

    /// Every window's handle: they share the menu bar's items.
    pub(crate) fn handles(&self) -> Vec<AppHandle<Ui>> {
        self.handles.borrow().clone()
    }
}

impl Ui {
    /// The workspace, to read. Never held across a call that changes it.
    pub(crate) fn workspace(&self) -> Ref<'_, Workspace> {
        self.shared.workspace.borrow()
    }

    /// The workspace, to change; the other windows draw what changed.
    pub(crate) fn workspace_mut(&self) -> RefMut<'_, Workspace> {
        self.touched.set(true);
        self.shared.workspace.borrow_mut()
    }

    /// Whether this is the main window, not a mod's.
    pub(crate) fn is_main(&self) -> bool {
        self.shows.is_none()
    }

    /// A window that shows the mod `name`, drawing from `main`'s workspace.
    pub(crate) fn for_window(main: &Ui, name: &str) -> Self {
        let mut ui = Ui::new(Rc::clone(&main.shared), Some(name.to_owned()));
        ui.views = main.views.clone();
        ui.terminals.backends = Arc::clone(&main.terminals.backends);
        ui.icon = main.icon.clone();
        ui.dark = main.dark;
        ui.seen.set(main.shared.generation.get());
        ui
    }

    /// Tells the other windows this one changed the workspace: they draw
    /// again, and the main one does what mods now ask of the app.
    pub(crate) fn changed_workspace(&self) {
        let generation = self.shared.generation.get() + 1;
        self.shared.generation.set(generation);
        self.seen.set(generation);
        for handle in self.shared.handles.borrow().iter() {
            handle.wake();
        }
    }

    /// Whether another window changed the workspace since this one last
    /// looked: it draws again.
    pub(crate) fn others_changed(&self) -> bool {
        let generation = self.shared.generation.get();
        self.seen.replace(generation) != generation
    }

    /// Opens the mod `name`'s window, or raises it; `place` outranks where
    /// its `window` key puts it. Only the main window keeps their handles;
    /// false when the mod has none.
    pub(crate) fn open_mod_window(&mut self, name: &str, place: Option<WindowPlace>) -> bool {
        let Some(window) = self.mod_windows.get(name) else {
            return false;
        };
        if place.unwrap_or(window.place) != WindowPlace::default() {
            tracing::warn!(
                "`{name}` asks for a screen or full screen, which only GPUI does yet: it opens as an ordinary window"
            );
        }
        window.handle.update(|ui, _| ui.opened = true);
        window.native.open();
        let state = window.state(true);
        self.workspace_mut().set_mod_window(name, Some(state));
        true
    }

    /// Does `app:edit` from `from` where its field is: in the window that
    /// drew it, which keeps its text. The main window's own fields first.
    pub(crate) fn edit_where_drawn(&mut self, from: &str, edit: Edit) {
        let key = format!("{from}:{}", edit.field);
        let theirs = (!self.fields.get_mut().contains_key(&key))
            .then(|| self.shared.field_window(&key))
            .flatten()
            .and_then(|name| self.mod_windows.get(&name));
        if let Some(window) = theirs {
            let from = from.to_owned();
            window
                .handle
                .update(move |ui, _| ui.apply_edit(&from, &edit));
            return;
        }
        self.apply_edit(from, &edit);
    }

    /// Closes the mod `name`'s window, as `app:window` asks.
    pub(crate) fn close_mod_window(&mut self, name: &str) {
        let Some(window) = self.mod_windows.get(name) else {
            return;
        };
        window.handle.update(|ui, _| ui.opened = false);
        window.native.close();
        self.workspace_mut().set_mod_window(name, None);
    }

    /// Does what a mod's window cannot do itself: the commands the main
    /// window keeps (the menus, the appearance, Inspect, Quit), in the main
    /// window. False when this is the main window, which does them.
    pub(crate) fn hand_to_main(&self, from: &str, action: &Action) -> bool {
        if self.is_main() {
            return false;
        }
        let Some(main) = self.shared.main.borrow().clone() else {
            return false;
        };
        let (from, action) = (from.to_owned(), action.clone());
        main.update(move |ui, tx| ui.act(&from, &action, tx));
        true
    }
}

/// Whether a command is about the window it came from, which a mod's window
/// does itself.
pub(crate) fn window_command(command: Command) -> bool {
    matches!(
        command,
        Command::CloseWindow | Command::Minimize | Command::Maximize
    )
}

/// A mod's window, as the main window keeps it.
pub(crate) struct ModWindow {
    native: WindowHandle,
    handle: AppHandle<Ui>,
    titlebar: Titlebar,
    /// Where its `window` key asks it to open.
    place: WindowPlace,
}

impl ModWindow {
    /// What the mod reads as `app.window` while its window shows.
    fn state(&self, active: bool) -> WindowState {
        state(self.titlebar, active)
    }
}

/// `app.window` for a mod's window with `titlebar`.
fn state(titlebar: Titlebar, active: bool) -> WindowState {
    WindowState {
        platform: Platform::current(),
        titlebar,
        active,
        ..WindowState::default()
    }
}

/// The title bar a mod's window has: a standard one, as GPUI's, unless
/// its `window` key hides it, which macOS alone does (the mod then leaves
/// room for the window's buttons, as `app.window.titlebar` says).
fn chrome(sizing: &ModWindowSize) -> (Titlebar, Chrome) {
    match sizing.titlebar {
        Titlebar::Hidden if Platform::current() == Platform::Macos => (Titlebar::Hidden, Chrome {
            traffic_lights: sizing.traffic_lights,
            ..Chrome::DEFAULT
        }),
        _ => (Titlebar::System, Chrome {
            full_size_content: false,
            ..Chrome::DEFAULT
        }),
    }
}

/// Adds a window, hidden until a mod asks for it, for every mod of the
/// app: any can be opened in one, as on GPUI. A mod that first shows after
/// launch (a live edit adds it) has none until the next one.
pub(crate) fn add(app: &mut App<Ui>, main: &mut Ui) {
    let names: Vec<(String, String, ModWindowSize)> = {
        let workspace = main.workspace();
        workspace
            .mod_names()
            .filter(|name| *name != SHELL)
            .filter_map(|name| {
                let meta = workspace.slot(name)?.meta();
                let title = meta
                    .and_then(|meta| meta.title.clone())
                    .unwrap_or_else(|| name.to_owned());
                let sizing = meta.and_then(|meta| meta.window).unwrap_or_default();
                Some((name.to_owned(), title, sizing))
            })
            .collect()
    };
    let mut handles = vec![app.handle()];
    for (name, title, sizing) in names {
        let (size, min_size) = fitted(&sizing);
        // Rune takes a title for good; a handful per run.
        let title: &'static str = Box::leak(title.into_boxed_str());
        let spec = AppSpec {
            setup: Some(crate::setup),
            poll: Some(crate::poll),
            on_key: Some(crate::on_key),
            ..AppSpec::new(title, size, crate::unused_init, crate::view)
        };
        let (titlebar, chrome) = chrome(&sizing);
        let mut ui = Ui::for_window(main, &name);
        ui.titlebar = titlebar;
        // Its pages (`ui.browser`) go in its own native window.
        let parent = Rc::clone(&ui.browsers.parent);
        // Made when first opened, not at launch.
        let window = App::new(spec)
            .chrome(chrome)
            .windowless()
            .on_close(closed)
            .init(move || ui);
        let ready = window.handle();
        let window = window.on_ready(move |native| {
            *parent.borrow_mut() = Some(native.clone());
            // An idle window polls no more: what waited for it has to hear.
            ready.update(|_, _| {});
        });
        let spec = WindowSpec::from_app(name.clone(), window)
            .options(WindowOptions {
                min_size: Some(min_size),
                ..WindowOptions::default()
            })
            .on_focus(focused);
        let (handle, native) = app.window(spec);
        handles.push(handle.clone());
        main.mod_windows.insert(name, ModWindow {
            native,
            handle,
            titlebar,
            place: sizing.place,
        });
    }
    *main.shared.main.borrow_mut() = Some(app.handle());
    *main.shared.handles.borrow_mut() = handles;
}

/// The size a mod's window opens at and its smallest, as its `window` key
/// asks: as GPUI's, an explicit minimum grows the default size.
pub(crate) fn fitted(sizing: &ModWindowSize) -> ((f32, f32), (f32, f32)) {
    let (min_width, min_height) = sizing.min_size.unwrap_or(MIN_SIZE);
    let size = sizing
        .size
        .unwrap_or((SIZE.0.max(min_width), SIZE.1.max(min_height)));
    (size, (min_width.min(size.0), min_height.min(size.1)))
}

/// The user closed a mod's window: Rune hides it, and its mod hears that
/// it is back in the main window's context.
pub(crate) fn closed(ui: &mut Ui, _: &mut Tx) {
    ui.opened = false;
    if let Some(name) = ui.shows.clone() {
        ui.workspace_mut().set_mod_window(&name, None);
        ui.changed_workspace();
    }
}

/// A mod's window came to the front or left it, which its mod reads as
/// `app.window.active`.
pub(crate) fn focused(ui: &mut Ui, active: bool, _: &mut Tx) {
    let Some(name) = ui.shows.clone().filter(|_| ui.opened) else {
        return;
    };
    ui.workspace_mut()
        .set_mod_window(&name, Some(state(ui.titlebar, active)));
    ui.changed_workspace();
}

/// The windows the main one keeps, by the mod each shows.
pub(crate) type ModWindows = HashMap<String, ModWindow>;
