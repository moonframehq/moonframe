//! `moonframe.view` in the window: a provider's call, on its own thread,
//! asks the front main window to read a mod's view or act on it, and waits
//! for the answer. Each ask runs on the main thread as a job of its own, so
//! what an act sets off (a field's `change`) has landed before the next
//! ask reads the view again.

use {
    super::*,
    moonframe_core::{Acting, Input, ViewAct},
    std::sync::{Mutex, PoisonError, mpsc},
};

/// Work for the main thread, from a provider's thread.
type Job = Box<dyn FnOnce(&mut App) + Send>;

/// Where jobs go once the app runs.
static JOBS: Mutex<Option<futures::channel::mpsc::UnboundedSender<Job>>> = Mutex::new(None);

/// Runs the jobs providers ask of the window, from now on.
pub(super) fn serve(cx: &mut App) {
    let (jobs, mut waiting) = futures::channel::mpsc::unbounded::<Job>();
    *JOBS.lock().unwrap_or_else(PoisonError::into_inner) = Some(jobs);
    cx.spawn(async move |cx| {
        while let Some(job) = waiting.next().await {
            cx.update(job);
        }
    })
    .detach();
}

/// Runs `work` on the front main window's app, on the main thread, and
/// waits up to `timeout` for what it answers: from a provider's thread,
/// never the main one.
pub(crate) fn on_front<R: Send + 'static>(
    timeout: Duration,
    work: impl FnOnce(&mut MoonframeApp, &mut Window, &mut Context<MoonframeApp>) -> Result<R, String>
    + Send
    + 'static,
) -> Result<R, String> {
    let (answer, answered) = mpsc::channel();
    let job: Job = Box::new(move |cx| {
        let result = match front_window(cx) {
            Some((app, window)) => window
                .update(cx, |_, window, cx| {
                    app.update(cx, |app, cx| work(app, window, cx))
                })
                .unwrap_or_else(|error| Err(format!("the window is gone: {error}"))),
            None => Err("the app has no main window open".to_owned()),
        };
        let _ = answer.send(result);
    });
    JOBS.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .ok_or("the app has not started its windows")?
        .unbounded_send(job)
        .map_err(|_| "the app is closing".to_owned())?;
    answered
        .recv_timeout(timeout)
        .map_err(|_| "the window did not answer in time".to_owned())?
}

/// The front main window's app, and its window.
fn front_window(cx: &App) -> Option<(Entity<MoonframeApp>, AnyWindowHandle)> {
    let app = hub::front(cx)?;
    let id = app.read(cx).main_window;
    let window = cx
        .windows()
        .into_iter()
        .find(|window| window.window_id() == id)?;
    Some((app, window))
}

impl MoonframeApp {
    /// Does `act` to the element `reference` of the mod `name`'s view, as
    /// a person's click, typing or key would: the element's own messages,
    /// a field's through the field itself while it is drawn.
    pub(crate) fn act_on_view(
        &mut self,
        name: &str,
        reference: &str,
        act: &ViewAct,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match self.workspace.view_act(name, reference, act)? {
            Acting::Send { owner, action } => self.dispatch(&owner, &action, window, cx),
            Acting::Fill {
                owner,
                id,
                to,
                change,
                text,
            } => {
                let typed = id
                    .as_deref()
                    .is_some_and(|id| self.inputs.fill(&owner, id, &text, window, cx).is_ok());
                if !typed && let Some(change) = change {
                    let action = Action::Send {
                        to,
                        msg: Input::message_at(&change, &text, text.len()),
                    };
                    self.dispatch(&owner, &action, window, cx);
                }
            },
            Acting::FieldKey {
                owner,
                id,
                to,
                msg,
                value,
            } => {
                let value = id
                    .and_then(|id| self.inputs.text(&owner, &id, cx))
                    .unwrap_or(value);
                let action = Action::Send {
                    to,
                    msg: Input::message(&msg, &value),
                };
                self.dispatch(&owner, &action, window, cx);
            },
        }
        Ok(())
    }

    /// What the mod `name` and the copies and components it drew report
    /// wrong, as `name: problem` lines.
    pub(crate) fn view_problems(&self, name: &str) -> Vec<String> {
        let own = |other: &str| {
            other == name
                || other
                    .strip_prefix(name)
                    .is_some_and(|rest| rest.starts_with(['/', '#']))
        };
        self.workspace
            .problems()
            .into_iter()
            .filter(|(other, _)| own(other))
            .map(|(other, problem)| format!("{other}: {problem}"))
            .collect()
    }
}
