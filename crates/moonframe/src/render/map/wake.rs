//! Tile arrivals and pulse timers are shared, but every map that asks must
//! be woken: a live view in the main window, or an app drawing other windows.

use {
    super::{super::Cx, tiles},
    gpui_kit::EntityId,
    std::{cell::RefCell, collections::HashSet, time::Duration},
};

/// A slow breath needs no more than thirty frames a second on a 60 Hz screen.
const PULSE_FRAME: Duration = Duration::from_millis(20);

thread_local! {
    static WAITING: RefCell<Pending> = RefCell::default();
    static PULSING: RefCell<Pending> = RefCell::default();
}

#[derive(Default)]
struct Pending {
    targets: HashSet<EntityId>,
}

impl Pending {
    /// Remembers a target; only the first starts the shared task.
    fn request(&mut self, target: EntityId) -> bool {
        let start = self.targets.is_empty();
        self.targets.insert(target);
        start
    }

    /// Clears the pending task before notifying all its targets.
    fn take(&mut self) -> HashSet<EntityId> {
        std::mem::take(&mut self.targets)
    }
}

pub(super) fn arrival(target: EntityId, cx: Cx) {
    if !WAITING.with_borrow_mut(|pending| pending.request(target)) {
        return;
    }
    let arrived = tiles::arrival();
    cx.spawn(async move |_, cx| {
        let _ = arrived.await;
        let targets = WAITING.with_borrow_mut(Pending::take);
        // The app that started the task may be gone; the other targets still
        // need their wakeup, so do not update through that app's weak handle.
        cx.update(|cx| {
            for target in targets {
                cx.notify(target);
            }
        });
    })
    .detach();
}

pub(super) fn pulse(target: EntityId, cx: Cx) {
    if !PULSING.with_borrow_mut(|pending| pending.request(target)) {
        return;
    }
    cx.spawn(async move |_, cx| {
        cx.background_executor().timer(PULSE_FRAME).await;
        let targets = PULSING.with_borrow_mut(Pending::take);
        cx.update(|cx| {
            for target in targets {
                cx.notify(target);
            }
        });
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use {super::Pending, gpui_kit::EntityId, std::collections::HashSet};

    #[test]
    fn a_shared_wakeup_reaches_live_views_and_other_windows() {
        let live = EntityId::from(1);
        let app = EntityId::from(2);
        let other_live = EntityId::from(3);
        let mut pending = Pending::default();
        assert!(pending.request(live));
        assert!(!pending.request(app));
        assert!(!pending.request(other_live));
        assert!(!pending.request(live));
        assert_eq!(pending.take(), HashSet::from([live, app, other_live]));

        // The first view can disappear; the remaining window starts and
        // receives the next wakeup independently.
        assert!(pending.request(app));
        assert_eq!(pending.take(), HashSet::from([app]));
        assert!(pending.take().is_empty());
    }
}
