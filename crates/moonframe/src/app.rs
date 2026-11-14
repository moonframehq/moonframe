//! The window's one view: the workspace of Luau modules, reloaded when a
//! file in one of the app's folders changes.

use {
    crate::{
        actions::*,
        alerts::{Alerts, Posted},
        backends::Backends,
        browser::Browsers,
        color::rgba_color,
        gallery::Gallery,
        hosted::{Registry, Views},
        input::Inputs,
        inspect::{self, Inspector, Placed},
        mod_window::ModWindow,
        motion::Motion,
        packages,
        paths::{self, Folders},
        render::{Finished, KeyLayer, ListRequest, Painter, cached::Cached},
        settings,
        theme_picker::ThemePicker,
        themes::{self, Scheme},
    },
    futures::StreamExt as _,
    gpui_kit::{
        component::{ActiveTheme, Theme, ThemeMode, ThemeRegistry, ThemeTokens},
        prelude::FluentBuilder as _,
        *,
    },
    moonframe_core::{
        Action, AppIcon, Appearance, Chrome, Command, Data, Error, Platform, Request, Roots, SHELL,
        Sound, Stylesheet, Titlebar, Token, WindowRequest, WindowState, Workspace,
    },
    moonframe_providers::{Natives, Providers},
    notify::{RecommendedWatcher, RecursiveMode, Watcher as _},
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
        rc::Rc,
        sync::Arc,
        time::Duration,
    },
};

#[cfg(feature = "terminal")]
use {crate::terminal::Terminals, listeners::wake_on_output};

// keep-sorted start
mod appearance;
mod devtools;
mod dialogs;
mod entries;
mod fonts;
pub(crate) mod guard;
mod history;
mod hub;
mod icon;
mod listeners;
mod live;
mod live_settings;
mod machine;
mod main_window;
mod measuring;
mod menu;
mod menu_bar;
mod omarchy;
mod panes;
mod picking;
mod problems;
mod quick_inspect;
mod toasts;
mod updates;
mod view;
mod watch;
mod windows;
pub(crate) use fonts::list_ahead as list_fonts_ahead;
#[cfg(all(test, unix, feature = "markdown"))]
pub(crate) use picking::selection_file;

#[cfg(all(test, unix, feature = "markdown"))]
pub(crate) use hub::every as main_windows;
pub use hub::open_main_window;
pub(crate) use {
    hub::{Hub, Launch},
    listeners::display_number,
    main_window::MainWindow,
};
// keep-sorted end

use {
    appearance::Look,
    listeners::{deliver_from_views, flush_on_quit, report},
    menu::{is_dark, name_of, stock_preview},
    watch::watching,
};

/// Editors save in several steps; wait this long and take the burst as one.
const SETTLE: Duration = Duration::from_millis(120);
/// How long a toast shows.
const TOAST: Duration = Duration::from_millis(1600);
/// How long a notification shown as a toast does: it has more to read.
const NOTICE: Duration = Duration::from_secs(5);
/// gpui-component's stock 16px reads large in a dense tool window.
const DEFAULT_FONT_SIZE: Pixels = px(14.);

