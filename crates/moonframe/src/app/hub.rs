//! What an app's main windows share. Each main window is a
//! [`MoonframeApp`] of its own, with its own workspace of Luau modules;
//! the hub, made once as the app starts, holds what there is one of:
//!
//! - the providers, whose data every window shows, and whose replies go
//!   back to the window that asked (the message's `origin`);
//! - the last of everything shared as `app.<key>`, for a window that opens
//!   later;
//! - the history of the app's folders, and what runs once for the app:
//!   the menu actions, notification clicks, `moonframe pane new`, update
//!   checks.
//!
//! What app-wide comes without a window (a menu item, a notification
//! click) goes to the front main window: the active one, else the one
//! active last.
//!
//! Only the first window keeps its mods' state across launches. Another
//! starts from the mods' own, saves nothing, and its terminals are local
//! shells that end with it, as two windows keeping the same state would
//! overwrite each other's.

use {
    super::*,
    crate::{
        backends::Backends,
        history::Kept,
        hosted::Registry,
        paths::{self, Folders},
    },
    gpui_kit::component::Root,
    moonframe_providers::Event,
    std::collections::BTreeMap,
};

/// How far another window opens from the front one.
const CASCADE: Pixels = px(28.);

/// What opening another main window takes, from the first.
#[derive(Clone)]
pub(crate) struct Launch {
    pub(crate) folders: Folders,
    pub(crate) backends: Arc<Backends>,
    pub(crate) views: Registry,
    pub(crate) chrome: Chrome,
}

pub(crate) struct Hub {
    /// The app's providers, which publish data as `app.<name>`.
    providers: Providers,
    /// How each process or socket provider's link stands: `app.links`.
    links: BTreeMap<String, Data>,
    /// The last of each `app.<key>` shared app-wide.
    latest: BTreeMap<String, Data>,
    /// The main windows, the one active last first.
    windows: Vec<Member>,
    /// The number the next window joining takes.
    next: u64,
    launch: Launch,
    history: Option<Arc<Kept>>,
}

/// A main window, as the hub knows it.
#[derive(Clone)]
struct Member {
    id: u64,
    window: WindowId,
    app: WeakEntity<MoonframeApp>,
}

struct Installed(Entity<Hub>);

impl Global for Installed {}

/// What a window joining the hub is given.
pub(super) struct Joined {
    pub(super) hub: Entity<Hub>,
    /// Its number: where provider replies for it go.
    pub(super) id: u64,
    /// The first window, which keeps state and terminals across launches.
    pub(super) first: bool,
    /// What was shared app-wide before it opened.
    pub(super) latest: BTreeMap<String, Data>,
    pub(super) history: Option<Arc<Kept>>,
    /// How the windows open already look.
    pub(super) look: Option<Look>,
}

/// What a provider event does in one window.
enum Delivery {
    Share(String, Data),
    Failed(String, Error),
    Send { from: String, to: String, msg: Data },
}

