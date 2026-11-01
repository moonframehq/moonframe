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
        // Tabs `moonframe pane new` asks for, also those left for this
        // window before it opened. Runs without the user's folder (checks,
        // screenshots) take none.
        if app.folders.user.is_some() {
            let inbox = crate::panes::inbox(app.folders.profile.as_deref());
            cx.spawn(async move |this, cx| {
                loop {
                    let requests = crate::panes::take(&inbox);
                    let open = this.update(cx, |app, cx| {
                        for request in requests {
                            app.open_pane(request, cx);
                        }
                    });
                    if open.is_err() {
                        break;
                    }
                    cx.background_executor().timer(crate::panes::POLL).await;
                }
            })
            .detach();
        }
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

impl MoonframeApp {
    /// Starts picking an element, or stops.
    pub(crate) fn toggle_inspect(&mut self, cx: &mut Context<Self>) {
        self.inspector.picking = !self.inspector.picking;
        self.inspector.hovered = None;
        if !self.inspector.picking {
            // Stopped without a pick: the mod that asked hears so.
            self.answer_asker(None);
        }
        cx.notify();
    }

    /// Starts picking for the mod `from`, which hears the pick (or that
    /// there was none) on `app/inspect` instead of a card opening.
    fn inspect_for(&mut self, from: &str, cx: &mut Context<Self>) {
        // A preview starts nothing.
        if from.starts_with(moonframe_core::DEMO) {
            return;
        }
        // A pick another mod started ends for it first, unanswered.
        if self
            .inspector
            .asker
            .as_deref()
            .is_some_and(|asker| asker != from)
        {
            self.answer_asker(None);
        }
        self.inspector.selected = None;
        self.inspector.asker = Some(from.to_owned());
        self.inspector.picking = true;
        self.inspector.hovered = None;
        // It hears that picking started, to get out of the way (a dialog).
        let started = std::collections::BTreeMap::from([
            ("asker".to_owned(), Data::String(from.to_owned())),
            ("picking".to_owned(), Data::Bool(true)),
        ]);
        self.workspace
            .app_topic("inspect", from, Data::Map(started));
        cx.notify();
    }

    /// Tells the mod that started the pick what was picked, if any:
    /// `{ asker, picking = false, picked, element?, mod?, at?, report? }`
    /// on `app/inspect`.
    fn answer_asker(&mut self, picked: Option<&Placed>) {
        let Some(asker) = self.inspector.asker.take() else {
            return;
        };
        let mut fields = std::collections::BTreeMap::from([
            ("asker".to_owned(), Data::String(asker.clone())),
            ("picking".to_owned(), Data::Bool(false)),
            ("picked".to_owned(), Data::Bool(picked.is_some())),
        ]);
        if let Some(picked) = picked {
            let pick = inspect::pick(picked, &self.workspace);
            fields.insert("element".to_owned(), Data::String(pick.element));
            fields.insert("mod".to_owned(), Data::String(pick.owner));
            fields.insert("at".to_owned(), Data::String(pick.at));
            fields.insert("report".to_owned(), Data::String(pick.report));
        }
        self.workspace
            .app_topic("inspect", &asker, Data::Map(fields));
    }