pub(crate) struct MoonframeApp {
    pub(crate) workspace: Workspace,
    /// What the window was opened with; it can't change while it is open.
    chrome: Chrome,
    /// What the shell last asked for, to say once that it waits for a restart.
    asked: Chrome,
    /// The pointer went down on a `drag = true` stack, and moving it now
    /// moves the window.
    pub(crate) drag: bool,
    /// Picking an element for an agent, and what was picked.
    pub(crate) inspector: Inspector,
    /// The stylesheet's and providers' problems: open, or dismissed.
    problem_state: problems::Problems,
    /// What the window hits, written for `--check` (see `crate::live`).
    live: crate::live::Live,
    /// The shells `ui.terminal` elements show.
    #[cfg(feature = "terminal")]
    pub(crate) terminals: Terminals,
    /// Saves what mods keep across launches; `None` keeps nothing.
    /// The pages `ui.browser` elements show.
    pub(crate) browsers: Browsers,
    /// The text `ui.input` fields edit.
    pub(crate) inputs: Inputs,
    /// The app's own views `ui.view` elements show.
    pub(crate) views: Views,
    /// The component the browser shows, and its window while it is open.
    chosen: String,
    gallery: Option<WindowHandle<component::Root>>,
    /// The themes and appearance picked, the same in every main window;
    /// whether the system is dark; and the picker's window while it is
    /// open.
    look: Look,
    system_dark: bool,
    themes: Option<WindowHandle<component::Root>>,
    /// Mods open in windows of their own (`window = "<mod>"`), by name.
    mod_windows: HashMap<String, WindowHandle<component::Root>>,
    /// The built-in app, the user's folder, and the profile, from which the
    /// app's layers are found again when the user installs a package.
    folders: Folders,
    /// Where the theme is saved: `settings.toml` in the user's folder, or
    /// the profile's, when
    /// there is one.
    settings: Option<PathBuf>,
    /// A short note at the bottom, such as "Copied", and which one it is,
    /// so an older note's timer does not hide a newer one.
    toast: Option<(u64, SharedString)>,
    /// The notification a toast shows in place of the system's (by the
    /// toast's id): who posted it and its `id`, told when it is clicked.
    toast_notice: Option<(u64, String, Option<String>)>,
    /// How many toasts have shown: each takes the next number as its id,
    /// never one an earlier toast (or its timer, or its notice) had.
    toasts: u64,
    /// A bundle is being built (Build App Bundle).
    building: bool,
    /// Checking for, or installing, an update.
    #[cfg(feature = "updates")]
    updating: bool,
    /// When a mod last asked for a reload (`app:command`).
    requested_reload: Option<std::time::Instant>,
    /// The sounds and notifications mods ask for.
    alerts: Alerts,
    focus: FocusHandle,
    /// Where each animated pane is in its transition.
    motion: Motion,
    cached: Cached,
    /// What the app's main windows share, the providers among it; and
    /// this window's number there, where provider replies for it go.
    pub(crate) hub: Entity<Hub>,
    id: u64,
    /// The repository of the terminal last used, which providers follow.
    #[cfg(feature = "terminal")]
    pub(crate) project: crate::project::Project,
    /// The theme's own settings, restored when a stylesheet stops setting them.
    defaults: Defaults,
    /// Snapshots of the window's own folder, for Undo Last Change; `None`
    /// without a user's folder or git.
    history: Option<Arc<crate::history::Kept>>,
    /// Dropping it stops the events.
    _watcher: Option<RecommendedWatcher>,
    _settings_watcher: Option<RecommendedWatcher>,
    quick_inspect: quick_inspect::Shortcut,
    devtools: devtools::Tools,
    _omarchy: Option<RecommendedWatcher>,
    /// The main window, the one this view draws.
    main_window: WindowId,
    /// The mod holding back closing and quitting, if one is.
    close_guard: Option<guard::CloseGuard>,
    awake: crate::awake::Awake,
    /// The open popovers and overlays each window's last frame drew, which
    /// take their `keys` before anything else in that window only.
    pub(crate) key_layers: HashMap<WindowId, Vec<KeyLayer>>,
    /// Sends keys to the topmost overlay or popover, else a focused
    /// terminal, before key bindings see them.
    _keys: Subscription,
    /// The number of the last tab `moonframe pane new` opened, which names
    /// it: each one's is higher, even for two in the same millisecond.
    last_pane: i64,
}

struct Defaults {
    font_size: Pixels,
    font_family: SharedString,
    mono_font_size: Pixels,
    mono_font_family: SharedString,
    radius: Pixels,
}