impl Hub {
    /// Makes the app's hub: starts the providers `workspace` wants first
    /// (with the native ones the app has) and what runs once for the app.
    pub(crate) fn install(
        workspace: &Workspace,
        natives: Natives,
        launch: Launch,
        cx: &mut App,
    ) -> Entity<Self> {
        let hub = cx.new(|cx| {
            let (sender, mut events) = futures::channel::mpsc::unbounded();
            // Only those the first views read or the mods hear, and those
            // that always start: the rest wait until something wants them.
            let providers = Providers::start_wanting(
                workspace.roots().clone(),
                paths::builds(),
                Arc::new(move |event| {
                    let _ = sender.unbounded_send(event);
                }),
                natives,
                workspace.wanted_providers(),
            );
            cx.spawn(async move |hub, cx| {
                while let Some(event) = events.next().await {
                    let Ok(deliveries) = hub.update(cx, |hub: &mut Self, _| hub.route(event))
                    else {
                        break;
                    };
                    cx.update(|cx| deliver(deliveries, cx));
                }
            })
            .detach();
            // Processes are told to leave as the app quits, and waited for.
            cx.on_app_quit(|hub, _| {
                hub.providers.shutdown();
                async {}
            })
            .detach();
            let history = crate::history::open(&launch.folders);
            // An agent's run is marked before it starts, to come back to.
            #[cfg(feature = "acp")]
            if let Some(history) = &history {
                moonframe_acp::set_keeper(history.clone());
            }
            // What changed while the app was closed is a snapshot of its own.
            if let Some(history) = &history {
                capture(history.clone(), "launch", cx);
            }
            Self {
                providers,
                links: BTreeMap::new(),
                latest: last_crash().into_iter().collect(),
                windows: Vec::new(),
                next: 0,
                launch,
                history,
            }
        });
        cx.set_global(Installed(hub.clone()));
        cx.on_window_closed(|cx, id| {
            if let Some(hub) = hub_of(cx) {
                hub.update(cx, |hub, _| {
                    hub.windows.retain(|member| member.window != id)
                });
            }
        })
        .detach();
        let folders = hub.read(cx).launch.folders.clone();
        // Projects are kept only with the user's folder: `--no-user` runs
        // start fresh and keep nothing, as their state.
        crate::project::restore(
            (folders.user.as_ref())
                .map(|_| crate::project::kept_file(folders.profile.as_deref()))
                .as_deref(),
        );
        menu_actions(cx);
        hear_notification_clicks(cx);
        take_panes(&folders, cx);
        #[cfg(feature = "updates")]
        check_for_updates_daily(&folders, cx);
        hub
    }

    /// Adds the window `window`, drawn by `app`.
    pub(super) fn join(
        hub: &Entity<Self>,
        window: WindowId,
        app: WeakEntity<MoonframeApp>,
        cx: &mut App,
    ) -> Joined {
        let look = front(cx).map(|front| front.read(cx).look);
        hub.update(cx, |this, _| {
            let id = this.next;
            this.next += 1;
            this.windows.push(Member { id, window, app });
            Joined {
                hub: hub.clone(),
                id,
                first: id == 0,
                latest: this.latest.clone(),
                history: this.history.clone(),
                look,
            }
        })
    }

    /// The window `id` became active: it is the front one now.
    pub(super) fn raise(&mut self, id: u64) {
        if let Some(at) = self.windows.iter().position(|member| member.id == id) {
            let member = self.windows.remove(at);
            self.windows.insert(0, member);
        }
    }

    /// Starts the providers `wanted` names, and hands each message the
    /// window `origin` sent on to its provider.
    pub(super) fn send(
        &mut self,
        origin: u64,
        wanted: Option<std::collections::BTreeSet<String>>,
        outbox: Vec<(String, String, Data)>,
        from: &str,
    ) {
        self.providers.want(wanted);
        for (provider, sender, message) in outbox {
            if let Err(problem) = self.providers.send_for(&provider, &sender, origin, message) {
                tracing::warn!("{from}: {problem}");
            }
        }
    }

    /// Restarts the providers whose folders changed (from `roots`, if
    /// given); the names of those that are gone.
    pub(super) fn reload_providers(&mut self, roots: Option<Roots>) -> Vec<String> {
        let before: Vec<String> = self.providers.names().map(str::to_owned).collect();
        match roots {
            Some(roots) => self.providers.set_roots(roots),
            None => self.providers.reload(),
        }
        let gone: Vec<String> = before
            .into_iter()
            .filter(|name| !self.providers.names().any(|now| now == name.as_str()))
            .collect();
        for name in &gone {
            self.latest.remove(name);
        }
        gone
    }

    /// Starts the provider `name` again (`app:restart`, or Retry).
    pub(super) fn restart(&mut self, name: &str) -> Result<(), Error> {
        self.latest.remove(name);
        self.providers.restart(name)
    }

    /// Tells the providers that follow it the project moved.
    #[cfg(feature = "terminal")]
    pub(crate) fn tell_project(&self) {
        self.providers.tell_project();
    }