    /// Selects `picked` (or nothing) and saves it where agents read it.
    pub(crate) fn select(&mut self, picked: Option<Placed>, cx: &mut Context<Self>) {
        self.inspector.picking = false;
        self.inspector.hovered = None;
        let path = paths::selection();
        let saved = match &picked {
            Some(picked) => path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, inspect::report(picked, &self.workspace))),
            None => match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            },
        };
        match saved {
            Ok(()) if picked.is_some() => tracing::info!("selection saved to {}", path.display()),
            Ok(()) => {},
            Err(error) => {
                tracing::warn!("cannot save the selection to {}: {error}", path.display())
            },
        }
        if self.inspector.asker.is_some() {
            // Picked for a mod: it shows the pick itself, so no card.
            self.answer_asker(picked.as_ref());
            self.inspector.selected = None;
        } else {
            self.inspector.selected = picked;
        }
        cx.notify();
    }

    /// Copies the selection for an agent and closes its card. The saved
    /// file stays, so an agent told "here" still finds it.
    pub(crate) fn copy_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(selected) = self.inspector.selected.take() {
            let report = inspect::report(&selected, &self.workspace);
            self.copy(report, cx);
        }
        cx.notify();
    }

    pub(crate) fn copy(&mut self, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.show_toast("Copied", cx);
    }

    /// Whether `window` is the main window, not a mod's, the components
    /// browser or the theme picker.
    pub(crate) fn is_main(&self, window: &Window) -> bool {
        window.window_handle().window_id() == self.main_window
    }

    /// A keystroke no overlay, field or terminal took: a shell menu item
    /// whose shortcut is a single typing key (`j`, `shift-c`) runs, as a
    /// mail client's keys do, but only in the main window and while no
    /// text field or terminal has focus, so typing is never taken. Returns
    /// whether one ran.
    pub(crate) fn menu_keystroke(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.is_main(window)
            || self.inputs.any_focused(window, cx)
            || self.terminals.any_focused(window)
        {
            return false;
        }
        let modifiers = keystroke.modifiers;
        let pressed = moonframe_core::Pressed {
            key: &keystroke.key,
            text: keystroke.key_char.as_deref(),
            ctrl: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
            cmd: modifiers.platform,
        };
        let mac = cfg!(target_os = "macos");
        self.run_menu_item_where(window, cx, |_, item| {
            item.shortcut
                .as_ref()
                .is_some_and(|chord| chord.types_text() && chord.matches(&pressed, mac))
        })
    }

    /// The shell menu item whose (non-typing) shortcut is `keystroke`, in
    /// GPUI's spelling, runs. Returns whether one did.
    fn run_menu_key(
        &mut self,
        keystroke: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.run_menu_item_where(window, cx, |_, item| {
            item.shortcut.as_ref().is_some_and(|chord| {
                !chord.types_text() && crate::actions::keystroke(chord) == keystroke
            })
        })
    }

    /// Runs the shell menu item `id`, as a click from the shell would.
    fn run_menu_item(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.run_menu_item_where(window, cx, |title, item| item.id(title) == id) {
            tracing::warn!("no menu item {:?} any more", id.replace('\u{1f}', " > "));
        }
    }

    /// Runs the first shell menu item `matches` picks (given its menu's
    /// title). Returns whether there was one.
    fn run_menu_item_where(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        matches: impl Fn(&str, &moonframe_core::MenuCommand) -> bool,
    ) -> bool {
        let action = self.workspace.menus().iter().find_map(|menu| {
            menu.items.iter().find_map(|entry| match entry {
                moonframe_core::MenuEntry::Item(item) if matches(&menu.title, item) => {
                    Some(item.action.clone())
                },
                _ => None,
            })
        });
        match action {
            Some(action) => {
                self.dispatch(SHELL, &action, window, cx);
                true
            },
            None => false,
        }
    }

    /// The About panel: the app's name, icon and version, and what the
    /// shell's `about` says. macOS's own panel there; a toast elsewhere.
    pub(crate) fn show_about(&mut self, cx: &mut Context<Self>) {
        let about = self.workspace.about();
        let name = about.name.clone().unwrap_or_else(crate::title);
        if crate::dock::about_panel(&name, &about, self.folders.icon().as_deref()) {
            return;
        }
        let mut text = name;
        if let Some(version) = &about.version {
            text = format!("{text} {version}");
        }
        if let Some(description) = &about.description {
            text = format!("{text}: {description}");
        }
        self.show_toast_text(text.into(), cx);
    }

    /// Opens the tab `request` asks for in the main tab bar: a terminal in
    /// its folder (the window's own user folder by default), with the
    /// agent's command typed into it.
    fn open_pane(&mut self, request: crate::panes::Request, cx: &mut Context<Self>) {
        const OPEN: &str = "tabs/main/open";
        if !self.workspace.hears(OPEN) {
            tracing::warn!(
                "pane new: this app's shell has no main tab bar (`tabbed {{ id = \"main\" }}`)"
            );
            self.show_toast("This app has no main tab bar to open a pane in", cx);
            return;
        }
        // Unique, and above any number the editor gave a request.
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| {
                i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
            });
        let id = format!("pane-{n}");
        if let Some(bytes) = request.typed() {
            self.terminals.type_on_start(&id, bytes);
        }
        let mut props = Vec::new();
        if let Some(cwd) = request.cwd.clone().or_else(|| self.folders.own()) {
            props.push(("cwd", Data::from(cwd.to_string_lossy().into_owned())));
        }
        // The tab's id names its terminal (`<mod>/<tab id>`), so the
        // command goes to that one.
        let mut asked = vec![
            ("n", Data::from(n)),
            ("id", Data::from(id.as_str())),
            ("kind", Data::from("component:terminal")),
            ("props", Data::map(props)),
        ];
        if let Some(title) = request.title() {
            asked.push(("title", Data::from(title)));
        }
        let from = SHELL;
        self.workspace.send(
            from,
            &format!("{}{OPEN}", moonframe_core::TOPIC),
            Data::map(asked),
        );
        self.flush(from, cx);
        // Brought forward, to watch the agent; not when opened behind others.
        if !crate::background() {
            cx.activate(true);
        }
    }

    /// Shows `message` at the bottom for a moment.
    fn show_toast(&mut self, message: &'static str, cx: &mut Context<Self>) {
        self.show_toast_text(message.into(), cx);
    }

    /// Builds the app this window runs as a release bundle, in the
    /// background, and shows it in Finder when it is done.
    pub(crate) fn build_app(&mut self, cx: &mut Context<Self>) {
        if self.building {
            self.show_toast("The app is already being built", cx);
            return;
        }
        let Some(checkout) = crate::build::checkout(&self.folders.builtin) else {
            self.show_toast(
                "Building needs the app's sources: run it from its checkout",
                cx,
            );
            return;
        };
        let build = crate::build::Build {
            checkout,
            profile: self.folders.profile.clone(),
            user: self.folders.user.clone(),
        };
        self.building = true;
        // Stays up until the build ends: cargo can take minutes.
        let id = self.toast.as_ref().map_or(0, |(id, _)| id + 1);
        self.toast = Some((id, format!("Building {}…", crate::title()).into()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let built = cx
                .background_executor()
                .spawn(async move { build.run() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.building = false;
                match built {
                    Ok(bundle) => {
                        let name = bundle
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        crate::build::reveal(&bundle);
                        this.show_toast_text(format!("Built {name}").into(), cx);
                    },
                    Err(problem) => {
                        this.show_toast_text(format!("The build failed: {problem}").into(), cx);
                    },
                }
            });
        })
        .detach();
    }

    /// Asks the app's repository for a newer release and offers it. `asked`:
    /// the user chose Check for Updates, so every outcome is told; the
    /// daily check speaks only of a newer release.
    pub(crate) fn check_for_updates(&mut self, asked: bool, cx: &mut Context<Self>) {
        let Some(source) = crate::update::source().cloned() else {
            if asked {
                self.show_toast("This app names no repository to update from", cx);
            }
            return;
        };
        if self.updating {
            if asked {
                self.show_toast("Already checking for updates", cx);
            }
            return;
        }
        self.updating = true;
        if asked {
            self.show_toast("Checking for updates…", cx);
        }
        let profile = self.folders.profile.clone();
        cx.spawn(async move |this, cx| {
            let looked = source.clone();
            let found = cx
                .background_executor()
                .spawn(async move { crate::update::check(&looked) })
                .await;
            // Only a check that reached GitHub counts as the day's: one that
            // failed (offline, rate limited) is tried again within the hour.
            if found.is_ok() {
                crate::update::checked(profile.as_deref());
            }
            let title = crate::title();
            let release = match found {
                Ok(crate::update::Found::Newer(release)) => release,
                other => {
                    let _ = this.update(cx, |this, cx| {
                        this.updating = false;
                        let message = match other {
                            Ok(crate::update::Found::UpToDate { latest }) => {
                                format!("{title} is up to date ({latest})")
                            },
                            Ok(_) => format!("{title} has no release yet"),
                            Err(problem) => {
                                tracing::warn!("checking for updates: {problem}");
                                format!("Cannot check for updates: {problem}")
                            },
                        };
                        if asked {
                            this.show_toast_text(message.into(), cx);
                        }
                    });
                    return;
                },
            };
            let installable = release.installable(&source);
            let answers: &[&str] = match (installable, &release.asset) {
                (true, _) => &["Install and Relaunch", "Release Notes", "Later"],
                (false, Some(_)) => &["Download", "Release Notes", "Later"],
                (false, None) => &["Release Notes", "Later"],
            };
            let message = format!("{title} {} is available", release.version);
            let detail = if installable {
                format!(
                    "You have {}. It downloads and replaces this copy, then {title} starts again.",
                    source.version
                )
            } else {
                format!("You have {}.", source.version)
            };
            let window =
                cx.update(|cx| cx.active_window().or_else(|| cx.windows().first().copied()));
            let prompt = window.and_then(|window| {
                cx.update_window(window, |_, window, cx| {
                    window.prompt(PromptLevel::Info, &message, Some(&detail), answers, cx)
                })
                .ok()
            });
            let choice = match prompt {
                Some(answer) => answer
                    .await
                    .ok()
                    .and_then(|index| answers.get(index).copied()),
                None => None,
            };
            match choice {
                Some("Install and Relaunch") => {
                    let _ = this.update(cx, |this, cx| this.install_update(release, source, cx));
                    return;
                },
                Some("Download") => {
                    if let Some(asset) = &release.asset {
                        cx.update(|cx| cx.open_url(&asset.url));
                    }
                },
                Some("Release Notes") => cx.update(|cx| cx.open_url(&release.page)),
                _ => {},
            }
            let _ = this.update(cx, |this, _| this.updating = false);
        })
        .detach();
    }

    /// Downloads `release` and puts it in place of the running app, which
    /// quits and starts again; on a failure, says why and opens the
    /// release's page to download it from.
    fn install_update(
        &mut self,
        release: crate::update::Release,
        source: crate::update::Source,
        cx: &mut Context<Self>,
    ) {
        // Stays up until the download ends.
        let id = self.toast.as_ref().map_or(0, |(id, _)| id + 1);
        self.toast = Some((
            id,
            format!("Downloading {} {}…", crate::title(), release.version).into(),
        ));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let staged = cx
                .background_executor()
                .spawn({
                    let release = release.clone();
                    async move {
                        crate::update::stage(&release, &source).and_then(crate::update::relaunch)
                    }
                })
                .await;
            match staged {
                Ok(()) => {
                    tracing::info!("installing {} and starting again", release.version);
                    cx.update(|cx| cx.quit());
                },
                Err(problem) => {
                    tracing::warn!("installing the update: {problem}");
                    cx.update(|cx| cx.open_url(&release.page));
                    let _ = this.update(cx, |this, cx| {
                        this.updating = false;
                        this.show_toast_text(format!("The update failed: {problem}").into(), cx);
                    });
                },
            }
        })
        .detach();
    }

    fn show_toast_text(&mut self, message: SharedString, cx: &mut Context<Self>) {
        let id = self.toast.as_ref().map_or(0, |(id, _)| id + 1);
        self.toast = Some((id, message));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST).await;
            let _ = this.update(cx, |this, cx| {
                if this.toast.as_ref().is_some_and(|(shown, _)| *shown == id) {
                    this.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Shows the component `name` (or the last one, when empty) in the
    /// browser's window, opening it if it is closed.
    pub(crate) fn show_component(&mut self, name: &str, cx: &mut Context<Self>) {
        if !name.is_empty() {
            self.chosen = name.to_owned();
        }
        // The demos run only while their window is open.
        self.workspace.show_demos(true);
        cx.notify();
        let open = self.gallery.filter(|gallery| {
            cx.windows()
                .iter()
                .any(|window| window.window_id() == gallery.window_id())
        });
        // Both after this update: the action may have come from the
        // browser's own window, which cannot be updated while it is busy,
        // and a new window draws by asking the app.
        match open {
            Some(_) if crate::background() => {},
            Some(gallery) => cx.defer(move |cx| {
                let _ = gallery.update(cx, |_, window, _| window.activate_window());
            }),
            None => {
                let app = cx.entity();
                cx.defer(move |cx| {
                    let gallery = Gallery::open(app.clone(), cx);
                    app.update(cx, |app, _| app.gallery = gallery);
                });
            },
        }
    }

    /// Opens the mod `name` in a window of its own, or raises it.
    fn show_mod_window(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(slot) = self.workspace.slot(name) else {
            tracing::warn!("no mod `{name}` to open in a window");
            return;
        };
        let title = slot
            .meta()
            .and_then(|meta| meta.title.clone())
            .unwrap_or_else(|| name.to_owned());
        let open = self.mod_windows.get(name).copied().filter(|handle| {
            cx.windows()
                .iter()
                .any(|window| window.window_id() == handle.window_id())
        });
        // After this update, as for the components window: the click may
        // have come from that window, which is busy until it returns.
        match open {
            Some(_) if crate::background() => {},
            Some(handle) => cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.activate_window());
            }),
            None => {
                let app = cx.entity();
                let name = name.to_owned();
                cx.defer(move |cx| {
                    if let Some(handle) = ModWindow::open(app.clone(), name.clone(), title, cx) {
                        app.update(cx, |app, _| app.mod_windows.insert(name, handle));
                    }
                });
            },
        }
    }

    /// A mod window's content, and whether it is still moving.
    pub(crate) fn mod_window_content(
        &mut self,
        name: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, bool) {
        let mut painter = Painter::new(
            &self.workspace,
            &mut self.motion,
            &mut self.terminals,
            &self.inputs,
            None,
            cx.theme(),
        )
        .with_views(&mut self.views);
        let content = painter.window(name, cx);
        let frame = painter.finish();
        let animating = self.after_frame(frame, window, cx);
        (content, animating)
    }

    /// Does what a frame drawn in `window` left to do: keeps the keys of
    /// its open popovers and overlays, asks for the rows its lists want,
    /// and makes its views. Returns whether it still moves.
    fn after_frame(&mut self, frame: Finished, window: &Window, cx: &mut Context<Self>) -> bool {
        self.key_layers
            .insert(window.window_handle().window_id(), frame.key_layers);
        self.request_list_rows(frame.list_requests, cx);
        self.make_views(window, cx);
        frame.animating
    }

    /// Makes the views the frame drew that are not made yet, and gives the
    /// others their new props, once the frame is done; draws again to show
    /// what was made.
    fn make_views(&mut self, window: &Window, cx: &mut Context<Self>) {
        let requests = self.views.take_requests();
        if requests.is_empty() {
            return;
        }
        // In the window that drew them, which a view may want at its start.
        let this = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let _ = this.update(cx, |this, cx| {
                if this.views.apply(requests, window, cx) {
                    cx.notify();
                }
            });
        });
    }

    /// The browser window's content, and whether it is still moving.
    pub(crate) fn gallery_content(
        &mut self,
        query: &str,
        search: AnyElement,
        cx: &mut Context<Self>,
    ) -> (AnyElement, bool) {
        let mut painter = Painter::new(
            &self.workspace,
            &mut self.motion,
            &mut self.terminals,
            &self.inputs,
            None,
            cx.theme(),
        )
        .previewing();
        let content = painter.gallery(&self.chosen, query, search, cx);
        let frame = painter.finish();
        self.request_list_rows(frame.list_requests, cx);
        (content, frame.animating)
    }

    /// Asks an agent to add the component `name`, at the selection if any.
    pub(crate) fn copy_component_request(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(component) = self
            .workspace
            .components()
            .iter()
            .find(|component| component.name == name)
        else {
            return;
        };
        let place = match &self.inspector.selected {
            Some(selected) => format!(
                "at the selected element:\n\n{}",
                inspect::report(selected, &self.workspace)
            ),
            None => "where it fits (no element is selected; use View > Inspect Element to pick a place).".to_owned(),
        };
        let request = format!(
            "Add the `{name}` component ({}) {place}\nIt is used like this:\n\n{}\n",
            component.path.display(),
            component.usage
        );
        self.copy(request, cx);
    }

    /// Esc: stops picking, else clears the selection.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.inspector.picking {
            self.toggle_inspect(cx);
        } else if self.inspector.selected.is_some() {
            self.select(None, cx);
        }
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