impl MoonframeApp {
    /// The window's view of `workspace`, in a window opened with `chrome`;
    /// `folders` are the ones it was opened from, and `views` the host
    /// views the app registered. It joins the app's [`Hub`], installed
    /// first.
    pub(crate) fn new(
        mut workspace: Workspace,
        chrome: Chrome,
        folders: Folders,
        backends: Arc<Backends>,
        views: Registry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        #[allow(clippy::expect_used)]
        let hub = hub::hub_of(cx).expect("the hub is installed before a main window opens");
        let joined = Hub::join(
            &hub,
            window.window_handle().window_id(),
            cx.weak_entity(),
            cx,
        );
        let theme = cx.theme();
        let defaults = Defaults {
            font_size: DEFAULT_FONT_SIZE,
            font_family: theme.font_family.clone(),
            mono_font_size: theme.mono_font_size,
            mono_font_family: fonts::default_mono(cx, &theme.mono_font_family),
            radius: theme.radius,
        };
        workspace.set_dark(theme.is_dark());
        workspace.set_window(window_state(chrome.titlebar, window, cx));
        // What the providers shared before this window opened.
        for (key, data) in joined.latest {
            workspace.set_host(&key, data);
        }
        report(&workspace);
        cx.observe_window_bounds(window, |this, window, cx| this.sync_window(window, cx))
            .detach();
        cx.observe_window_activation(window, |this, window, cx| this.sync_window(window, cx))
            .detach();
        // The system's appearance, which `system` follows as it changes.
        cx.observe_window_appearance(window, |this, window, cx| {
            this.system_dark = is_dark(window.appearance());
            if this.look.appearance == Appearance::System {
                this.apply_theme(cx);
                cx.notify();
            }
        })
        .detach();
        let watcher = watching(&workspace, &folders, cx);
        let messages = deliver_from_views(cx);
        crate::video::deliver_to(window.window_handle().window_id(), messages.clone());
        let views = Views::new(views, messages);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        bind_menu_keys(workspace.menus(), cx);
        crate::host_menus::install(workspace.menus(), cx);
        // Another window's terminals are local shells that end with it:
        // the first one's keep theirs, by the same names.
        #[cfg(feature = "terminal")]
        let terminals = Terminals::new(backends, wake_on_output(cx), joined.first);
        #[cfg(not(feature = "terminal"))]
        let _ = (backends, joined.first);
        #[cfg(feature = "terminal")]
        let project = crate::project::Project::start(
            (folders.user.as_ref()).map(|_| crate::project::kept_file(folders.profile.as_deref())),
            cx,
        );
        flush_on_quit(cx);
        Self::watch_closing(window, cx);
        Self::watch_displays(cx);
        let settings = folders.settings().into_iter().next();
        // Watch before reading, so a swap during startup cannot be missed.
        let omarchy = omarchy::watching(cx);
        themes::omarchy::load();
        // Another window looks as those open do, saved or not.
        let look = joined
            .look
            .unwrap_or_else(|| Look::saved(&folders, omarchy.is_some()));
        show_appearance(look.appearance);
        let settings_watcher = live_settings::watching(&folders, cx);
        let mut app = Self {
            #[cfg(feature = "terminal")]
            terminals,
            browsers: Browsers::new(crate::browser::wake_on_report(cx)),
            inputs: Inputs::default(),
            views,
            workspace,
            chrome,
            asked: chrome,
            drag: false,
            inspector: Inspector::default(),
            problem_state: problems::Problems::default(),
            live: crate::live::Live::new(folders.user.is_some(), joined.id, watcher.is_some()),
            chosen: String::new(),
            gallery: None,
            look,
            system_dark: is_dark(window.appearance()),
            themes: None,
            mod_windows: HashMap::new(),
            folders,
            settings,
            toast: None,
            toast_notice: None,
            toasts: 0,
            building: false,
            #[cfg(feature = "updates")]
            updating: false,
            requested_reload: None,
            alerts: Alerts::default(),
            focus,
            motion: Motion::default(),
            cached: Cached::default(),
            hub: joined.hub,
            id: joined.id,
            #[cfg(feature = "terminal")]
            project,
            defaults,
            main_window: window.window_handle().window_id(),
            close_guard: None,
            awake: Default::default(),
            key_layers: HashMap::new(),
            history: joined.history,
            _watcher: watcher,
            _settings_watcher: settings_watcher,
            quick_inspect: Default::default(),
            devtools: Default::default(),
            _omarchy: omarchy,
            _keys: crate::keys::intercept(cx),
            last_pane: 0,
        };
        app.load_measure();
        live::ticking(cx);
        app.load_inspect_shortcut(cx);
        app.apply_theme(cx);
        // What the mods' `init` sent providers as they started.
        app.forward("start", cx);
        app
    }

