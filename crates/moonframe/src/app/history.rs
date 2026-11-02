//! Undo Last Change and Redo Change: the window's own folder taken back
//! or forward one snapshot. Git runs off the UI thread; writing the files
//! is what reloads the window, as any other change does.

use super::*;

impl MoonframeApp {
    /// Takes a snapshot of the window's folder, saying `reason`, if
    /// anything changed since the last one.
    pub(super) fn snapshot(&self, reason: &'static str, cx: &mut Context<Self>) {
        let Some(history) = self.history.clone() else {
            return;
        };
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = history.capture(reason) {
                    tracing::warn!("no snapshot of {}: {error}", history.folder().display());
                }
                if reason == "launch"
                    && let Err(error) = history.tidy()
                {
                    tracing::warn!("cannot tidy the history: {error}");
                }
            })
            .detach();
    }

    /// Undoes the last change to the window's folder (`back`), or redoes
    /// the one undone last, and says which in a toast.
    pub(super) fn step_history(&mut self, back: bool, cx: &mut Context<Self>) {
        let Some(history) = self.history.clone() else {
            self.show_toast("No history to undo: the app runs without your folder", cx);
            return;
        };
        let step = cx.background_executor().spawn(async move {
            if back {
                history.undo()
            } else {
                history.redo()
            }
        });
        cx.spawn(async move |this, cx| {
            let stepped = step.await;
            let _ = this.update(cx, |app, cx| {
                let text = match (stepped, back) {
                    (Ok(Some(undone)), true) => format!("Undid {}", undone.label),
                    (Ok(Some(redone)), false) => format!("Redid {}", redone.label),
                    (Ok(None), true) => "Nothing to undo".to_owned(),
                    (Ok(None), false) => "Nothing to redo".to_owned(),
                    (Err(error), _) => {
                        let verb = if back {
                            "undo"
                        } else {
                            "redo"
                        };
                        tracing::warn!("{verb}: {error}");
                        format!("Could not {verb}: {error}")
                    },
                };
                app.show_toast_text(text.into(), cx);
            });
        })
        .detach();
    }
}
