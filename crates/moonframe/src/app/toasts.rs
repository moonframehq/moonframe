//! The short notes shown at the bottom of the window.

use super::*;

impl MoonframeApp {
    /// Shows `message` at the bottom for a moment.
    pub(super) fn show_toast(&mut self, message: &'static str, cx: &mut Context<Self>) {
        self.show_toast_text(message.into(), cx);
    }

    /// A new toast's id, replacing the one showing: a notice it carried
    /// goes with it.
    pub(super) fn next_toast(&mut self) -> u64 {
        self.toasts += 1;
        self.toast_notice = None;
        self.toasts
    }

    pub(super) fn show_toast_text(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.show_toast_for(message, TOAST, cx);
    }

    pub(super) fn show_toast_for(
        &mut self,
        message: SharedString,
        shown: Duration,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_toast();
        self.toast = Some((id, message));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(shown).await;
            let _ = this.update(cx, |this, cx| {
                if this.toast.as_ref().is_some_and(|(shown, _)| *shown == id) {
                    this.toast = None;
                    this.toast_notice = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

/// A pill at the bottom center, over everything; a `notice` (a
/// notification shown here) is clickable, as the system's would be.
pub(super) fn toast(message: SharedString, notice: bool, cx: &Context<MoonframeApp>) -> AnyElement {
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
                    .id("toast")
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .bg(theme.foreground)
                    .text_color(theme.background)
                    .text_sm()
                    .shadow_md()
                    .child(message)
                    .child(crate::browser::cover())
                    .when(notice, |pill| {
                        pill.cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| app.click_toast(cx)))
                    }),
            ),
    )
    .with_priority(10)
    .into_any_element()
}