    /// Puts the picked theme (or the one previewed), then the stylesheet's
    /// fonts, radius, and color overrides into the global theme, so
    /// gpui-component controls follow them too. Starts from the stock theme
    /// each time, so a removed override goes away. A picked theme brings its
    /// own colors, so the stylesheet's apply to the stock look alone.
    fn apply_theme(&mut self, cx: &mut App) {
        let dark = self.dark();
        // A theme shows in the appearance it is made for; in the other the
        // stock look does, and the theme stays picked.
        let scheme = themes::shown(self.look.preview, self.scheme(dark), dark);
        let config = scheme.map(|scheme| Rc::new(scheme.config()));
        let was_dark = cx.theme().is_dark();
        let mode = if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        let registry = ThemeRegistry::global(cx);
        let (light, dark) = (
            registry.default_light_theme().clone(),
            registry.default_dark_theme().clone(),
        );
        let theme = Theme::global_mut(cx);
        theme.light_theme = light;
        theme.dark_theme = dark;
        match &config {
            Some(config) if config.mode.is_dark() => theme.dark_theme = config.clone(),
            Some(config) => theme.light_theme = config.clone(),
            None => {},
        }
        Theme::change(mode, None, cx);
        #[cfg(feature = "terminal")]
        self.terminals.set_palette(scheme.map(Scheme::palette));
        let sheet: &Stylesheet = self.workspace.stylesheet();
        let theme = Theme::global_mut(cx);
        theme.font_size = sheet.font_size.map_or(self.defaults.font_size, px);
        theme.font_family = sheet
            .font_family
            .clone()
            .map_or_else(|| self.defaults.font_family.clone(), SharedString::from);
        theme.mono_font_size = sheet.mono.size.map_or(self.defaults.mono_font_size, px);
        theme.mono_font_family = sheet.mono.family.clone().map_or_else(
            || self.defaults.mono_font_family.clone(),
            SharedString::from,
        );
        theme.radius = sheet.radius.map_or(self.defaults.radius, px);
        let dark = theme.is_dark();
        let overrides = if scheme.is_some() {
            &[][..]
        } else {
            &sheet.overrides[..]
        };
        for (token, shade) in overrides {
            let color = rgba_color(if dark {
                shade.dark
            } else {
                shade.light
            });
            let colors = &mut theme.colors;
            match token {
                Token::Background => colors.background = color,
                Token::Foreground => colors.foreground = color,
                Token::Muted => colors.muted = color,
                Token::MutedForeground => colors.muted_foreground = color,
                Token::Sidebar => colors.sidebar = color,
                Token::Card => colors.popover = color,
                Token::Secondary => colors.secondary = color,
                Token::Border => {
                    colors.border = color;
                    colors.sidebar_border = color;
                },
                Token::Accent => {
                    colors.primary = color;
                    colors.primary_hover = color.opacity(0.9);
                    colors.primary_active = color;
                    colors.button_primary = color;
                    colors.button_primary_hover = color.opacity(0.9);
                    colors.button_primary_active = color;
                    colors.ring = color.opacity(0.5);
                },
                Token::AccentForeground => {
                    colors.primary_foreground = color;
                    colors.button_primary_foreground = color;
                },
                Token::Success => colors.success = color,
                Token::Warning => colors.warning = color,
                Token::Danger => colors.danger = color,
                Token::ListHover => colors.list_hover = color,
                Token::ListActive => colors.list_active = color,
            }
        }
        theme.tokens = ThemeTokens::from(&theme.colors);
        Theme::sync_base(cx);
        cx.refresh_windows();
        if dark != was_dark {
            self.workspace.set_dark(dark);
        }
        let stock = |dark| stock_preview(&self.workspace.stylesheet().overrides, dark);
        let fields = std::collections::BTreeMap::from([
            ("theme".to_owned(), Data::from(self.theme_name())),
            (
                "light_theme".to_owned(),
                Data::from(name_of(self.look.light_scheme)),
            ),
            (
                "dark_theme".to_owned(),
                Data::from(name_of(self.look.dark_scheme)),
            ),
            (
                "themes".to_owned(),
                themes::listing(stock(false), stock(true)),
            ),
        ]);
        self.workspace.set_appearance(self.look.appearance, fields);
    }

