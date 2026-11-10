//! Registry changes arrive from arbitrary embedder threads, then reload on
//! the window's thread just like file changes.

use super::*;

pub(super) fn listen(entries: moonframe_core::entries::Entries, cx: &mut App) {
    let (send, mut changes) = futures::channel::mpsc::unbounded();
    let _ = send.unbounded_send(()); // Reconcile changes made during launch.
    entries.on_change(move || {
        let _ = send.unbounded_send(());
    });
    cx.spawn(async move |cx| {
        while changes.next().await.is_some() {
            while changes.try_recv().is_ok() {}
            cx.update(|cx| {
                for app in hub::every(cx) {
                    app.update(cx, |app, cx| {
                        app.reload_packages(cx);
                    });
                }
            });
        }
    })
    .detach();
}
