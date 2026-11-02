//! The tabs `moonframe pane new` asks for (see `crate::panes`).

use super::*;

impl MoonframeApp {
    /// Opens the tabs `moonframe pane new` asks for, also those left for
    /// this window before it opened. Runs without the user's folder
    /// (checks, screenshots) take none.
    pub(super) fn take_panes(&self, cx: &mut Context<Self>) {
        let Some(user) = &self.folders.user else {
            return;
        };
        let inbox = crate::panes::inbox(user, self.folders.profile.as_deref());
        cx.spawn(async move |this, cx| {
            loop {
                let requests = crate::panes::take(&inbox);
                let open = this.update(cx, |app, cx| {
                    for request in requests {
                        app.open_pane(request, cx);
                    }
                });
                if open.is_err() {
                    break;
                }
                cx.background_executor().timer(crate::panes::POLL).await;
            }
        })
        .detach();
    }

    /// Opens the tab `request` asks for in the main tab bar: a terminal in
    /// its folder (the window's own user folder by default), with the
    /// agent's command typed into it.
    fn open_pane(&mut self, request: crate::panes::Request, cx: &mut Context<Self>) {
        const OPEN: &str = "tabs/main/open";
        if !self.workspace.hears(OPEN) {
            tracing::warn!(
                "pane new: this app's shell has no main tab bar (`tabbed {{ id = \"main\" }}`)"
            );
            self.show_toast("This app has no main tab bar to open a pane in", cx);
            return;
        }
        // Unique: above the last one, and above any number the editor
        // gave a request (its own count) or an earlier run used.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| {
                i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
            });
        let n = now.max(self.last_pane + 1);
        self.last_pane = n;
        let id = format!("pane-{n}");
        let cwd = request.cwd.clone().or_else(|| self.folders.own());
        let shell = crate::panes::Shell::current();
        if let Some(bytes) = request.typed(shell) {
            self.terminals.type_on_start(&id, cwd.clone(), bytes);
        }
        let mut props = Vec::new();
        if let Some(cwd) = &cwd {
            props.push(("cwd", Data::from(cwd.to_string_lossy().into_owned())));
        }
        // The tab's id names its terminal (`<mod>/<tab id>`), so the
        // command goes to that one.
        let mut asked = vec![
            ("n", Data::from(n)),
            ("id", Data::from(id.as_str())),
            ("kind", Data::from("component:terminal")),
            ("props", Data::map(props)),
        ];
        if let Some(title) = request.title() {
            asked.push(("title", Data::from(title)));
        }
        let from = SHELL;
        // The main bar publishes `tabs/main` at every change, so one that
        // opened the tab changed it; any other bar ignores the request.
        let before = self.workspace.topic("tabs/main").cloned();
        self.workspace.send(
            from,
            &format!("{}{OPEN}", moonframe_core::TOPIC),
            Data::map(asked),
        );
        self.flush(from, cx);
        if self.workspace.topic("tabs/main") == before.as_ref() {
            tracing::warn!("pane new: no tab bar `main` took the request");
            self.show_toast("This app has no main tab bar to open a pane in", cx);
            return;
        }
        if request.prompt_lost(shell) {
            tracing::warn!(
                "pane new: cannot quote a prompt for this shell; started the agent without it"
            );
            self.show_toast(
                "Your shell's quoting is unknown: the agent started without the prompt",
                cx,
            );
        }
        // Brought forward, to watch the agent; not when opened behind others.
        if !crate::background() {
            cx.activate(true);
        }
    }
}
