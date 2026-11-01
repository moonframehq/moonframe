//! The tasks the view starts to hear what runs elsewhere: terminals,
//! the app's views, providers, and the app quitting.

use super::*;

/// A callback for terminal sessions, which run on threads of their own: each
/// call asks for one more frame, and a burst of calls makes one.
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
                    Event::Published { provider, data } => {
                        // No view read it: the window looks the same.
                        let drawn = this.workspace.set_host(&provider, data);
                        // Mods that heard it may have asked a provider.
                        this.forward(&provider, cx);
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
pub(super) fn report(workspace: &Workspace) {
    for (name, error) in workspace.problems() {
        tracing::warn!("{name}: {error}");
    }
}

/// Saves the state that changed since the last save, waiting for it, as
/// the app quits: the workspace saves as it goes, but writes are queued.
pub(super) fn flush_on_quit(cx: &mut Context<MoonframeApp>) {
    cx.on_app_quit(|app, _| {
        app.workspace.flush();
        async {}
    })
    .detach();
}