    /// What `event` does in each window it is for.
    fn route(&mut self, event: Event) -> Vec<(WeakEntity<MoonframeApp>, Delivery)> {
        match event {
            Event::Published { provider, data } => self.share(provider, data),
            // The newest there is by now, not what was first sent.
            Event::Newest { provider, newest } => newest
                .take()
                .map(|data| self.share(provider, data))
                .unwrap_or_default(),
            Event::Link { provider, link } => {
                self.links.insert(provider, link.data());
                self.share("links".to_owned(), Data::Map(self.links.clone()))
            },
            Event::Failed { provider, problem } => {
                tracing::warn!("provider {provider}: {problem}");
                self.latest.remove(&provider);
                let problem = Error::new(format!("provider failed: {problem}"));
                self.to_each(|| Delivery::Failed(provider.clone(), problem.clone()))
            },
            // A native provider's answer goes to the mod that asked, in the
            // window it asked from; one it sends by itself, to every window.
            Event::Sent {
                provider,
                to,
                msg,
                origin,
            } => {
                let from = format!("{}{provider}", moonframe_core::PROVIDER);
                let send = || Delivery::Send {
                    from: from.clone(),
                    to: to.clone(),
                    msg: msg.clone(),
                };
                let asker = origin.and_then(|origin| {
                    self.windows
                        .iter()
                        .find(|member| member.id == origin && member.app.upgrade().is_some())
                });
                match asker {
                    Some(member) => vec![(member.app.clone(), send())],
                    None => self.to_each(send),
                }
            },
        }
    }

    /// Shares `data` as `app.<key>` with every window, and with those that
    /// open later.
    fn share(&mut self, key: String, data: Data) -> Vec<(WeakEntity<MoonframeApp>, Delivery)> {
        self.latest.insert(key.clone(), data.clone());
        self.to_each(|| Delivery::Share(key.clone(), data.clone()))
    }

    fn to_each(
        &self,
        delivery: impl Fn() -> Delivery,
    ) -> Vec<(WeakEntity<MoonframeApp>, Delivery)> {
        self.windows
            .iter()
            .map(|member| (member.app.clone(), delivery()))
            .collect()
    }
}

/// Does what provider events do in each window, once the hub is free:
/// a window hearing one sends providers its mods' messages through it.
fn deliver(deliveries: Vec<(WeakEntity<MoonframeApp>, Delivery)>, cx: &mut App) {
    for (app, delivery) in deliveries {
        let _ = app.update(cx, |app, cx| match delivery {
            Delivery::Share(key, data) => app.published(&key, data, cx),
            Delivery::Failed(provider, problem) => {
                app.workspace.clear_host(&provider, Some(problem));
                cx.notify();
            },
            Delivery::Send { from, to, msg } => {
                app.workspace.send(&from, &to, msg);
                app.flush(&from, cx);
            },
        });
    }
}

pub(super) fn hub_of(cx: &App) -> Option<Entity<Hub>> {
    cx.try_global::<Installed>()
        .map(|installed| installed.0.clone())
}

/// Every main window's app, the front one first.
pub(crate) fn every(cx: &App) -> Vec<Entity<MoonframeApp>> {
    let Some(hub) = hub_of(cx) else {
        return Vec::new();
    };
    let active = cx.active_window().map(|window| window.window_id());
    let windows = &hub.read(cx).windows;
    let mut apps: Vec<(bool, Entity<MoonframeApp>)> = windows
        .iter()
        .filter_map(|member| Some((Some(member.window) != active, member.app.upgrade()?)))
        .collect();
    // Stable: the others stay in the order they were last active.
    apps.sort_by_key(|(behind, _)| *behind);
    apps.into_iter().map(|(_, app)| app).collect()
}

/// The front main window's app: the active one, else the one active last.
pub(crate) fn front(cx: &App) -> Option<Entity<MoonframeApp>> {
    every(cx).into_iter().next()
}

/// Runs `f` on the front main window's app, if there is one.
fn on_front(cx: &mut App, f: impl FnOnce(&mut MoonframeApp, &mut Context<MoonframeApp>)) {
    if let Some(app) = front(cx) {
        app.update(cx, f);
    }
}

/// Runs `f` on every main window's app.
fn on_every(cx: &mut App, f: impl Fn(&mut MoonframeApp, &mut Context<MoonframeApp>)) {
    for app in every(cx) {
        app.update(cx, &f);
    }
}

