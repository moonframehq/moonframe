//! The window's one view: the workspace of Luau modules, reloaded when a
//! file in one of the app's folders changes.

use {
    crate::{
        actions::*,
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
        *,
    },
    moonframe_core::{
        Action, Chrome, Command, Data, Error, Platform, Roots, SHELL, Stylesheet, Titlebar, Token,
        WindowState, Workspace,
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

mod listeners;
mod menu;
mod picking;
mod toasts;
mod updates;
mod view;
mod watch;
mod windows;

use {
    listeners::{deliver_from_views, flush_on_quit, report, start_providers, wake_on_output},
    menu::menu_actions,
    watch::watch,
};

/// Editors save in several steps; wait this long and take the burst as one.
const SETTLE: Duration = Duration::from_millis(120);
/// How long a toast shows.
const TOAST: Duration = Duration::from_millis(1600);
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
    /// The theme picked in View > Themes (`None`: the stock look), the one
    /// the picker shows while the pointer is over another, and the picker's
    /// window while it is open.
    scheme: Option<&'static Scheme>,
    preview: Option<Option<&'static Scheme>>,
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
    /// A bundle is being built (Build App Bundle).
    building: bool,
    /// Checking for, or installing, an update.
    updating: bool,
    focus: FocusHandle,
    /// Where each animated pane is in its transition.
    motion: Motion,
    /// The app's providers, which publish data as `app.<name>`.
    providers: Providers,
    /// The theme's own settings, restored when a stylesheet stops setting them.
    defaults: Defaults,
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
            mono_font_family: theme.mono_font_family.clone(),
            radius: theme.radius,
        };
        workspace.set_dark(theme.is_dark());
        workspace.set_window(window_state(chrome, window));
        report(&workspace);
        cx.observe_window_bounds(window, |this, window, cx| this.sync_window(window, cx))
            .detach();
        cx.observe_window_activation(window, |this, window, cx| this.sync_window(window, cx))
            .detach();
        let watcher = watch(&workspace, &folders, cx)
            .inspect_err(|error| tracing::warn!("not watching for changes: {error}"))
            .ok();
        let providers = start_providers(&workspace, parts.natives, cx);
        let views = Views::new(parts.views, deliver_from_views(cx));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        bind_menu_keys(workspace.menus(), cx);
        cx.set_menus(menus(workspace.components(), workspace.menus()));
        menu_actions(cx);
        let terminals = Terminals::new(backends, wake_on_output(cx));
        flush_on_quit(cx);
        let settings = folders.settings().into_iter().next();
        let scheme = folders
            .settings()
            .iter()
            .find_map(|path| settings::theme(path))
            .and_then(|name| {
                let scheme = themes::find(&name);
                if scheme.is_none() && name != themes::DEFAULT {
                    tracing::warn!("no theme called {name:?}; using {}", themes::DEFAULT);
                }
                scheme
            });
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
            scheme,
            preview: None,
            themes: None,
            mod_windows: HashMap::new(),
            folders,
            settings,
            toast: None,
            building: false,
            updating: false,
            focus,
            motion: Motion::default(),
            providers,
            defaults,
            main_window: window.window_handle().window_id(),
            key_layers: HashMap::new(),
            _watcher: watcher,
            _terminal_keys: crate::terminal::intercept_keys(cx),
        };
        app.apply_theme(cx);
        // What the mods' `init` sent providers as they started.
        app.forward("start");
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
        let scheme = self.preview.unwrap_or(self.scheme);
        let config = scheme.map(|scheme| Rc::new(scheme.config()));
        let was_dark = cx.theme().is_dark();
        let mode = config
            .as_ref()
            .map_or(cx.theme().mode, |config| config.mode);
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
    }

    /// The picked theme's name.
    pub(crate) fn theme_name(&self) -> &'static str {
        self.scheme.map_or(themes::DEFAULT, |scheme| scheme.name)
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

    /// Picks the theme `name` and saves it for the next run.
    pub(crate) fn pick_theme(&mut self, name: &str, cx: &mut Context<Self>) {
        self.preview = None;
        self.scheme = themes::find(name);
        self.apply_theme(cx);
        if let Some(path) = &self.settings
            && let Err(problem) = settings::save_theme(path, self.theme_name())
        {
            tracing::warn!("cannot save the theme: {problem}");
        }
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
        self.forward(from);
        report(&self.workspace);
        cx.notify();
    }

    fn run(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::Quit => cx.quit(),
            Command::Reload => self.reload(cx),
            Command::ToggleTheme => {
                // Back to the stock look, in the other mode.
                let mode = if cx.theme().is_dark() {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                };
                if self.scheme.is_some() {
                    self.pick_theme(themes::DEFAULT, cx);
                }
                Theme::change(mode, Some(window), cx);
                self.apply_theme(cx);
                cx.notify();
            },
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
    fn forward(&mut self, from: &str) {
        // What the views read and the mods hear now may want a provider
        // that hasn't started; one a message is for starts as it is sent.
        self.providers.want(self.workspace.wanted_providers());
        for (provider, sender, message) in self.workspace.take_outbox() {
            if let Err(problem) = self.providers.send(&provider, &sender, message) {
                tracing::warn!("{from}: {problem}");
            }
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        crate::render::images::forget();
        self.workspace.reload();
        self.forward("reload");
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
        self.forward("packages");
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
