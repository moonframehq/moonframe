//! The short notes shown at the bottom of the window.

use super::*;

impl MoonframeApp {
    /// Shows `message` at the bottom for a moment.
    pub(super) fn show_toast(&mut self, message: &'static str, cx: &mut Context<Self>) {
        self.show_toast_text(message.into(), cx);
    }

    pub(super) fn show_toast_text(&mut self, message: SharedString, cx: &mut Context<Self>) {
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
}

/// A pill at the bottom center, over everything.
pub(super) fn toast(message: SharedString, cx: &Context<MoonframeApp>) -> AnyElement {
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
