//! The app checked before and after a run that changes it: a change that
//! adds problems is undone, so the window never stays broken. The check is
//! the app's own command (`Editable::check`), run once as the run starts,
//! beside the agent, and once its agent has exited.

use std::{
    collections::BTreeSet,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

/// The longest a check may take before it counts as failed.
const TIMEOUT: Duration = Duration::from_secs(120);
/// The most of a check's output kept, in bytes.
const MAX_OUTPUT: usize = 64 << 10;

/// What a check said.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Checked {
    pub(crate) ok: bool,
    /// Its problems, each line with its numbers blanked, so one that only
    /// moved is still the same problem.
    problems: BTreeSet<String>,
    /// Its output, as it said it.
    pub(crate) output: String,
    pub(crate) ms: u64,
}

impl Checked {
    /// The problems in `self` that `before` did not have: none when it
    /// passes, every one when it failed for no problem it named.
    pub(crate) fn added(&self, before: &Self) -> Vec<String> {
        if self.ok {
            return Vec::new();
        }
        let added: Vec<String> = self
            .output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter(|line| !before.problems.contains(&blank(line)))
            .map(str::to_owned)
            .collect();
        if added.is_empty() && before.ok {
            return vec!["the check failed".to_owned()];
        }
        added
    }
}

/// A check started on a thread of its own, to wait for later.
#[derive(Clone, Default)]
pub(crate) struct Pending(Arc<(Mutex<Option<Checked>>, Condvar)>);

impl Pending {
    /// Starts `command` in `cwd`.
    pub(crate) fn start(command: &str, cwd: &Path) -> Self {
        let pending = Self::default();
        let done = pending.clone();
        let (command, cwd) = (command.to_owned(), cwd.to_owned());
        let started = std::thread::Builder::new()
            .name("acp-check".to_owned())
            .spawn(move || done.set(run(&command, &cwd)));
        if let Err(error) = started {
            pending.set(Checked {
                output: format!("cannot check the app: {error}"),
                ..Checked::default()
            });
        }
        pending
    }

    fn set(&self, checked: Checked) {
        let (slot, ready) = &*self.0;
        if let Ok(mut slot) = slot.lock() {
            *slot = Some(checked);
            ready.notify_all();
        }
    }

    /// What it said, waiting for it.
    pub(crate) fn wait(&self) -> Option<Checked> {
        let (slot, ready) = &*self.0;
        let slot = slot.lock().ok()?;
        let (slot, _) = ready
            .wait_timeout_while(slot, TIMEOUT, |slot| slot.is_none())
            .ok()?;
        slot.clone()
    }
}

/// Runs the shell command `command` in `cwd` and reads what it says.
pub(crate) fn run(command: &str, cwd: &Path) -> Checked {
    let started = Instant::now();
    let mut shell = shell(command);
    shell
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::tree::own_group(&mut shell);
    let mut child = match shell.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Checked {
                output: format!("cannot run `{command}`: {error}"),
                ..Checked::default()
            };
        },
    };
    let readers: Vec<_> = [
        child
            .stdout
            .take()
            .map(|out| Box::new(out) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|err| Box::new(err) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|mut stream| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = (&mut stream)
                .take(MAX_OUTPUT as u64)
                .read_to_end(&mut bytes);
            // Drained past the limit, so the check never blocks on a pipe.
            let _ = std::io::copy(&mut stream, &mut std::io::sink());
            String::from_utf8_lossy(&bytes).into_owned()
        })
    })
    .collect();
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            },
            _ => {
                crate::tree::signal(child.id(), true);
                let _ = child.kill();
                let _ = child.wait();
                break None;
            },
        }
    };
    let output: String = readers
        .into_iter()
        .filter_map(|reader| reader.join().ok())
        .collect::<Vec<_>>()
        .join("");
    let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let ok = status.is_some_and(|status| status.success());
    let output = if status.is_none() {
        format!(
            "{output}\nthe check took longer than {}s",
            TIMEOUT.as_secs()
        )
    } else {
        output
    };
    Checked {
        ok,
        problems: output.lines().map(blank).collect(),
        output,
        ms,
    }
}

#[cfg(unix)]
fn shell(command: &str) -> Command {
    let mut shell = Command::new("/bin/sh");
    shell.arg("-c").arg(command);
    shell
}

#[cfg(not(unix))]
fn shell(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}

/// `line` with each run of digits as `#`.
fn blank(line: &str) -> String {
    let mut blanked = String::with_capacity(line.len());
    let mut digits = false;
    for c in line.trim().chars() {
        if c.is_ascii_digit() {
            if !digits {
                blanked.push('#');
            }
            digits = true;
        } else {
            blanked.push(c);
            digits = false;
        }
    }
    blanked
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn only_problems_a_change_added_count() {
        let dir = tempfile::tempdir().unwrap();
        let clean = run("echo fine", dir.path());
        assert!(clean.ok, "{clean:?}");
        let broken = run("echo 'mods/a/main.luau:12: oops' >&2; exit 1", dir.path());
        assert!(!broken.ok);
        assert_eq!(broken.added(&clean), ["mods/a/main.luau:12: oops"]);
        // The same problem, moved down a line, was there before.
        let moved = run("echo 'mods/a/main.luau:14: oops'; exit 1", dir.path());
        assert!(moved.added(&broken).is_empty());
        // A failure that names nothing, where it passed before.
        let silent = run("exit 3", dir.path());
        assert_eq!(silent.added(&clean), ["the check failed"]);
        assert!(clean.added(&broken).is_empty());
    }

    #[test]
    fn a_pending_check_is_waited_for() {
        let dir = tempfile::tempdir().unwrap();
        let pending = Pending::start("sleep 0.1; echo done", dir.path());
        let checked = pending.wait().unwrap();
        assert!(checked.ok);
        assert_eq!(checked.output.trim(), "done");
    }
}
