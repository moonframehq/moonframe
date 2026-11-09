//! What mods ask of the machine rather than the window: staying awake
//! (`app:awake`), and starting a provider again (`app:restart`). Also tells
//! them of the last launch's crash, as `app.crash`.

use {
    super::*,
    moonframe_core::{Awake as AwakeRequest, Dock},
};

impl MoonframeApp {
    /// Whether the app was `built` with `feature`; if not, the mod `from`,
    /// which asked for something that needs it, hears so.
    pub(super) fn built_with(&mut self, from: &str, built: bool, feature: &str) -> bool {
        if !built {
            self.workspace.refuse(
                from,
                Error::new(format!(
                    "this app was built without the `{feature}` feature"
                )),
            );
        }
        built
    }

    /// Does what the mod `from` asked with `app:dock`.
    pub(super) fn dock_request(&mut self, from: &str, dock: &Dock) {
        if self.built_with(from, cfg!(feature = "dock"), "dock") {
            crate::dock::show(dock);
        }
    }

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
                .spawn(async move { answer.wait() })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.awake.settled(result);
                app.published("awake", app.awake.data(), cx);
            });
        })
        .detach();
    }

    /// Starts the provider `name` again, for the mod `from`.
    pub(super) fn restart_provider(&mut self, from: &str, name: &str, cx: &mut Context<Self>) {
        if !self.built_with(from, cfg!(feature = "restart"), "restart") {
            return;
        }
        if let Err(problem) = self.hub.update(cx, |hub, _| hub.restart(name)) {
            self.workspace
                .refuse(from, Error::new(format!("`app:restart`: {problem}")));
        }
    }
}