    /// Makes the window light, dark, or follow the system, keeping the
    /// themes picked, and saves the choice for the next run.
    pub(crate) fn set_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        self.look.appearance = appearance;
        self.look.preview = None;
        self.apply_theme(cx);
        self.share_look(cx);
        show_appearance(appearance);
        crate::host_menus::install(self.workspace.menus(), cx);
        if let Some(path) = &self.settings
            && let Err(problem) = settings::save(path, &[(settings::APPEARANCE, appearance.name())])
        {
            tracing::warn!("cannot save the appearance: {problem}");
        }
        cx.notify();
    }

    /// Whether the window is dark. On Omarchy the system is as dark as its
    /// theme, which may tell the desktop so only after it has changed.
    pub(crate) fn dark(&self) -> bool {
        let omarchy = themes::omarchy::current().map(Scheme::dark);
        self.look
            .appearance
            .is_dark(omarchy.unwrap_or(self.system_dark))
    }

    /// The theme picked for dark windows, or for light ones.
    fn scheme(&self, dark: bool) -> Option<&'static Scheme> {
        if dark {
            self.look.dark_scheme
        } else {
            self.look.light_scheme
        }
    }

    /// The name of the theme picked for the appearance the window is in.
    pub(crate) fn theme_name(&self) -> &'static str {
        name_of(self.scheme(self.dark()))
    }

    /// Shows the theme `name` without picking it, or the picked one again
    /// when `None`.
    pub(crate) fn preview_theme(&mut self, name: Option<&str>, cx: &mut Context<Self>) {
        let preview = name.map(themes::find);
        if preview == self.look.preview {
            return;
        }
        self.look.preview = preview;
        self.apply_theme(cx);
        self.share_look(cx);
        cx.notify();
    }

    /// Opens the theme picker's window, or raises it.
    pub(crate) fn show_themes(&mut self, cx: &mut Context<Self>) {
        let open = self.themes.filter(|picker| {
            cx.windows()
                .iter()
                .any(|window| window.window_id() == picker.window_id())
        });
        // After this update, as for the component browser.
        match open {
            Some(picker) => cx.defer(move |cx| {
                let _ = picker.update(cx, |_, window, _| window.activate_window());
            }),
            None => {
                let app = cx.entity();
                cx.defer(move |cx| {
                    let picker = ThemePicker::open(app.clone(), cx);
                    app.update(cx, |app, _| app.themes = picker);
                });
            },
        }
    }

    pub(crate) fn dispatch(
        &mut self,
        from: &str,
        action: &Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Action::Browser { to, id, command } = action {
            let owner = to.as_deref().unwrap_or(from);
            // Only web addresses reach the system: a `file` one would open
            // whatever program the file is.
            if let Some(url) = self.browsers.command(owner, id, *command)
                && moonframe_core::is_remote_url(&url)
            {
                cx.open_url(&url);
            }
            return;
        }
        if let Action::Window(name) = action {
            // A preview opens nothing.
            if !from.starts_with(moonframe_core::DEMO) {
                self.show_mod_window(name, None, cx);
            }
            return;
        }
        if let Some(command) = self.workspace.dispatch(from, action) {
            if command == Command::Inspect {
                self.inspect_for(from, cx);
            } else {
                self.run(command, window, cx);
            }
        }
        self.flush(from, cx);
    }

    /// Sends `msg` to the mod `owner`'s own `update`, from the host: what
    /// one of its browsers reported.
    pub(crate) fn deliver(&mut self, owner: &str, msg: Data, cx: &mut Context<Self>) {
        self.workspace.send(owner, owner, msg);
        self.flush(owner, cx);
    }

    /// Has each mod build the list rows the frame just drawn showed without,
    /// once the frame is done, and draws again if any changed.
    fn request_list_rows(&self, requests: Vec<ListRequest>, cx: &mut Context<Self>) {
        if requests.is_empty() {
            return;
        }
        let this = cx.entity().downgrade();
        cx.defer(move |cx| {
            this.update(cx, |this, cx| {
                let mut changed = false;
                for (owner, id, window) in &requests {
                    changed |= this.workspace.set_list_window(owner, id, *window);
                }
                if changed {
                    this.flush("list", cx);
                }
            })
            .ok();
        });
    }

    /// Passes on what the mods sent providers, and draws again.
    fn flush(&mut self, from: &str, cx: &mut Context<Self>) {
        self.forward(from, cx);
        report(&self.workspace);
        cx.notify();
    }

    fn run(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            // Unless a mod in any window holds it back.
            Command::Quit => cx.defer(guard::quit),
            Command::Reload => self.reload(cx),
            // The other of light and dark, whatever the system is.
            Command::ToggleTheme => self.set_appearance(
                if self.dark() {
                    Appearance::Light
                } else {
                    Appearance::Dark
                },
                cx,
            ),
            Command::Minimize => window.minimize_window(),
            Command::Maximize => window.zoom_window(),
            Command::CloseWindow if self.held(guard::Closing::Window, cx) => {},
            Command::CloseWindow => window.remove_window(),
            Command::ShowThemes => self.show_themes(cx),
            Command::Inspect => self.inspect_for(SHELL, cx),
            Command::CheckForUpdates => self.check_for_updates(true, cx),
            Command::NewWindow => cx.defer(open_main_window),
            Command::Activate => {
                cx.activate(true);
                window.activate_window();
            },
        }
    }

    /// Shares the window's state with the views as `app.window`.
    fn sync_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.is_window_active() {
            let id = self.id;
            self.hub.update(cx, |hub, _| hub.raise(id));
        }
        self.workspace
            .set_window(window_state(self.chrome.titlebar, window, cx));
        cx.notify();
    }

    /// Starts the providers the views now want, and passes on what the mods
    /// sent providers: after a click, and after whatever else runs mods (a
    /// start, a reload, a provider's data), so a request from `init` or a
    /// topic is not left waiting for a click.
    fn forward(&mut self, from: &str, cx: &mut Context<Self>) {
        // What the views read and the mods hear now may want a provider
        // that hasn't started; one a message is for starts as it is sent.
        let wanted = self.workspace.wanted_providers();
        let outbox = self.workspace.take_outbox();
        let id = self.id;
        self.hub
            .update(cx, |hub, _| hub.send(id, wanted, outbox, from));
        let mut requests = self.workspace.take_requests();
        self.sweep_menu_bar((from == "reload").then_some(&requests[..]), cx);
        // A reload runs every `init` again, as a new window does: one that
        // asks for either would loop, so what `init` asks as the app (or a
        // window) starts or reloads reloads nothing and opens no window.
        if matches!(from, "start" | "reload" | "packages") {
            requests.retain(|(asker, request)| {
                let looping = match request {
                    Request::Command(Command::Reload) => Some("a reload"),
                    Request::Command(Command::NewWindow) => Some("a new window"),
                    _ => None,
                };
                if let Some(what) = looping {
                    tracing::warn!("{asker}: {what} asked for as the app loads is ignored");
                }
                looping.is_none()
            });
        }
        if !requests.is_empty() {
            self.carry_out(requests, cx);
        }
    }

    /// Does what mods asked of the app (`to = "app:<what>"`), once the
    /// update that asked is done, in the main window: a command there
    /// runs as a control's `command` would.
    fn carry_out(&mut self, requests: Vec<(String, Request)>, cx: &mut Context<Self>) {
        let main = self.main_window;
        let app = cx.entity();
        cx.defer(move |cx| {
            for (from, request) in requests {
                let target = match &request {
                    Request::Edit(edit) => app
                        .read(cx)
                        .inputs
                        .window(&from, &edit.field)
                        .unwrap_or(main),
                    _ => main,
                };
                let Some(window) = cx
                    .windows()
                    .into_iter()
                    .find(|window| window.window_id() == target)
                else {
                    continue;
                };
                let _ = window.update(cx, |_, window, cx| {
                    app.update(cx, |app, cx| {
                        app.request(&from, request, window, cx);
                        app.flush("app", cx);
                    });
                });
            }
        });
    }

    /// A notification the mod `from` posted was clicked: brings the app
    /// and its window forward and tells the mod, on `app/notification`:
    /// `{ id, clicked = true }`.
    fn notification_clicked(&mut self, from: &str, id: Option<String>, cx: &mut Context<Self>) {
        cx.activate(true);
        let main = self.main_window;
        if let Some(window) = cx
            .windows()
            .into_iter()
            .find(|window| window.window_id() == main)
        {
            let _ = window.update(cx, |_, window, _| window.activate_window());
        }
        let clicked = std::collections::BTreeMap::from([
            ("id".to_owned(), id.map_or(Data::Nil, Data::String)),
            ("clicked".to_owned(), Data::Bool(true)),
        ]);
        self.workspace
            .app_topic("notification", from, Data::Map(clicked));
        self.flush("notification", cx);
    }

    /// The toast was clicked: when it shows a notification, as a click on
    /// the system's one would be.
    fn click_toast(&mut self, cx: &mut Context<Self>) {
        let shown = self.toast.as_ref().map(|(id, _)| *id);
        let Some((id, from, notice)) = self.toast_notice.take() else {
            return;
        };
        if Some(id) != shown {
            return;
        }
        self.toast = None;
        self.notification_clicked(&from, notice, cx);
    }

    /// Plays what the mod `from` asked for: a file in the app's folders
    /// (as `ui.image` finds one), or the chime.
    fn play(&mut self, from: &str, sound: &Sound) {
        let file = match sound {
            Sound::Chime => None,
            Sound::File(src) => {
                let Some(file) = self.mod_file(from, src, crate::alerts::MAX_SOUND_BYTES) else {
                    self.workspace.refuse(
                        from,
                        Error::new(format!(
                            "`app:sound`: no sound file `{src}` of at most 2 MiB in the app's folders"
                        )),
                    );
                    return;
                };
                Some(file)
            },
        };
        if let Err(problem) = self.alerts.play(file.as_deref()) {
            tracing::warn!("{from}: {problem}");
        }
    }

    /// Does one thing the mod `from` asked of the app.
    fn request(
        &mut self,
        from: &str,
        request: Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match request {
            Request::Command(Command::Inspect) => self.inspect_for(from, cx),
            Request::Command(Command::Reload) => {
                // At most one a second, however mods ask.
                let now = std::time::Instant::now();
                if self
                    .requested_reload
                    .is_some_and(|at| now.duration_since(at) < Duration::from_secs(1))
                {
                    tracing::warn!(
                        "{from}: reloads asked for less than a second apart are ignored"
                    );
                    return;
                }
                self.requested_reload = Some(now);
                self.reload(cx);
            },
            Request::Command(command) => self.run(command, window, cx),
            Request::Appearance(appearance) => self.set_appearance(appearance, cx),
            Request::Sound(sound) => self.play(from, &sound),
            Request::Icon(icon) => self.show_icon(from, &icon),
            Request::Pick(pick) => self.pick(from, pick, cx),
            Request::Edit(edit) => {
                if let Err(problem) = self.inputs.edit(from, &edit, window, cx) {
                    self.workspace
                        .refuse(from, Error::new(format!("`app:edit`: {problem}")));
                }
            },
            Request::Window(request) => self.window_request(from, request, cx),
            Request::Guard(request) => self.guard_request(from, request, window, cx),
            Request::Awake(request) => self.awake_request(from, request, cx),
            Request::Dock(dock) => self.dock_request(from, &dock),
            Request::Restart(name) => self.restart_provider(from, &name, cx),
            Request::Devtools(request) => self.devtools_request(from, request, cx),
            Request::StatusItem(request) => self.status_item_request(from, request, cx),
            Request::Hotkey(request) => self.hotkey_request(from, request, cx),
            Request::Open(url) => {
                if self.alerts.may_open() {
                    cx.open_url(&url);
                } else {
                    tracing::warn!("{from}: `app:open` asked too often; {url} not opened");
                }
            },
            Request::Notify(notification) => match self.alerts.notify(from, &notification) {
                Ok(Posted::System) => {},
                Ok(Posted::Toast) => {
                    let text = if notification.body.is_empty() {
                        notification.title.clone()
                    } else {
                        format!("{}: {}", notification.title, notification.body)
                    };
                    self.show_toast_for(text.into(), NOTICE, cx);
                    // A click on it does what one on the system's would.
                    if let Some((shown, _)) = &self.toast {
                        self.toast_notice = Some((*shown, from.to_owned(), notification.id));
                    }
                },
                Err(problem) => tracing::warn!("{from}: {problem}"),
            },
            Request::Theme { light, dark } => {
                if let Err(problem) = self.set_themes(light.as_deref(), dark.as_deref(), cx) {
                    self.workspace
                        .refuse(from, Error::new(format!("`app:theme`: {problem}")));
                }
            },
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        crate::render::images::forget();
        self.workspace.reload();
        self.forward("reload", cx);
        let before = std::mem::replace(&mut self.asked, self.workspace.chrome());
        if self.asked != before && self.asked != self.chrome {
            tracing::warn!(
                "main.luau asks for {:?}; the window keeps {:?} until the app restarts",
                self.asked,
                self.chrome
            );
            self.show_toast("The window's title bar changes when the app restarts", cx);
        }
        bind_menu_keys(self.workspace.menus(), cx);
        crate::host_menus::install(self.workspace.menus(), cx);
        self.apply_theme(cx);
        report(&self.workspace);
        self.live.reloaded(self._watcher.is_some());
        cx.notify();
    }

    /// Lays the app over again when the installed packages change: mods
    /// and providers that stay keep running, and the new folders are
    /// watched. False if the layers are the same.
    fn reload_packages(&mut self, cx: &mut Context<Self>) -> bool {
        let roots = self.hub.read(cx).entry_roots(self.folders.roots());
        if &roots == self.workspace.roots() {
            return false;
        }
        tracing::info!("packages changed: {roots}");
        crate::render::images::forget();
        self.workspace.set_roots(roots.clone());
        self.forward("packages", cx);
        self.rewatch(cx);
        bind_menu_keys(self.workspace.menus(), cx);
        crate::host_menus::install(self.workspace.menus(), cx);
        self.apply_theme(cx);
        report(&self.workspace);
        self.live.reloaded(self._watcher.is_some());
        self.reload_providers(Some(roots), cx);
        true
    }

    /// Restarts the providers whose folders changed, rebuilding wasm ones,
    /// and stops sharing the data of providers that are gone, in every
    /// window; from `roots` instead, if given.
    fn reload_providers(&mut self, roots: Option<Roots>, cx: &mut Context<Self>) {
        let gone = self.hub.update(cx, |hub, _| hub.reload_providers(roots));
        for name in &gone {
            self.workspace.clear_host(name, None);
        }
        cx.notify();
        let me = self.id;
        cx.defer(move |cx| {
            for app in hub::every(cx) {
                app.update(cx, |app, cx| {
                    if app.id != me {
                        for name in &gone {
                            app.workspace.clear_host(name, None);
                        }
                        cx.notify();
                    }
                });
            }
        });
    }
}

/// The window as views see it. The title bar is hidden only where the
/// system went along: a Linux compositor may keep its own decorations.
pub(crate) fn window_state(titlebar: Titlebar, window: &Window, cx: &App) -> WindowState {
    let platform = Platform::current();
    let titlebar = match (titlebar, platform, window.window_decorations()) {
        (Titlebar::Hidden, Platform::Linux, Decorations::Server) => Titlebar::System,
        (titlebar, ..) => titlebar,
    };
    WindowState {
        platform,
        titlebar,
        maximized: window.is_maximized(),
        fullscreen: window.is_fullscreen(),
        active: window.is_window_active(),
        display: window
            .display(cx)
            .map(|display| display_number(display.id())),
    }
}
