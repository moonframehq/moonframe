//! The project the user works in: the git repository of the terminal they
//! last used. A thread asks that terminal which folder it is in, every
//! second and at once when another terminal takes the keyboard. Once that
//! folder is in another repository, the files, git and docs providers move
//! there (see [`moonframe_providers::set_project`]). Outside any
//! repository, or in home (a dotfiles repository), they stay where they
//! were.
//!
//! The projects followed are kept in a file beside the app's state, so a
//! markdown tab restored at the next launch may still read and save its
//! file in the project it came from.

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
/// Projects kept across launches, the most recent last.
const KEEP: usize = 64;

/// The terminal followed, shared with the thread that asks it.
pub(crate) struct Project {
    followed: Arc<Mutex<Option<Folder>>>,
    /// Wakes the thread to look at once: another terminal is followed.
    poke: mpsc::Sender<()>,
}

impl Project {
    /// Starts following; the providers hear each move through the app.
    pub(crate) fn start(profile: Option<&str>, cx: &mut Context<MoonframeApp>) -> Self {
        let kept = kept_file(profile);
        let mut projects = read_kept(&kept);
        moonframe_providers::remember_projects(projects.iter().cloned());
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
                        keep(&kept, &mut projects, top);
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

/// The file the projects followed are kept in: beside the app's state,
/// one per profile.
fn kept_file(profile: Option<&str>) -> PathBuf {
    let name = match profile {
        Some(profile) => format!("projects-{profile}.txt"),
        None => "projects.txt".to_owned(),
    };
    crate::paths::state(profile).with_file_name(name)
}

/// The projects kept by an earlier launch: absolute folders, one a line.
fn read_kept(file: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .collect()
}

/// Adds `project` to those kept, the most recent last, and writes them.
fn keep(file: &Path, projects: &mut Vec<PathBuf>, project: PathBuf) {
    projects.retain(|kept| kept != &project);
    projects.push(project);
    let excess = projects.len().saturating_sub(KEEP);
    projects.drain(..excess);
    let text: String = projects
        .iter()
        .filter_map(|path| path.to_str())
        .filter(|path| !path.contains('\n'))
        .map(|path| format!("{path}\n"))
        .collect();
    let written = file
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(file, text));
    if let Err(error) = written {
        tracing::warn!("cannot keep the projects in {}: {error}", file.display());
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    // Not `super::*`: gpui_kit's `test` would replace the standard one.
    use {
        super::{KEEP, keep, read_kept},
        std::path::PathBuf,
    };

    #[test]
    fn kept_projects_come_back_newest_last_without_repeats_or_excess() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("state").join("projects.txt");
        let mut projects = read_kept(&file);
        assert!(projects.is_empty());
        for index in 0..KEEP + 2 {
            keep(
                &file,
                &mut projects,
                PathBuf::from(format!("/repo/{index}")),
            );
        }
        keep(&file, &mut projects, PathBuf::from("/repo/5"));
        let read = read_kept(&file);
        assert_eq!(read, projects);
        assert_eq!(read.len(), KEEP);
        assert_eq!(read.last(), Some(&PathBuf::from("/repo/5")));
        assert_eq!(read.iter().filter(|path| path.ends_with("5")).count(), 1);
        assert!(!read.contains(&PathBuf::from("/repo/0")));
    }
}
