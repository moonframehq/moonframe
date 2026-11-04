//! What mods ask of the machine rather than the window: staying awake
//! (`app:awake`), and starting a provider again (`app:restart`). Also tells
//! them of the last launch's crash, as `app.crash`.

use {super::*, moonframe_core::Awake as AwakeRequest};

impl MoonframeApp {
    /// Does what the mod `from` asked with `app:awake`, and says how it
    /// stands in `app.awake` once the system has answered.
    pub(super) fn awake_request(
        &mut self,
        from: &str,
        request: AwakeRequest,
        cx: &mut Context<Self>,
    ) {
        let waiting = self.awake.change(from, request);
        self.published("awake", self.awake.data(), cx);
        let Some(answer) = waiting else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    answer
                        .recv()
                        .unwrap_or_else(|_| Err("the keep-awake thread stopped".to_owned()))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.awake.settled(result);
                app.published("awake", app.awake.data(), cx);
            });
        })
        .detach();
    }

    /// Starts the provider `name` again, for the mod `from`.
    pub(super) fn restart_provider(&mut self, from: &str, name: &str) {
        if let Err(problem) = self.providers.restart(name) {
            self.workspace
                .refuse(from, Error::new(format!("`app:restart`: {problem}")));
        }
    }

    /// Shares the last launch's crash, if it left a report.
    pub(super) fn tell_last_crash(cx: &mut Context<Self>) {
        let Some(crash) = crate::crash::last(&crate::crash_dir()) else {
            return;
        };
        tracing::warn!("the last run crashed: {crash:?}");
        // After the app exists: this runs as it is being made.
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |app, cx| app.published("crash", crash, cx));
        })
        .detach();
    }
}
