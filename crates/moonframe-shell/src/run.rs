//! One command: the user's shell running it in a folder, its output read
//! as it comes, and how it ended.

use {
    crate::ansi::{self, Output},
    serde_json::{Value, json},
    std::{
        io::Read,
        path::Path,
        process::{Child, Command, Stdio},
        sync::{Arc, Mutex, PoisonError},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    },
};

/// Lines kept of one command's output; earlier ones are dropped.
const MAX_LINES: usize = 5000;
/// The plain text of the output shared for copying, from its end, in bytes:
/// what a view may hold in one string.
const MAX_TEXT: usize = 15 * 1024;
/// The output shared of one command, from its end, in bytes.
const MAX_OUTPUT: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Running,
    /// It exited on its own, with this code (0 is success).
    Exited(i32),
    /// The user stopped it.
    Stopped,
    /// It could not start, or was ended by a signal it was not sent.
    Failed,
}

impl Status {
    fn name(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Exited(0) => "finished",
            Self::Exited(_) | Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

/// What the threads of a run share.
pub(crate) struct Shared {
    pub(crate) output: Output,
    pub(crate) status: Status,
    pub(crate) error: Option<String>,
    pub(crate) finished: Option<Instant>,
    pub(crate) finished_ms: Option<u64>,
    /// Stop was asked; the exit that follows is a stop.
    pub(crate) stopping: bool,
    /// Something changed since the provider last published.
    pub(crate) changed: bool,
}

pub(crate) struct Run {
    pub(crate) id: String,
    pub(crate) owner: String,
    pub(crate) command: String,
    pub(crate) cwd: String,
    started: Instant,
    started_ms: u64,
    pid: Option<u32>,
    pub(crate) shared: Arc<Mutex<Shared>>,
}

pub(crate) fn lock(shared: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

impl Run {
    /// Starts `command` in `cwd` with `shell` (`-l -c`), its output and
    /// errors on one pipe so they keep their order.
    pub(crate) fn start(
        id: String,
        owner: String,
        command: String,
        cwd: &Path,
        shell: &Path,
    ) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            output: Output::new(MAX_LINES),
            status: Status::Running,
            error: None,
            finished: None,
            finished_ms: None,
            stopping: false,
            changed: true,
        }));
        let mut run = Self {
            id,
            owner,
            command,
            cwd: cwd.display().to_string(),
            started: Instant::now(),
            started_ms: now_ms(),
            pid: None,
            shared,
        };
        match run.spawn(cwd, shell) {
            Ok(child) => {
                run.pid = Some(child.id());
                run.wait(child);
            },
            Err(error) => {
                let mut shared = lock(&run.shared);
                shared.status = Status::Failed;
                shared.error = Some(error);
                shared.finished = Some(Instant::now());
                shared.finished_ms = Some(now_ms());
            },
        }
        run
    }

    fn spawn(&self, cwd: &Path, shell: &Path) -> Result<Child, String> {
        let (reader, writer) =
            std::io::pipe().map_err(|error| format!("cannot open a pipe: {error}"))?;
        let errors = writer
            .try_clone()
            .map_err(|error| format!("cannot open a pipe: {error}"))?;
        let mut command = Command::new(shell);
        command
            .arg("-l")
            .arg("-c")
            .arg(&self.command)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::from(writer))
            .stderr(Stdio::from(errors))
            // Colour where programs choose it for a terminal they can't
            // see, and nothing that waits on a reader.
            .env("TERM", "xterm-256color")
            .env("CLICOLOR", "1")
            .env("COLUMNS", "120")
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .env("GIT_TERMINAL_PROMPT", "0");
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // Its own group, so Stop reaches what it started too.
            command.process_group(0);
        }
        let child = command
            .spawn()
            .map_err(|error| format!("cannot start {}: {error}", shell.display()))?;
        // The command now holds the only writers: the reader ends with it.
        drop(command);
        let shared = Arc::clone(&self.shared);
        thread::Builder::new()
            .name("shell-output".to_owned())
            .spawn(move || read(reader, &shared))
            .map_err(|error| format!("cannot read its output: {error}"))?;
        Ok(child)
    }

    /// Reaps the command on a thread of its own and records how it ended.
    fn wait(&self, mut child: Child) {
        let shared = Arc::clone(&self.shared);
        let spawned = thread::Builder::new()
            .name("shell-wait".to_owned())
            .spawn(move || {
                let status = child.wait();
                // Let the last output in before saying it ended.
                thread::sleep(Duration::from_millis(60));
                let mut shared = lock(&shared);
                shared.status = match status {
                    _ if shared.stopping => Status::Stopped,
                    Ok(status) => status.code().map_or(Status::Failed, Status::Exited),
                    Err(_) => Status::Failed,
                };
                shared.finished = Some(Instant::now());
                shared.finished_ms = Some(now_ms());
                shared.changed = true;
            });
        if let Err(error) = spawned {
            tracing::warn!("shell: cannot wait on a command: {error}");
        }
    }

    pub(crate) fn running(&self) -> bool {
        lock(&self.shared).status == Status::Running
    }

    /// Stops it and what it started: asked to end, then made to.
    pub(crate) fn stop(&self) {
        let Some(pid) = self.pid else {
            return;
        };
        {
            let mut shared = lock(&self.shared);
            if shared.status != Status::Running {
                return;
            }
            shared.stopping = true;
            shared.changed = true;
        }
        signal(pid, false);
        let shared = Arc::clone(&self.shared);
        let _ = thread::Builder::new()
            .name("shell-stop".to_owned())
            .spawn(move || {
                thread::sleep(Duration::from_secs(2));
                if lock(&shared).status == Status::Running {
                    signal(pid, true);
                }
            });
    }

    /// The run as views see it; `lines` is how many lines of output to
    /// share, from the end.
    pub(crate) fn json(&self, lines: usize) -> Value {
        let shared = lock(&self.shared);
        let all = shared.output.lines();
        let mut from = all.len().saturating_sub(lines);
        // At most MAX_OUTPUT bytes, from the end.
        let mut bytes = 0;
        for (at, line) in all.iter().enumerate().skip(from).rev() {
            bytes += line.len() + 1;
            if bytes > MAX_OUTPUT {
                from = at + 1;
                break;
            }
        }
        let shown = &all[from..];
        let output = shown.join("\n");
        let mut text = ansi::plain(&output);
        if text.len() > MAX_TEXT {
            let mut cut = text.len() - MAX_TEXT;
            while !text.is_char_boundary(cut) {
                cut += 1;
            }
            text.drain(..cut);
        }
        let elapsed = shared
            .finished
            .unwrap_or_else(Instant::now)
            .duration_since(self.started);
        json!({
            "id": self.id,
            "owner": self.owner,
            "command": self.command,
            "cwd": self.cwd,
            "status": shared.status.name(),
            "code": match shared.status {
                Status::Exited(code) => Some(code),
                _ => None,
            },
            "error": shared.error,
            "started": self.started_ms,
            "finished": shared.finished_ms,
            "elapsed_ms": u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            "output": output,
            "text": text,
            "lines": shared.output.len(),
            "hidden": from + shared.output.dropped,
        })
    }
}

fn read(mut reader: std::io::PipeReader, shared: &Mutex<Shared>) {
    let mut buffer = [0; 16 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                let mut shared = lock(shared);
                shared.output.feed(&buffer[..read]);
                shared.changed = true;
            },
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {},
            Err(_) => break,
        }
    }
}

/// Signals the group `pid` leads: to end, or (`kill`) at once.
#[cfg(unix)]
fn signal(pid: u32, kill: bool) {
    use rustix::process::{Pid, Signal, kill_process_group};
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return;
    };
    let signal = if kill {
        Signal::KILL
    } else {
        Signal::TERM
    };
    if let Err(error) = kill_process_group(pid, signal) {
        tracing::debug!("shell: cannot signal {pid:?}: {error}");
    }
}

#[cfg(not(unix))]
fn signal(pid: u32, _kill: bool) {
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .status();
}
