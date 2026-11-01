//! The window's one view: the workspace of Luau modules, reloaded when a
//! file in one of the app's folders changes.

use {
    crate::{
        actions::*,
        browser::Browsers,
        color::rgba_color,
        gallery::Gallery,
        input::Inputs,
        inspect::{self, Inspector, Placed},
        mod_window::ModWindow,
        motion::Motion,
        packages, paths,
        render::{KeyLayer, Painter},
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
        Action, Chrome, Command, Data, Error, Platform, Roots, Stylesheet, Titlebar, Token,
        WindowState, Workspace,
    },
    moonframe_providers::{Event, Providers},
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
    /// The pages `ui.browser` elements show.
    pub(crate) browsers: Browsers,
    /// The text `ui.input` fields edit.
    pub(crate) inputs: Inputs,
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
    /// The built-in app and the user's folder, from which the app's layers
    /// are found again when the user installs a package.
    builtin: PathBuf,
    user: Option<PathBuf>,
    /// Where the theme is saved: `settings.toml` in the user's folder, when
    /// there is one.
    settings: Option<PathBuf>,
    /// A short note at the bottom, such as "Copied", and which one it is,
    /// so an older note's timer does not hide a newer one.
    toast: Option<(u64, SharedString)>,
    focus: FocusHandle,
    /// Where each animated pane is in its transition.
    motion: Motion,
    /// The app's providers, which publish data as `app.<name>`.
    providers: Providers,
    /// The theme's own settings, restored when a stylesheet stops setting them.
    defaults: Defaults,
    /// Dropping it stops the events.
    _watcher: Option<RecommendedWatcher>,
    /// The open popovers and overlays the last frame drew, which take
    /// their `keys` before anything else.
    pub(crate) key_layers: Vec<KeyLayer>,
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
    /// `builtin` and `user` are the folders it was opened from.
    pub(crate) fn new(
        mut workspace: Workspace,
        chrome: Chrome,
        builtin: PathBuf,
        user: Option<PathBuf>,
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
        let watcher = watch(&workspace, user.as_deref(), cx)
            .inspect_err(|error| tracing::warn!("not watching for changes: {error}"))
            .ok();
        let providers = start_providers(&workspace, cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        cx.set_menus(menus(workspace.components()));
        menu_actions(cx);
        let terminals = Terminals::new(wake_on_output(cx));
        let settings = user.as_ref().map(|user| user.join("settings.toml"));
        let scheme = settings
            .as_deref()
            .and_then(settings::theme)
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
            builtin,
            user,
            settings,
            toast: None,
            focus,
            motion: Motion::default(),
            providers,
            defaults,
            key_layers: Vec::new(),
            _watcher: watcher,
            _terminal_keys: crate::terminal::intercept_keys(cx),
        };
        app.apply_theme(cx);
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
            self.run(command, window, cx);
        }
        self.flush(from, cx);
    }

    /// Sends `msg` to the mod `owner`'s own `update`, from the host: what
    /// one of its browsers reported.
    pub(crate) fn deliver(&mut self, owner: &str, msg: Data, cx: &mut Context<Self>) {
        self.workspace.send(owner, owner, msg);
        self.flush(owner, cx);
    }

    /// Passes on what the mods sent providers, and draws again.
    fn flush(&mut self, from: &str, cx: &mut Context<Self>) {
        for (provider, message) in self.workspace.take_outbox() {
            if let Err(problem) = self.providers.send(&provider, message) {
                tracing::warn!("{from}: {problem}");
            }
        }
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
        }
    }

    /// Shares the window's state with the views as `app.window`.
    fn sync_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.workspace.set_window(window_state(self.chrome, window));
        cx.notify();
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.workspace.reload();
        let before = std::mem::replace(&mut self.asked, self.workspace.chrome());
        if self.asked != before && self.asked != self.chrome {
            tracing::warn!(
                "main.luau asks for {:?}; the window keeps {:?} until the app restarts",
                self.asked,
                self.chrome
            );
            self.show_toast("The window's title bar changes when the app restarts", cx);
        }
        cx.set_menus(menus(self.workspace.components()));
        self.apply_theme(cx);
        report(&self.workspace);
        cx.notify();
    }

    /// Lays the app over again when the installed packages change: mods
    /// and providers that stay keep running, and the new folders are
    /// watched. False if the layers are the same.
    fn reload_packages(&mut self, cx: &mut Context<Self>) -> bool {
        let roots = paths::roots(self.builtin.clone(), self.user.clone());
        if &roots == self.workspace.roots() {
            return false;
        }
        tracing::info!("packages changed: {roots}");
        self.workspace.set_roots(roots.clone());
        self._watcher = watch(&self.workspace, self.user.as_deref(), cx)
            .inspect_err(|error| tracing::warn!("not watching for changes: {error}"))
            .ok();
        cx.set_menus(menus(self.workspace.components()));
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
        cx.notify();
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
        self.inspector.selected = picked;
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

    /// Shows `message` at the bottom for a moment.
    fn show_toast(&mut self, message: &'static str, cx: &mut Context<Self>) {
        let id = self.toast.as_ref().map_or(0, |(id, _)| id + 1);
        self.toast = Some((id, message.into()));
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
        cx: &mut Context<Self>,
    ) -> (AnyElement, bool) {
        let mut painter = Painter::new(
            &self.workspace,
            &mut self.motion,
            &mut self.terminals,
            &self.inputs,
            None,
        );
        let content = painter.window(name, cx);
        (content, painter.animating())
    }

    /// The browser window's content.
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
        )
        .previewing();
        let content = painter.gallery(&self.chosen, query, search, cx);
        (content, painter.animating())
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
    cx.on_action(move |_: &ShowThemes, cx| {
        let _ = app.update(cx, |app, cx| app.show_themes(cx));
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

/// Starts the providers, sharing what each publishes as `app.<name>`.
fn start_providers(workspace: &Workspace, cx: &mut Context<MoonframeApp>) -> Providers {
    let (sender, mut events) = futures::channel::mpsc::unbounded();
    let providers = Providers::start(
        workspace.roots().clone(),
        paths::builds(),
        Arc::new(move |event| {
            let _ = sender.unbounded_send(event);
        }),
    );
    cx.spawn(async move |this, cx| {
        while let Some(event) = events.next().await {
            let handled = this.update(cx, |this, cx| {
                match event {
                    Event::Published { provider, data } => {
                        this.workspace.set_host(&provider, data);
                    },
                    Event::Failed { provider, problem } => {
                        tracing::warn!("provider {provider}: {problem}");
                        let problem = Error::new(format!("provider failed: {problem}"));
                        this.workspace.clear_host(&provider, Some(problem));
                    },
                }
                cx.notify();
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

/// Watches every layer, and the user's list of packages.
fn watch(
    workspace: &Workspace,
    user: Option<&Path>,
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
    let list = user.map(|user| {
        user.canonicalize()
            .unwrap_or_else(|_| user.to_owned())
            .join(packages::FILE)
    });
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
        self.inputs.begin_frame();
        let last_keyed = self.motion.begin_frame();
        self.terminals.begin_frame();
        let mut painter = Painter::new(
            &self.workspace,
            &mut self.motion,
            &mut self.terminals,
            &self.inputs,
            self.inspector.sink(),
        )
        .with_browsers(&mut self.browsers)
        .with_keyed(&last_keyed);
        let body = painter.shell(cx);
        if painter.animating() {
            window.request_animation_frame();
        }
        let key_layers = painter.take_key_layers();
        self.key_layers = key_layers;
        // A terminal that just appeared (a new tab, or one selected) takes
        // the keyboard, once it is drawn.
        if let Some(focus) = self.terminals.take_appeared() {
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
            .key_context("MoonframeApp")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                this.run(Command::ToggleTheme, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Inspect, _, cx| this.toggle_inspect(cx)))
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