/// Opens another main window of the app, with its own copy of the
/// layout, sharing the app's providers: what File > New Window does. It
/// opens a little below and right of the front main window, and keeps no
/// state across launches (only the first window does). Does nothing
/// before the app has started.
pub fn open_main_window(cx: &mut App) {
    let Some(hub) = hub_of(cx) else {
        tracing::warn!("no window opened: the app has not started");
        return;
    };
    let launch = hub.read(cx).launch.clone();
    let workspace = crate::capabilities::open(launch.folders.roots(), cx.theme().is_dark(), None);
    let bounds = cascaded(launch.chrome, cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(640.), px(400.))),
        app_id: Some(crate::app_id()),
        ..crate::cli::framed(launch.chrome)
    };
    let opened = cx.open_window(options, |window, cx| {
        let app = cx.new(|cx| {
            MoonframeApp::new(
                workspace,
                launch.chrome,
                launch.folders,
                launch.backends,
                launch.views,
                window,
                cx,
            )
        });
        // It cascades rather than reopening where it was: only the first
        // window's place is kept.
        let view = cx.new(|cx| MainWindow::new(app, None, window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    });
    match opened {
        Ok(_) => cx.activate(true),
        Err(error) => tracing::warn!("cannot open another window: {error}"),
    }
}

/// Where another window opens: below and right of the front main window,
/// back at its screen's corner when that would leave the screen; at the
/// shell's size, centered, when there is none.
fn cascaded(chrome: Chrome, cx: &mut App) -> Bounds<Pixels> {
    let front = front(cx).map(|app| app.read(cx).main_window);
    let placed = front
        .and_then(|id| {
            cx.windows()
                .into_iter()
                .find(|window| window.window_id() == id)
        })
        .and_then(|handle| {
            handle
                .update(cx, |_, window, cx| {
                    (
                        window.bounds(),
                        window.display(cx).map(|display| display.bounds()),
                    )
                })
                .ok()
        });
    let Some((bounds, screen)) = placed else {
        let (width, height) = chrome.size.unwrap_or((1280., 820.));
        return Bounds::centered(None, size(px(width), px(height)), cx);
    };
    let mut origin = bounds.origin + point(CASCADE, CASCADE);
    if let Some(screen) = screen
        && (origin.x + bounds.size.width > screen.right()
            || origin.y + bounds.size.height > screen.bottom())
    {
        origin = screen.origin + point(CASCADE, CASCADE);
    }
    Bounds::new(origin, bounds.size)
}

/// Takes a snapshot of each folder kept, saying `reason`, off the UI
/// thread; at launch, tidies the history too.
pub(super) fn capture(history: Arc<Kept>, reason: &'static str, cx: &App) {
    cx.background_executor()
        .spawn(async move {
            if let Err(error) = history.capture(reason) {
                tracing::warn!("no snapshot: {error}");
            }
            if reason == "launch"
                && let Err(error) = history.tidy()
            {
                tracing::warn!("cannot tidy the history: {error}");
            }
        })
        .detach();
}

/// The last launch's crash, shared as `app.crash`, if it left a report.
fn last_crash() -> Option<(String, Data)> {
    if !cfg!(feature = "crash") {
        return None;
    }
    let crash = crate::crash::last(&crate::crash_dir())?;
    tracing::warn!("the last run crashed: {crash:?}");
    Some(("crash".to_owned(), crash))
}

/// The menu actions that need no particular window, handled app-wide, so
/// they work from the menu bar or their shortcut whichever window has
/// focus, or none (the app in the background, or every window minimized):
/// in the front main window, or in every one for what changes them all.
fn menu_actions(cx: &mut App) {
    cx.on_action(|_: &NewWindow, cx| open_main_window(cx));
    cx.on_action(|_: &Reload, cx| on_every(cx, MoonframeApp::reload));
    cx.on_action(|_: &BrowseComponents, cx| {
        on_front(cx, |app, cx| app.show_component("", cx));
    });
    cx.on_action(|action: &ShowComponent, cx| {
        on_front(cx, |app, cx| app.show_component(&action.name, cx));
    });
    cx.on_action(|_: &ShowThemes, cx| on_front(cx, MoonframeApp::show_themes));
    cx.on_action(|action: &SetAppearance, cx| {
        on_front(cx, |app, cx| app.set_appearance(action.appearance, cx));
    });
    cx.on_action(|_: &AboutApp, cx| on_front(cx, MoonframeApp::show_about));
    cx.on_action(|_: &BuildApp, cx| on_front(cx, MoonframeApp::build_app));
    cx.on_action(|_: &CheckForUpdates, cx| {
        on_front(cx, |app, cx| app.check_for_updates(true, cx));
    });
    cx.on_action(|_: &ToggleBetaReleases, cx| {
        on_front(cx, MoonframeApp::toggle_beta_releases);
    });
    cx.on_action(|_: &UndoChange, cx| on_front(cx, |app, cx| app.step_history(true, cx)));
    cx.on_action(|_: &RedoChange, cx| on_front(cx, |app, cx| app.step_history(false, cx)));
    cx.on_action(|_: &ToggleDevtools, cx| on_front(cx, MoonframeApp::toggle_devtools));
    cx.on_action(|_: &ToggleDeveloperMode, cx| {
        let developer = crate::release();
        crate::set_developer(developer);
        on_every(cx, |app, cx| {
            if !developer && app.inspector.picking {
                app.toggle_inspect(cx);
            }
            if !developer {
                app.close_devtools(cx);
            }
            app.rewatch(cx);
            crate::host_menus::install(app.workspace.menus(), cx);
            cx.notify();
        });
        on_front(cx, |app, cx| {
            app.show_toast(
                if developer {
                    "Developer Mode: Components… (⇧⌘K), Inspect Element (⌥⌘I), Developer Tools (⌥⌘J) and Reload Modules (⌘R)"
                } else {
                    "Developer Mode is off"
                },
                cx,
            );
        });
        cx.refresh_windows();
    });
    cx.on_action(|_: &ToggleSlowAnimations, cx| {
        crate::motion::set_slow(!crate::motion::is_slow());
        // The menu shows the switch checked while it is on.
        crate::host_menus::refresh_menus(cx);
        cx.refresh_windows();
    });
}

/// Hears the clicks on the notifications mods posted: each brings the
/// app and its front window forward and tells the mod there that posted
/// it, on `app/notification`: `{ id, clicked = true }`.
fn hear_notification_clicks(cx: &mut App) {
    let (sender, mut clicks) = futures::channel::mpsc::unbounded::<(String, Option<String>)>();
    crate::alerts::listen(move |from, id| {
        let _ = sender.unbounded_send((from, id));
    });
    cx.spawn(async move |cx| {
        while let Some((from, id)) = clicks.next().await {
            cx.update(|cx| on_front(cx, |app, cx| app.notification_clicked(&from, id, cx)));
        }
    })
    .detach();
}

/// Opens the tabs `moonframe pane new` asks for, also those left before
/// the app opened, in the front main window. Runs without the user's
/// folder (checks, screenshots) take none.
fn take_panes(folders: &Folders, cx: &mut App) {
    let Some(user) = &folders.user else {
        return;
    };
    let inbox = crate::panes::inbox(user, folders.profile.as_deref());
    cx.spawn(async move |cx| {
        loop {
            cx.update(|cx| {
                // Left waiting while no window is open to take them.
                let Some(app) = front(cx) else {
                    return;
                };
                let requests = crate::panes::take(&inbox);
                app.update(cx, |app, cx| {
                    for request in requests {
                        app.open_pane(request, cx);
                    }
                });
            });
            cx.background_executor().timer(crate::panes::POLL).await;
        }
    })
    .detach();
}

/// An installed app looks for updates by itself, once a day, also while
/// it stays open, whatever Developer Mode says; runs from source and runs
/// without the user's folder (checks, screenshots) never do.
#[cfg(feature = "updates")]
fn check_for_updates_daily(folders: &Folders, cx: &mut App) {
    if !(crate::update::checks_by_itself()
        && crate::update::source().is_some()
        && folders.user.is_some())
    {
        return;
    }
    let profile = folders.profile.clone();
    cx.spawn(async move |cx| {
        loop {
            if crate::update::due(profile.as_deref()) {
                cx.update(|cx| on_front(cx, |app, cx| app.check_for_updates(false, cx)));
            }
            cx.background_executor().timer(crate::update::POLL).await;
        }
    })
    .detach();
}
