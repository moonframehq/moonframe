//! How long the running turn and each background activity have run, as a
//! view shows them: views have no clock, so whoever prepares the view adds
//! them, at the time it shares it, and shares again as they grow.

use crate::{ChatView, text};

/// Fills in `working_for` and each activity's `running_for` at `now_ms`
/// (Unix ms).
pub fn clock(view: &mut ChatView, now_ms: u64) {
    view.working_for = view.since_ms.map(|since| elapsed(now_ms, since));
    for activity in &mut view.activities {
        activity.running_for = Some(elapsed(now_ms, activity.started_ms));
    }
}

/// Whether the view shows a clock that moves, and so wants sharing again
/// each second.
pub fn ticking(view: &ChatView) -> bool {
    view.since_ms.is_some() || !view.activities.is_empty()
}

/// `12s`, `3m 05s`: whole seconds, so a clock does not flicker in ms.
fn elapsed(now_ms: u64, since_ms: u64) -> String {
    let ms = now_ms.saturating_sub(since_ms);
    if ms < 1000 {
        return "0s".to_owned();
    }
    text::duration(ms)
}
