//! Without `ui.video` (the `video` feature, or off macOS): nothing to do,
//! and [`crate::render`] says so where one is drawn.

use {
    futures::channel::mpsc::UnboundedSender,
    gpui_kit::{IntoElement, WindowId, div},
    moonframe_core::Data,
};

pub(crate) fn deliver_to(_: UnboundedSender<(String, Data)>) {}

pub(crate) fn sweep() -> impl IntoElement {
    div()
}

pub(crate) fn window_closed(_: WindowId) {}
