//! What mods ask of notifications: posting one (`app:notify`), as the
//! system's or, where there is none, as a toast; and asking the user to
//! allow them (`{ ask = true }`), whose answer `app.notifications` shares.

use {super::*, crate::alerts::Posted, moonframe_core::Notification};

impl MoonframeApp {
    /// Posts `notification` for the mod `from`.
    pub(super) fn notify_request(
        &mut self,
        from: &str,
        notification: Notification,
        cx: &mut Context<Self>,
    ) {
        match self.alerts.notify(from, &notification) {
            Ok(Posted::System) => {},
            Ok(Posted::Toast) => {
                let text = if notification.body.is_empty() {
                    notification.title.clone()
                } else {
                    format!("{}: {}", notification.title, notification.body)
                };
                // The toast shows one at a time: a new one, replacing or
                // not, takes the place of the last.
                self.show_toast_for(text.into(), NOTICE, cx);
                // A click on it does what one on the system's would.
                if let Some((shown, _)) = &self.toast {
                    self.toast_notice = Some((*shown, from.to_owned(), notification.id));
                }
            },
            Err(problem) => tracing::warn!("{from}: {problem}"),
        }
    }
}
