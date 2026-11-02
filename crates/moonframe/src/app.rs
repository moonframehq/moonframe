//! The window's one view: the workspace of Luau modules, reloaded when a
//! file in one of the app's folders changes.

use {
    crate::{
        actions::*,
        alerts::{Alerts, Posted},
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
        render::{Finished, KeyLayer, ListRequest, Painter},
        settings,
        terminal::Terminals,
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
        Action, Appearance, Chrome, Command, Data, Error, Platform, Request, Roots, SHELL, Sound,
        Stylesheet, Titlebar, Token, WindowRequest, WindowState, Workspace,
    },
    moonframe_mux::Backends,
    moonframe_providers::{Event, Natives, Providers},
    notify::{RecommendedWatcher, RecursiveMode, Watcher as _},
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
        rc::Rc,
        sync::Arc,
        time::Duration,
    },
};

// keep-sorted start
mod dialogs;
mod fonts;
mod history;
mod listeners;
mod menu;
mod panes;
mod picking;
mod toasts;
mod updates;
mod view;
mod watch;
mod windows;
// keep-sorted end

use {
    listeners::{deliver_from_views, flush_on_quit, report, start_providers, wake_on_output},
    menu::{hear_notification_clicks, is_dark, menu_actions, name_of, stock_preview},
    watch::watch,
};

/// Editors save in several steps; wait this long and take the burst as one.
const SETTLE: Duration = Duration::from_millis(120);
/// How long a toast shows.
const TOAST: Duration = Duration::from_millis(1600);
/// How long a notification shown as a toast does: it has more to read.
const NOTICE: Duration = Duration::from_secs(5);
/// gpui-component's stock 16px reads large in a dense tool window.
const DEFAULT_FONT_SIZE: Pixels = px(14.);