/// The menu actions that need no particular window, handled app-wide, so
/// they work from the menu bar or their shortcut whichever window has focus,
/// or none (the app in the background, or every window minimized).
fn menu_actions(cx: &mut Context<MoonframeApp>) {
    let app = cx.entity().downgrade();
    // The app's own registry, not the view's of the same name.
    let cx: &mut App = cx;
    let on = app.clone();
    cx.on_action(move |_: &Reload, cx| {
        let _ = on.update(cx, |app, cx| app.reload(cx));
    });
    let on = app.clone();
    cx.on_action(move |_: &BrowseComponents, cx| {
        let _ = on.update(cx, |app, cx| app.show_component("", cx));
    });
    let on = app.clone();
    cx.on_action(move |action: &ShowComponent, cx| {
        let _ = on.update(cx, |app, cx| app.show_component(&action.name, cx));
    });
    let on = app.clone();
    cx.on_action(move |_: &ShowThemes, cx| {
        let _ = on.update(cx, |app, cx| app.show_themes(cx));
    });
    let on = app.clone();
    cx.on_action(move |_: &AboutApp, cx| {
        let _ = on.update(cx, |app, cx| app.show_about(cx));
    });
    let on = app.clone();
    cx.on_action(move |_: &BuildApp, cx| {
        let _ = on.update(cx, |app, cx| app.build_app(cx));
    });
    let on = app.clone();
    cx.on_action(move |_: &CheckForUpdates, cx| {
        let _ = on.update(cx, |app, cx| app.check_for_updates(true, cx));
    });
    let on = app.clone();
    cx.on_action(move |_: &ToggleDeveloperMode, cx| {
        let developer = crate::release();
        crate::set_developer(developer);
        let _ = on.update(cx, |app, cx| {
            if !developer && app.inspector.picking {
                app.toggle_inspect(cx);
            }
            cx.set_menus(menus(app.workspace.components(), app.workspace.menus()));
            app.show_toast(
                if developer {
                    "Developer Mode: the Components menu, Inspect Element (⌥⌘I) and Reload Modules (⌘R)"
                } else {
                    "Developer Mode is off"
                },
                cx,
            );
            cx.notify();
        });
        cx.refresh_windows();
    });
    cx.on_action(move |_: &ToggleSlowAnimations, cx| {
        crate::motion::set_slow(!crate::motion::is_slow());
        // The menu shows the switch checked while it is on.
        let _ = app.update(cx, |app, cx| {
            cx.set_menus(menus(app.workspace.components(), app.workspace.menus()));
        });
        cx.refresh_windows();
    });
}

