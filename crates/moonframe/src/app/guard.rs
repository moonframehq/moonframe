//! Holding back closing and quitting for a mod (`app:guard`).
//!
//! This covers the main window's close button, `close-window`, and `quit`
//! with its menu item and key. The system quitting the app itself (logging
//! out, Quit in the Dock) goes around it: GPUI gives no way to refuse that.

use {
    super::*,
    moonframe_core::{Guard, with_fields},
    std::time::Instant,
};

/// What a guard holds back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Closing {
    Window,
    Quit,
}

impl Closing {
    fn name(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Quit => "quit",
        }
    }
}

/// The mod holding closing back, and the attempt it is dealing with.
pub(crate) struct CloseGuard {
    owner: String,
    reply: Data,
    timeout: Duration,
    /// The attempt waiting, and when the first was made.
    waiting: Option<(Closing, Instant)>,
}

/// Reaches the app from the `Quit` action, which is the application's and
/// has no window.
struct QuitGuard(WeakEntity<MoonframeApp>);

impl Global for QuitGuard {}

/// Quits, unless a mod holds it back.
pub(crate) fn quit(cx: &mut App) {
    let app = cx
        .try_global::<QuitGuard>()
        .and_then(|guard| guard.0.upgrade());
    let held = app.is_some_and(|app| app.update(cx, |app, cx| app.held(Closing::Quit, cx)));
    if !held {
        cx.quit();
    }
}

impl MoonframeApp {
    /// Lets the main window's close button and the `Quit` action ask.
    pub(super) fn watch_closing(window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();
        cx.set_global(QuitGuard(app.clone()));
        window.on_window_should_close(cx, move |_, cx| {
            !app.update(cx, |app, cx| app.held(Closing::Window, cx))
                .unwrap_or(false)
        });
    }

    /// Whether a mod holds `closing` back; it has been told. An attempt
    /// past the guard's timeout goes through, and ends the guard.
    pub(crate) fn held(&mut self, closing: Closing, cx: &mut Context<Self>) -> bool {
        let Some(guard) = &mut self.close_guard else {
            return false;
        };
        let first = guard.waiting.map_or_else(Instant::now, |(_, first)| first);
        if first.elapsed() > guard.timeout {
            tracing::warn!("{} did not release in time; closing", guard.owner);
            self.close_guard = None;
            return false;
        }
        guard.waiting = Some((closing, first));
        let (owner, reply) = (guard.owner.clone(), guard.reply.clone());
        let msg = with_fields(
            &reply,
            [("closing".to_owned(), Data::from(closing.name()))].into(),
        );
        self.workspace.send("app:guard", &owner, msg);
        self.flush("app", cx);
        true
    }

    /// Does what the mod `from` asked with `app:guard`. A preview asks
    /// nothing, and only the mod holding can let go.
    pub(super) fn guard_request(
        &mut self,
        from: &str,
        request: Guard,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if from.starts_with(moonframe_core::DEMO) {
            return;
        }
        if let Some(guard) = &self.close_guard
            && guard.owner != from
        {
            self.workspace.refuse(
                from,
                Error::new(format!("`app:guard`: {} is holding", guard.owner)),
            );
            return;
        }
        match request {
            Guard::Hold { reply, timeout } => {
                self.close_guard = Some(CloseGuard {
                    owner: from.to_owned(),
                    reply,
                    timeout,
                    waiting: None,
                });
            },
            Guard::Stop => self.close_guard = None,
            Guard::Release => match self.close_guard.take().and_then(|guard| guard.waiting) {
                Some((Closing::Window, _)) => window.remove_window(),
                Some((Closing::Quit, _)) => cx.quit(),
                None => {},
            },
        }
    }
}
