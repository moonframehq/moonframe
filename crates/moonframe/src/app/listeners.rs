//! The tasks the view starts to hear what runs elsewhere: terminals,
//! the app's views, providers, and the app quitting.

use super::*;

/// A callback for terminal sessions, which run on threads of their own: each
/// call asks for one more frame, and a burst of calls makes one.
#[cfg(feature = "terminal")]
pub(super) fn wake_on_output(cx: &mut Context<MoonframeApp>) -> Arc<dyn Fn() + Send + Sync> {
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

/// Where the app's own views send messages: each goes to the `update` of
/// the mod that drew the view.
pub(super) fn deliver_from_views(
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
pub(super) fn start_providers(
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
                    Event::Published { provider, data } => this.published(&provider, data, cx),
                    // The newest there is by now, not what was first sent.
                    Event::Newest { provider, newest } => {
                        if let Some(data) = newest.take() {
                            this.published(&provider, data, cx);
                        }
                    },
                    Event::Link { provider, link } => {
                        this.links.insert(provider, link.data());
                        let links = Data::Map(this.links.clone());
                        this.published("links", links, cx);
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
pub(super) fn report(workspace: &Workspace) {
    for (name, error) in workspace.problems() {
        tracing::warn!("{name}: {error}");
    }
}

/// Saves the state that changed since the last save, waiting for it, and
/// lets processes leave, as the app quits: the workspace saves as it goes,
/// but writes are queued. This runs before GPUI's timeout on quitting
/// starts, which is too short for a process given a `stop-grace`.
pub(super) fn flush_on_quit(cx: &mut Context<MoonframeApp>) {
    cx.on_app_quit(|app, _| {
        app.workspace.flush();
        app.providers.shutdown();
        async {}
    })
    .detach();
}

impl MoonframeApp {
    /// Shares `data` as `app.<key>`, drawing again if a view read it.
    pub(super) fn published(&mut self, key: &str, data: Data, cx: &mut Context<Self>) {
        // No view read it: the window looks the same.
        let drawn = self.workspace.set_host(key, data);
        // Mods that heard it may have asked a provider.
        self.forward(key, cx);
        if drawn > 0 {
            // A view may fail on the new data: say so.
            report(&self.workspace);
            cx.notify();
        }
    }

    /// Shares the screens as `app.displays`, now and as they change.
    pub(super) fn watch_displays(cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        let share = move |cx: &mut App| {
            let displays = displays(cx);
            let _ = app.update(cx, |app, cx| app.published("displays", displays, cx));
        };
        let again = share.clone();
        cx.observe_displays(move |_, cx| again(cx)).detach();
        cx.defer(share);
    }
}

/// A screen's `id` as views read it.
pub(crate) fn display_number(id: DisplayId) -> i64 {
    i64::try_from(u64::from(id)).unwrap_or(i64::MAX)
}

/// Every screen, the primary one first, as views read them.
fn displays(cx: &App) -> Data {
    let primary = cx.primary_display().map(|display| display.id());
    let mut displays = cx.displays();
    displays.sort_by_key(|display| Some(display.id()) != primary);
    Data::List(
        displays
            .iter()
            .map(|display| {
                let bounds = display.bounds();
                let number = |value: Pixels| Data::Number(f64::from(f32::from(value)));
                Data::Map(std::collections::BTreeMap::from([
                    ("id".to_owned(), Data::Integer(display_number(display.id()))),
                    (
                        "primary".to_owned(),
                        Data::Bool(Some(display.id()) == primary),
                    ),
                    ("x".to_owned(), number(bounds.origin.x)),
                    ("y".to_owned(), number(bounds.origin.y)),
                    ("width".to_owned(), number(bounds.size.width)),
                    ("height".to_owned(), number(bounds.size.height)),
                ]))
            })
            .collect(),
    )
}