/// A callback for terminal sessions, which run on threads of their own: each
/// call asks for one more frame, and a burst of calls makes one.
fn wake_on_output(cx: &mut Context<MoonframeApp>) -> Arc<dyn Fn() + Send + Sync> {
    let (sender, mut woken) = futures::channel::mpsc::unbounded::<()>();
    cx.spawn(async move |this, cx| {
        while woken.next().await.is_some() {
            while woken.try_recv().is_ok() {}
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    })
    .detach();
    Arc::new(move || {
        let _ = sender.unbounded_send(());
    })
}

/// A pill at the bottom center, over everything.
fn toast(message: SharedString, cx: &Context<MoonframeApp>) -> AnyElement {
    let theme = cx.theme();
    deferred(
        div()
            .absolute()
            .bottom(px(48.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .bg(theme.foreground)
                    .text_color(theme.background)
                    .text_sm()
                    .shadow_md()
                    .child(message)
                    .child(crate::browser::cover()),
            ),
    )
    .with_priority(10)
    .into_any_element()
}

/// Where the app's own views send messages: each goes to the `update` of
/// the mod that drew the view.
fn deliver_from_views(
    cx: &mut Context<MoonframeApp>,
) -> futures::channel::mpsc::UnboundedSender<(String, Data)> {
    let (sender, mut messages) = futures::channel::mpsc::unbounded::<(String, Data)>();
    cx.spawn(async move |this, cx| {
        while let Some((owner, msg)) = messages.next().await {
            if this
                .update(cx, |this, cx| this.deliver(&owner, msg, cx))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
    sender
}

/// Starts the providers, sharing what each publishes as `app.<name>`.
fn start_providers(
    workspace: &Workspace,
    natives: Natives,
    cx: &mut Context<MoonframeApp>,
) -> Providers {
    let (sender, mut events) = futures::channel::mpsc::unbounded();
    // Only those the first views read or the mods hear, and those that
    // always start: the rest wait until something wants them.
    let providers = Providers::start_wanting(
        workspace.roots().clone(),
        paths::builds(),
        Arc::new(move |event| {
            let _ = sender.unbounded_send(event);
        }),
        natives,
        workspace.wanted_providers(),
    );
    cx.spawn(async move |this, cx| {
        while let Some(event) = events.next().await {
            let handled = this.update(cx, |this, cx| {
                match event {
                    Event::Published { provider, data } => {
                        // No view read it: the window looks the same.
                        let drawn = this.workspace.set_host(&provider, data);
                        // Mods that heard it may have asked a provider.
                        this.forward(&provider);
                        if drawn > 0 {
                            // A view may fail on the new data: say so.
                            report(&this.workspace);
                            cx.notify();
                        }
                    },
                    Event::Failed { provider, problem } => {
                        tracing::warn!("provider {provider}: {problem}");
                        let problem = Error::new(format!("provider failed: {problem}"));
                        this.workspace.clear_host(&provider, Some(problem));
                        cx.notify();
                    },
                    // A native provider's answer to the mod that asked.
                    Event::Sent { provider, to, msg } => {
                        let from = format!("{}{provider}", moonframe_core::PROVIDER);
                        this.workspace.send(&from, &to, msg);
                        this.flush(&from, cx);
                    },
                }
            });
            if handled.is_err() {
                break;
            }
        }
    })
    .detach();
    providers
}

/// Logs each module's problem, so a terminal shows what the window does.
fn report(workspace: &Workspace) {
    for (name, error) in workspace.problems() {
        tracing::warn!("{name}: {error}");
    }
}

/// What a burst of file events touched.
#[derive(Clone, Copy, Default)]
struct Changes {
    modules: bool,
    providers: bool,
    /// `packages.toml`: a package was installed, removed, or turned on or off.
    packages: bool,
}

/// Watches every layer, and the list of packages in the window's own
/// folder. Other profiles, inside the user's folder, are none of its
/// business.
fn watch(
    workspace: &Workspace,
    folders: &Folders,
    cx: &mut Context<MoonframeApp>,
) -> notify::Result<RecommendedWatcher> {
    let (sender, mut changes) = futures::channel::mpsc::unbounded();
    // Events name absolute, resolved paths; compare against the same.
    let layers: Vec<PathBuf> = workspace
        .roots()
        .layers()
        .filter(|layer| layer.is_dir())
        .map(Path::to_owned)
        .collect();
    let resolved = |folder: &Path| folder.canonicalize().unwrap_or_else(|_| folder.to_owned());
    let own = folders.own().map(|own| resolved(&own));
    let list = own.as_ref().map(|own| own.join(packages::FILE));
    let others = folders
        .user
        .as_deref()
        .map(|user| resolved(&paths::profiles_folder(user)));
    let providers: Vec<PathBuf> = layers
        .iter()
        .map(|layer| {
            layer
                .canonicalize()
                .unwrap_or_else(|_| layer.clone())
                .join("providers")
        })
        .collect();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        if event.kind.is_access() {
            return;
        }
        let mut found = Changes::default();
        for path in &event.paths {
            if others
                .as_ref()
                .is_some_and(|others| path.starts_with(others))
                && !own.as_ref().is_some_and(|own| path.starts_with(own))
            {
                continue;
            }
            // Git writes its own files as a package is fetched; the
            // checkout it makes is what counts.
            if path.components().any(|part| part.as_os_str() == ".git") {
                continue;
            }
            if list.as_deref() == Some(path.as_path()) {
                found.packages = true;
            } else if providers
                .iter()
                .any(|providers| path.starts_with(providers))
            {
                // Builds write their lock and output; those must not loop
                // back into a rebuild.
                let built = path.components().any(|part| part.as_os_str() == "target")
                    || path.file_name().is_some_and(|name| name == "Cargo.lock");
                found.providers |= !built && !path.is_dir();
            } else {
                found.modules |= path
                    .extension()
                    .is_some_and(|extension| extension == "luau")
                    || path.is_dir();
            }
        }
        if found.modules || found.providers || found.packages {
            let _ = sender.unbounded_send(found);
        }
    })?;
    // Packages are cloned inside the user's folder, already watched.
    for (index, layer) in layers.iter().enumerate() {
        let nested = layers
            .iter()
            .enumerate()
            .any(|(other, outer)| other != index && layer.starts_with(outer) && layer != outer);
        if !nested {
            watcher.watch(layer, RecursiveMode::Recursive)?;
        }
    }
    cx.spawn(async move |this, cx| {
        while let Some(first) = changes.next().await {
            cx.background_executor().timer(SETTLE).await;
            let mut burst = first;
            while let Ok(more) = changes.try_recv() {
                burst.modules |= more.modules;
                burst.providers |= more.providers;
                burst.packages |= more.packages;
            }
            let reloaded = this.update(cx, |this, cx| {
                // Lays the app over again, reloading everything in it.
                if burst.packages && this.reload_packages(cx) {
                    return;
                }
                if burst.modules {
                    this.reload(cx);
                }
                if burst.providers {
                    this.reload_providers(None, cx);
                }
            });
            if reloaded.is_err() {
                break;
            }
        }
    })
    .detach();
    Ok(watcher)
}

impl Render for MoonframeApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = std::time::Instant::now();
        let gallery_open = self.gallery.is_some_and(|gallery| {
            cx.windows()
                .iter()
                .any(|window| window.window_id() == gallery.window_id())
        });
        if !gallery_open {
            self.workspace.show_demos(false);
        }
        self.inputs.begin_frame();
        let last_keyed = self.motion.begin_frame();
        self.terminals.begin_frame();
        let mut painter = Painter::new(
            &self.workspace,
            &mut self.motion,
            &mut self.terminals,
            &self.inputs,
            self.inspector.sink(),
            cx.theme(),
        )
        .with_browsers(&mut self.browsers)
        .with_views(&mut self.views)
        .with_keyed(&last_keyed);
        let body = painter.shell(cx);
        let frame = painter.finish();
        let animating = frame.animating;
        if animating {
            window.request_animation_frame();
        }
        // Building the element tree from the views; GPUI lays it out and
        // paints it after. Slow frames are worth seeing in the log; fast
        // ones while animating would flood even `debug`.
        let built = started.elapsed();
        if built >= moonframe_core::perf::SLOW || !animating {
            moonframe_core::perf!(built, "frame: built in {built:.1?}");
        }
        // Closed windows' layers go: their ids are not reused while open.
        let open: Vec<WindowId> = cx
            .windows()
            .iter()
            .map(AnyWindowHandle::window_id)
            .collect();
        self.key_layers.retain(|id, _| open.contains(id));
        self.after_frame(frame, window, cx);
        // A terminal that just appeared (a new tab, or one selected) takes
        // the keyboard, once it is drawn.
        if let Some(focus) = self.terminals.end_frame() {
            window.defer(cx, move |window, cx| focus.focus(window, cx));
        }
        let layers = self.inspector_layers(window.viewport_size(), cx);
        let toast = self
            .toast
            .as_ref()
            .map(|(_, message)| toast(message.clone(), cx));
        let theme = cx.theme();
        div()
            .id("moonframe-app")
            .key_context(if crate::release() {
                "MoonframeApp"
            } else {
                "MoonframeApp developer"
            })
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                this.run(Command::ToggleTheme, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Inspect, _, cx| this.toggle_inspect(cx)))
            .on_action(cx.listener(|this, action: &MenuSend, window, cx| {
                this.run_menu_item(&action.id, window, cx);
            }))
            .on_action(cx.listener(|this, action: &MenuKey, window, cx| {
                if !this.run_menu_key(&action.keystroke, window, cx) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &Dismiss, _, cx| this.dismiss(cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font_family.clone())
            .text_size(theme.font_size)
            .relative()
            // The shell takes what the branch bar leaves.
            .child(div().flex_1().min_h_0().child(body))
            .children(crate::branch_bar::label().map(crate::branch_bar::render))
            .children(layers)
            .children(toast)
            .child(crate::browser::presenter(self.browsers.pages()))
    }
}

/// Saves the state that changed since the last save, waiting for it, as
/// the app quits: the workspace saves as it goes, but writes are queued.
fn flush_on_quit(cx: &mut Context<MoonframeApp>) {
    cx.on_app_quit(|app, _| {
        app.workspace.flush();
        async {}
    })
    .detach();
}
