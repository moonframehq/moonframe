//! The project the user works in: the git repository of the terminal they
//! last used. A thread asks that terminal which folder it is in, every
//! second and at once when another terminal takes the keyboard. Once that
//! folder is in another repository, the files, git and docs providers move
//! there (see [`moonframe_providers::set_project`]). Outside any
//! repository, or in home (a dotfiles repository), they stay where they
//! were.

use {
    crate::app::MoonframeApp,
    futures::StreamExt as _,
    gpui_kit::*,
    moonframe_terminal::Folder,
    std::{
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::{
            Arc, Mutex,
            mpsc::{self, RecvTimeoutError},
        },
        thread,
        time::Duration,
    },
};

/// How often the followed terminal is asked for its folder.
const LOOK_EVERY: Duration = Duration::from_secs(1);

/// The terminal followed, shared with the thread that asks it.
pub(crate) struct Project {
    followed: Arc<Mutex<Option<Folder>>>,
    /// Wakes the thread to look at once: another terminal is followed.
    poke: mpsc::Sender<()>,
}

impl Project {
    /// Starts following; the providers hear each move through the app.
    pub(crate) fn start(cx: &mut Context<MoonframeApp>) -> Self {
        let followed = Arc::new(Mutex::new(None));
        let (poke, poked) = mpsc::channel();
        let (moved, mut moves) = futures::channel::mpsc::unbounded::<()>();
        cx.spawn(async move |this, cx| {
            while moves.next().await.is_some() {
                while moves.try_recv().is_ok() {}
                if this
                    .update(cx, |app, _| app.providers.tell_project())
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let shared = Arc::downgrade(&followed);
        let spawned = thread::Builder::new()
            .name("project".into())
            .spawn(move || {
                let mut last: Option<PathBuf> = None;
                loop {
                    match poked.recv_timeout(LOOK_EVERY) {
                        Ok(()) | Err(RecvTimeoutError::Timeout) => {},
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                    // The app went: so does the thread.
                    let Some(shared) = shared.upgrade() else {
                        return;
                    };
                    let folder = shared.lock().ok().and_then(|folder| folder.clone());
                    drop(shared);
                    let Some(cwd) = folder.as_ref().and_then(Folder::get) else {
                        continue;
                    };
                    // A folder found in a repository is not asked again; one
                    // that is not is, as `git init` may make it one.
                    if last.as_ref() == Some(&cwd) {
                        continue;
                    }
                    let Some(top) = repository(&cwd) else {
                        last = None;
                        continue;
                    };
                    last = Some(cwd);
                    if moonframe_providers::set_project(&top) {
                        tracing::info!(project = %top.display(), "following the terminal's project");
                        if moved.unbounded_send(()).is_err() {
                            return;
                        }
                    }
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("cannot start a thread to follow the project: {error}");
        }
        Self { followed, poke }
    }

    /// Follows `folder`'s terminal from now on, when it is another one.
    pub(crate) fn follow(&self, folder: Folder) {
        let Ok(mut followed) = self.followed.lock() else {
            return;
        };
        if followed.as_ref().is_some_and(|old| old.same(&folder)) {
            return;
        }
        *followed = Some(folder);
        let _ = self.poke.send(());
    }
}

/// The git repository holding `dir`, with links resolved, unless it is
/// home.
fn repository(dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let found = String::from_utf8(output.stdout).ok()?;
    let top = PathBuf::from(found.trim_end_matches(['\r', '\n']))
        .canonicalize()
        .ok()?;
    let home = std::env::home_dir().and_then(|home| home.canonicalize().ok());
    (Some(&top) != home.as_ref()).then_some(top)
}