/// What the app adds to Moonframe in Rust: its native providers, and the
/// views its Luau places with `ui.view`.
pub(crate) struct Parts {
    pub(crate) natives: Natives,
    pub(crate) views: Registry,
}

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
    /// The shells `ui.terminal` elements show.
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
    /// The theme light windows show and the one dark windows show
    /// (`None`: the stock look), picked in View > Themes or by a mod; the
    /// one the picker shows while the pointer is over another; and the
    /// picker's window while it is open.
    light_scheme: Option<&'static Scheme>,
    dark_scheme: Option<&'static Scheme>,
    preview: Option<Option<&'static Scheme>>,
    /// Light, dark, or as the system is; and whether the system is dark.
    appearance: Appearance,
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
    updating: bool,
    /// When a mod last asked for a reload (`app:command`).
    requested_reload: Option<std::time::Instant>,
    /// The sounds and notifications mods ask for.
    alerts: Alerts,
    focus: FocusHandle,
    /// Where each animated pane is in its transition.
    motion: Motion,
    /// The app's providers, which publish data as `app.<name>`.
    pub(crate) providers: Providers,
    /// The repository of the terminal last used, which providers follow.
    pub(crate) project: crate::project::Project,
    /// The theme's own settings, restored when a stylesheet stops setting them.
    defaults: Defaults,
    /// Snapshots of the window's own folder, for Undo Last Change; `None`
    /// without a user's folder or git.
    history: Option<Arc<moonframe_history::History>>,
    /// Dropping it stops the events.
    _watcher: Option<RecommendedWatcher>,
    /// The main window, the one this view draws.
    main_window: WindowId,
    /// The open popovers and overlays each window's last frame drew, which
    /// take their `keys` before anything else in that window only.
    pub(crate) key_layers: HashMap<WindowId, Vec<KeyLayer>>,
    /// Sends keys to the topmost overlay or popover, else a focused
    /// terminal, before key bindings see them.
    _terminal_keys: Subscription,
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
    /// `folders` are the ones it was opened from.
    pub(crate) fn new(
        mut workspace: Workspace,
        chrome: Chrome,
        folders: Folders,
        backends: Arc<Backends>,
        parts: Parts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = cx.theme();
        let defaults = Defaults {
            font_size: DEFAULT_FONT_SIZE,
            font_family: theme.font_family.clone(),
            mono_font_size: theme.mono_font_size,
            mono_font_family: fonts::default_mono(cx, &theme.mono_font_family),
            radius: theme.radius,
        };
        workspace.set_dark(theme.is_dark());
        workspace.set_window(window_state(chrome, window));
        report(&workspace);
        cx.observe_window_bounds(window, |this, window, cx| this.sync_window(window, cx))
            .detach();
        cx.observe_window_activation(window, |this, window, cx| this.sync_window(window, cx))
            .detach();
        // The system's appearance, which `system` follows as it changes.
        cx.observe_window_appearance(window, |this, window, cx| {
            this.system_dark = is_dark(window.appearance());
            if this.appearance == Appearance::System {
                this.apply_theme(cx);
                cx.notify();
            }
        })
        .detach();
        let watcher = watch(&workspace, &folders, cx)
            .inspect_err(|error| tracing::warn!("not watching for changes: {error}"))
            .ok();
        let history = crate::history::open(&folders);
        let providers = start_providers(&workspace, parts.natives, cx);
        let views = Views::new(parts.views, deliver_from_views(cx));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        bind_menu_keys(workspace.menus(), cx);
        cx.set_menus(menus(workspace.components(), workspace.menus()));
        menu_actions(cx);
        let terminals = Terminals::new(backends, wake_on_output(cx));
        // Projects are kept only with the user's folder: `--no-user` runs
        // start fresh and keep nothing, as their state.
        let kept =
            (folders.user.as_ref()).map(|_| crate::project::kept_file(folders.profile.as_deref()));
        let project = crate::project::Project::start(kept, cx);
        flush_on_quit(cx);
        let settings = folders.settings().into_iter().next();
        let saved = |key| {
            folders
                .settings()
                .iter()
                .find_map(|path| settings::get(path, key))
        };
        // The one theme kept before light and dark had one each.
        let scheme = saved(settings::THEME).and_then(|name| {
            let scheme = themes::find(&name);
            if scheme.is_none() && name != themes::DEFAULT {
                tracing::warn!("no theme called {name:?}; using {}", themes::DEFAULT);
            }
            scheme
        });
        let slot = |key, dark: bool| match saved(key) {
            Some(name) => themes::fits(&name, dark)
                .inspect_err(|problem| {
                    tracing::warn!("{key}: {problem}; using {}", themes::DEFAULT);
                })
                .unwrap_or(None),
            None => scheme.filter(|scheme| scheme.dark() == dark),
        };
        let (light_scheme, dark_scheme) = (
            slot(settings::LIGHT_THEME, false),
            slot(settings::DARK_THEME, true),
        );
        // Before the appearance was kept, a theme picked set it.
        let appearance = match saved(settings::APPEARANCE) {
            Some(name) => Appearance::named(&name).unwrap_or_else(|| {
                tracing::warn!("no appearance called {name:?}; following the system");
                Appearance::System
            }),
            None => match scheme {
                Some(scheme) if scheme.dark() => Appearance::Dark,
                Some(_) => Appearance::Light,
                None => Appearance::System,
            },
        };
        show_appearance(appearance);
        let mut app = Self {
            terminals,
            browsers: Browsers::new(crate::browser::wake_on_report(cx)),
            inputs: Inputs::default(),
            views,
            workspace,
            chrome,
            asked: chrome,
            drag: false,
            inspector: Inspector::default(),
            chosen: String::new(),
            gallery: None,
            light_scheme,
            dark_scheme,
            preview: None,
            appearance,
            system_dark: is_dark(window.appearance()),
            themes: None,
            mod_windows: HashMap::new(),
            folders,
            settings,
            toast: None,
            toast_notice: None,
            toasts: 0,
            building: false,
            updating: false,
            requested_reload: None,
            alerts: Alerts::default(),
            focus,
            motion: Motion::default(),
            providers,
            project,
            defaults,
            main_window: window.window_handle().window_id(),
            key_layers: HashMap::new(),
            history,
            _watcher: watcher,
            _terminal_keys: crate::terminal::intercept_keys(cx),
            last_pane: 0,
        };
        app.apply_theme(cx);
        // What changed while the app was closed is a snapshot of its own.
        app.snapshot("launch", cx);
        hear_notification_clicks(cx);
        // What the mods' `init` sent providers as they started.
        app.forward("start", cx);
        app.take_panes(cx);
        // An installed app looks for updates by itself, once a day, also
        // while it stays open, whatever Developer Mode says; runs from
        // source and runs without the user's folder (checks, screenshots)
        // never do.
        if crate::update::checks_by_itself()
            && crate::update::source().is_some()
            && app.folders.user.is_some()
        {
            let profile = app.folders.profile.clone();
            cx.spawn(async move |this, cx| {
                loop {
                    if crate::update::due(profile.as_deref())
                        && this
                            .update(cx, |app, cx| app.check_for_updates(false, cx))
                            .is_err()
                    {
                        break;
                    }
                    cx.background_executor().timer(crate::update::POLL).await;
                    // The window is gone: so is the app.
                    if this.update(cx, |_, _| ()).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        app
    }

    /// Puts the picked theme (or the one previewed), then the stylesheet's
    /// fonts, radius, and color overrides into the global theme, so
    /// gpui-component controls follow them too. Starts from the stock theme
    /// each time, so a removed override goes away. A picked theme brings its
    /// own colors, so the stylesheet's apply to the stock look alone.
    fn apply_theme(&mut self, cx: &mut App) {
        let dark = self.appearance.is_dark(self.system_dark);
        // A theme shows in the appearance it is made for; in the other the
        // stock look does, and the theme stays picked.
        let scheme = themes::shown(self.preview, self.scheme(dark), dark);
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
                Data::from(name_of(self.light_scheme)),
            ),
            (
                "dark_theme".to_owned(),
                Data::from(name_of(self.dark_scheme)),
            ),
            (
                "themes".to_owned(),
                themes::listing(stock(false), stock(true)),
            ),
        ]);
        self.workspace.set_appearance(self.appearance, fields);
    }

    /// Makes the window light, dark, or follow the system, keeping the
    /// themes picked, and saves the choice for the next run.
    pub(crate) fn set_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        self.appearance = appearance;
        self.preview = None;
        self.apply_theme(cx);
        show_appearance(appearance);
        cx.set_menus(menus(self.workspace.components(), self.workspace.menus()));
        if let Some(path) = &self.settings
            && let Err(problem) = settings::save(path, &[(settings::APPEARANCE, appearance.name())])
        {
            tracing::warn!("cannot save the appearance: {problem}");
        }
        cx.notify();
    }

    /// Whether the window is dark.
    pub(crate) fn dark(&self) -> bool {
        self.appearance.is_dark(self.system_dark)
    }

    /// The theme picked for dark windows, or for light ones.
    fn scheme(&self, dark: bool) -> Option<&'static Scheme> {
        if dark {
            self.dark_scheme
        } else {
            self.light_scheme
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
        if preview == self.preview {
            return;
        }
        self.preview = preview;
        self.apply_theme(cx);
        cx.notify();
    }

    /// Picks the theme `name` for the appearance the window is in (View >
    /// Themes lists only those that fit it), and saves it for the next run.
    pub(crate) fn pick_theme(&mut self, name: &str, cx: &mut Context<Self>) {
        self.preview = None;
        let (light, dark) = if self.dark() {
            (None, Some(name))
        } else {
            (Some(name), None)
        };
        if let Err(problem) = self.set_themes(light, dark, cx) {
            tracing::warn!("{problem}");
        }
    }

    /// Picks the theme light windows show, the one dark windows show, or
    /// both, and saves them for the next run. Neither changes when a name
    /// is not a theme of its kind.
    pub(crate) fn set_themes(
        &mut self,
        light: Option<&str>,
        dark: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let light = light.map(|name| themes::fits(name, false)).transpose()?;
        let dark = dark.map(|name| themes::fits(name, true)).transpose()?;
        if let Some(scheme) = light {
            self.light_scheme = scheme;
        }
        if let Some(scheme) = dark {
            self.dark_scheme = scheme;
        }
        self.apply_theme(cx);
        if let Some(path) = &self.settings
            && let Err(problem) = settings::save(path, &[
                (settings::LIGHT_THEME, name_of(self.light_scheme)),
                (settings::DARK_THEME, name_of(self.dark_scheme)),
            ])
        {
            tracing::warn!("cannot save the themes: {problem}");
        }
        cx.notify();
        Ok(())
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
                self.show_mod_window(name, cx);
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
            Command::Quit => cx.quit(),
            Command::Reload => self.reload(cx),
            // The other of light and dark, whatever the system is.
            Command::ToggleTheme => self.set_appearance(
                if self.appearance.is_dark(self.system_dark) {
                    Appearance::Light
                } else {
                    Appearance::Dark
                },
                cx,
            ),
            Command::Minimize => window.minimize_window(),
            Command::Maximize => window.zoom_window(),
            Command::CloseWindow => window.remove_window(),
            Command::ShowThemes => self.show_themes(cx),
            Command::Inspect => self.inspect_for(SHELL, cx),
            Command::CheckForUpdates => self.check_for_updates(true, cx),
        }
    }

    /// Shares the window's state with the views as `app.window`.
    fn sync_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.workspace.set_window(window_state(self.chrome, window));
        cx.notify();
    }

    /// Starts the providers the views now want, and passes on what the mods
    /// sent providers: after a click, and after whatever else runs mods (a
    /// start, a reload, a provider's data), so a request from `init` or a
    /// topic is not left waiting for a click.
    fn forward(&mut self, from: &str, cx: &mut Context<Self>) {
        // What the views read and the mods hear now may want a provider
        // that hasn't started; one a message is for starts as it is sent.
        self.providers.want(self.workspace.wanted_providers());
        for (provider, sender, message) in self.workspace.take_outbox() {
            if let Err(problem) = self.providers.send(&provider, &sender, message) {
                tracing::warn!("{from}: {problem}");
            }
        }
        let mut requests = self.workspace.take_requests();
        // A reload runs every `init` again: one that asks for a reload
        // would loop, so what `init` asks as the app starts or reloads
        // reloads nothing.
        if matches!(from, "start" | "reload" | "packages") {
            requests.retain(|(asker, request)| {
                let reload = *request == Request::Command(Command::Reload);
                if reload {
                    tracing::warn!("{asker}: a reload asked for as the app loads is ignored");
                }
                !reload
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
                let folder = self
                    .workspace
                    .slot(from)
                    .and_then(|slot| slot.source())
                    .and_then(Path::parent)
                    .unwrap_or(Path::new("."));
                let allowed: Vec<PathBuf> = self
                    .workspace
                    .roots()
                    .layers()
                    .map(Path::to_owned)
                    .collect();
                let home = std::env::home_dir();
                let Some(file) = moonframe_core::resolve_file(
                    src,
                    folder,
                    home.as_deref(),
                    &allowed,
                    crate::alerts::MAX_SOUND_BYTES,
                ) else {
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
            Request::Pick(pick) => self.pick(from, pick, cx),
            Request::Edit(edit) => {
                if let Err(problem) = self.inputs.edit(from, &edit, window, cx) {
                    self.workspace
                        .refuse(from, Error::new(format!("`app:edit`: {problem}")));
                }
            },
            Request::Window(request) => self.window_request(from, request, cx),
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
        cx.set_menus(menus(self.workspace.components(), self.workspace.menus()));
        self.apply_theme(cx);
        report(&self.workspace);
        cx.notify();
    }

    /// Lays the app over again when the installed packages change: mods
    /// and providers that stay keep running, and the new folders are
    /// watched. False if the layers are the same.
    fn reload_packages(&mut self, cx: &mut Context<Self>) -> bool {
        let roots = self.folders.roots();
        if &roots == self.workspace.roots() {
            return false;
        }
        tracing::info!("packages changed: {roots}");
        crate::render::images::forget();
        self.workspace.set_roots(roots.clone());
        self.forward("packages", cx);
        self._watcher = watch(&self.workspace, &self.folders, cx)
            .inspect_err(|error| tracing::warn!("not watching for changes: {error}"))
            .ok();
        bind_menu_keys(self.workspace.menus(), cx);
        cx.set_menus(menus(self.workspace.components(), self.workspace.menus()));
        self.apply_theme(cx);
        report(&self.workspace);
        self.reload_providers(Some(roots), cx);
        true
    }

    /// Restarts the providers whose folders changed, rebuilding wasm ones,
    /// and stops sharing the data of providers that are gone; from `roots`
    /// instead, if given.
    fn reload_providers(&mut self, roots: Option<Roots>, cx: &mut Context<Self>) {
        let before: Vec<String> = self.providers.names().map(str::to_owned).collect();
        match roots {
            Some(roots) => self.providers.set_roots(roots),
            None => self.providers.reload(),
        }
        for gone in before
            .iter()
            .filter(|name| !self.providers.names().any(|now| now == name.as_str()))
        {
            self.workspace.clear_host(gone, None);
        }
        cx.notify();
    }
}

/// The window as views see it. The title bar is hidden only where the
/// system went along: a Linux compositor may keep its own decorations.
fn window_state(chrome: Chrome, window: &Window) -> WindowState {
    let platform = Platform::current();
    let titlebar = match (chrome.titlebar, platform, window.window_decorations()) {
        (Titlebar::Hidden, Platform::Linux, Decorations::Server) => Titlebar::System,
        (titlebar, ..) => titlebar,
    };
    WindowState {
        platform,
        titlebar,
        maximized: window.is_maximized(),
        fullscreen: window.is_fullscreen(),
        active: window.is_window_active(),
    }
}
