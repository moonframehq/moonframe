//! Following the Omarchy desktop's theme as it changes.

use {super::*, crate::themes::omarchy};

/// Watches for Omarchy changing its theme, on Omarchy only. Its own watcher,
/// because `watch` stays off in a release bundle and this must not.
pub(super) fn watching(cx: &mut Context<MoonframeApp>) -> Option<RecommendedWatcher> {
    let (sender, mut changes) = futures::channel::mpsc::unbounded();
    let watcher = omarchy::watch(move || {
        let _ = sender.unbounded_send(());
    })
    .inspect_err(|error| tracing::warn!("not following the omarchy theme: {error}"))
    .ok()
    .flatten()?;
    cx.spawn(async move |this, cx| {
        while changes.next().await.is_some() {
            cx.background_executor().timer(SETTLE).await;
            while changes.try_recv().is_ok() {}
            if this
                .update(cx, |this, cx| this.omarchy_changed(cx))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
    Some(watcher)
}

impl MoonframeApp {
    /// Shows Omarchy's new theme wherever its theme is the one picked. Other
    /// picks stay, and nothing is saved: the name is what is kept.
    fn omarchy_changed(&mut self, cx: &mut Context<Self>) {
        if !omarchy::load() {
            return;
        }
        for slot in [&mut self.light_scheme, &mut self.dark_scheme] {
            if slot.is_some_and(|scheme| scheme.name == omarchy::NAME) {
                *slot = omarchy::current();
            }
        }
        self.preview = None;
        self.apply_theme(cx);
        cx.notify();
    }
}
