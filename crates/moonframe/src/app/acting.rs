//! `moonframe.view` in the window: a provider's call, on its own thread,
//! asks a main window to read a mod's view or act on it, and waits for the
//! answer. Each ask runs on the main thread as a job of its own, so what
//! an act sets off (a field's `change`) has landed before the next ask
//! reads the view again; the read goes to the window the act went to. A
//! job its caller stopped waiting for is dropped, never run late.

use {
    super::*,
    moonframe_core::{Acting, Input, ViewAct, ViewTree},
    std::{
        collections::BTreeSet,
        sync::{
            Mutex, PoisonError,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
    },
};

/// Work for the main thread, from a provider's thread.
type Job = Box<dyn FnOnce(&mut App) + Send>;

/// Where a provider's calls reach the app's main windows: empty until the
/// app runs them. Each app has its own, made with its launch.
#[derive(Clone, Default)]
pub(crate) struct Windows(Arc<Mutex<Option<futures::channel::mpsc::UnboundedSender<Job>>>>);

/// The window that took an act, for the read after it.
pub(crate) type Chosen = WindowId;

impl Windows {
    /// Runs the jobs providers send, from now on.
    pub(super) fn serve(&self, cx: &mut App) {
        let (jobs, mut waiting) = futures::channel::mpsc::unbounded::<Job>();
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(jobs);
        cx.spawn(async move |cx| {
            while let Some(job) = waiting.next().await {
                cx.update(job);
            }
        })
        .detach();
    }

    /// Runs `work` on the app of the main window `chosen`, else the front
    /// one, on the main thread, and waits up to `timeout` for its answer
    /// and the window it ran in: from a provider's thread, never the main
    /// one. Given up, it does not run later.
    pub(crate) fn on<R: Send + 'static>(
        &self,
        chosen: Option<Chosen>,
        timeout: Duration,
        work: impl FnOnce(
            &mut MoonframeApp,
            &mut Window,
            &mut Context<MoonframeApp>,
        ) -> Result<R, String>
        + Send
        + 'static,
    ) -> Result<(R, Chosen), String> {
        let (answer, answered) = mpsc::channel();
        let wanted = Arc::new(AtomicBool::new(true));
        let still = Arc::clone(&wanted);
        let job: Job = Box::new(move |cx| {
            if !still.load(Ordering::Acquire) {
                return;
            }
            let result = match main_window(chosen, cx) {
                Some((app, window)) => window
                    .update(cx, |_, window, cx| {
                        app.update(cx, |app, cx| work(app, window, cx))
                    })
                    .unwrap_or_else(|error| Err(format!("the window is gone: {error}")))
                    .map(|answer| (answer, window.window_id())),
                None => Err("the app has no such main window open".to_owned()),
            };
            let _ = answer.send(result);
        });
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .ok_or("the app has not started its windows")?
            .unbounded_send(job)
            .map_err(|_| "the app is closing".to_owned())?;
        let answered = answered.recv_timeout(timeout);
        wanted.store(false, Ordering::Release);
        answered.map_err(|_| "the window did not answer in time".to_owned())?
    }
}

/// The main window `chosen`, else the front one, and its app.
fn main_window(
    chosen: Option<Chosen>,
    cx: &App,
) -> Option<(Entity<MoonframeApp>, AnyWindowHandle)> {
    let app = match chosen {
        Some(id) => hub::every(cx)
            .into_iter()
            .find(|app| app.read(cx).main_window == id)?,
        None => hub::front(cx)?,
    };
    let id = app.read(cx).main_window;
    let window = cx
        .windows()
        .into_iter()
        .find(|window| window.window_id() == id)?;
    Some((app, window))
}

impl MoonframeApp {
    /// The mod `name`'s view as `reaches` lets it be read, its fields
    /// showing the text typed in them.
    pub(crate) fn read_view(
        &self,
        name: &str,
        reaches: &dyn Fn(&str) -> bool,
        cx: &App,
    ) -> Result<ViewTree, String> {
        let live = |owner: &str, id: &str| self.inputs.text(owner, id, cx);
        self.workspace.view_tree(name, reaches, &live)
    }

    /// Does `act` to the element `reference` of the mod `name`'s view, as
    /// a person's click, typing or key would: the element's own messages,
    /// a field's through the field itself while it is drawn.
    pub(crate) fn act_on_view(
        &mut self,
        name: &str,
        reaches: &dyn Fn(&str) -> bool,
        reference: &str,
        act: &ViewAct,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match self.workspace.view_act(name, reaches, reference, act)? {
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
                match change {
                    _ if typed => {},
                    Some(change) => {
                        let action = Action::Send {
                            to,
                            msg: Input::message_at(&change, &text, text.len()),
                        };
                        self.dispatch(&owner, &action, window, cx);
                    },
                    None => return Err("the field is not drawn, and has no `change`".to_owned()),
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

    /// What the mods `owners` (those a view read reached) report wrong, as
    /// `name: problem` lines.
    pub(crate) fn view_problems(&self, owners: &BTreeSet<String>) -> Vec<String> {
        self.workspace
            .problems()
            .into_iter()
            .filter(|(name, _)| owners.contains(*name))
            .map(|(name, problem)| format!("{name}: {problem}"))
            .collect()
    }
}
