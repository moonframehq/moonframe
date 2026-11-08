//! Running a program to its end without trusting it to end: bounded by a
//! timeout, and by what it left holding its output.

use {
    crate::value::lock,
    std::{
        io::{Read, Write},
        process::{Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    },
};

/// How long output may stay open once the program has gone: something it
/// started can hold the pipe for ever.
const DRAIN: Duration = Duration::from_secs(1);
const SLOWEST_POLL: Duration = Duration::from_millis(20);
/// The most output kept from a program, as the Node providers allowed git:
/// past it the program is stopped, rather than held in memory whole.
const MAX_OUTPUT: usize = 64 * 1024 * 1024;

/// How a program went.
pub(crate) struct Ran {
    /// It left with status 0.
    pub(crate) ok: bool,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: String,
    /// It could not start, ran out of time, or how it left otherwise.
    pub(crate) failure: Option<String>,
}

impl Ran {
    /// Its output, or what to tell the user: stderr if it said anything.
    pub(crate) fn output(self) -> Result<String, String> {
        if self.ok {
            return Ok(String::from_utf8_lossy(&self.stdout).into_owned());
        }
        let said = self.stderr.trim();
        Err(if said.is_empty() {
            self.failure.unwrap_or_else(|| "it failed".to_owned())
        } else {
            said.to_owned()
        })
    }
}

/// Output as it arrives, kept apart from the end of it: a login shell's
/// background job can hold the pipe open long after the shell has spoken.
struct Collected {
    bytes: Arc<Mutex<Vec<u8>>>,
    ended: mpsc::Receiver<()>,
    /// It said more than [`MAX_OUTPUT`].
    over: Arc<AtomicBool>,
}

impl Collected {
    fn new(mut stream: impl Read + Send + 'static, limit: usize) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let over = Arc::new(AtomicBool::new(false));
        let (done, ended) = mpsc::channel();
        let (shared, full) = (Arc::clone(&bytes), Arc::clone(&over));
        thread::spawn(move || {
            let mut chunk = [0; 16 * 1024];
            while let Ok(count) = stream.read(&mut chunk) {
                if count == 0 {
                    break;
                }
                let mut kept = lock(&shared);
                if kept.len() + count > limit {
                    full.store(true, Ordering::SeqCst);
                    break;
                }
                kept.extend_from_slice(&chunk[..count]);
            }
            let _ = done.send(());
        });
        Self { bytes, ended, over }
    }

    fn over(&self) -> bool {
        self.over.load(Ordering::SeqCst)
    }

    /// What arrived, waiting a moment for the rest.
    fn take(self) -> Vec<u8> {
        let _ = self.ended.recv_timeout(DRAIN);
        std::mem::take(&mut *lock(&self.bytes))
    }
}

/// Runs `command`, with `input` on its stdin, for at most `timeout`.
pub(crate) fn run(command: Command, input: Option<&str>, timeout: Duration) -> Ran {
    run_until(command, input, timeout, &AtomicBool::new(false))
}

/// [`run`], stopped early once `stop` is set: the provider is going, and
/// nothing may outlive it.
pub(crate) fn run_until(
    command: Command,
    input: Option<&str>,
    timeout: Duration,
    stop: &AtomicBool,
) -> Ran {
    run_within(command, input, timeout, stop, MAX_OUTPUT)
}

/// [`run_until`], keeping at most `limit` bytes of each output.
fn run_within(
    mut command: Command,
    input: Option<&str>,
    timeout: Duration,
    stop: &AtomicBool,
    limit: usize,
) -> Ran {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Ran {
                ok: false,
                stdout: Vec::new(),
                stderr: String::new(),
                failure: Some(error.to_string()),
            };
        },
    };
    if let (Some(mut stdin), Some(input)) = (child.stdin.take(), input) {
        let input = input.to_owned();
        thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
    }
    let stdout = child.stdout.take().map(|out| Collected::new(out, limit));
    let stderr = child.stderr.take().map(|err| Collected::new(err, limit));
    let too_much = || {
        [&stdout, &stderr]
            .into_iter()
            .flatten()
            .any(Collected::over)
    };
    let until = Instant::now() + timeout;
    let mut poll = Duration::from_millis(1);
    let mut failure = None;
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if too_much() {
                    break false;
                }
                if !status.success() {
                    failure = Some(status.to_string());
                }
                break status.success();
            },
            Ok(None) if Instant::now() >= until || stop.load(Ordering::SeqCst) || too_much() => {
                let _ = child.kill();
                let _ = child.wait();
                failure = Some(if stop.load(Ordering::SeqCst) {
                    "stopped".to_owned()
                } else if too_much() {
                    String::new()
                } else {
                    format!("no answer after {}s", timeout.as_secs())
                });
                break false;
            },
            Ok(None) => {
                thread::sleep(poll);
                poll = (poll * 2).min(SLOWEST_POLL);
            },
            Err(error) => {
                failure = Some(error.to_string());
                break false;
            },
        }
    };
    // Decided once the readers are done: a program can leave before what
    // it said has all been read, and so before it is found to be too much.
    let flags: Vec<Arc<AtomicBool>> = [&stdout, &stderr]
        .into_iter()
        .flatten()
        .map(|output| Arc::clone(&output.over))
        .collect();
    let taken = |output: Option<Collected>| output.map(Collected::take).unwrap_or_default();
    let (stdout, stderr) = (taken(stdout), taken(stderr));
    let over = flags.iter().any(|flag| flag.load(Ordering::SeqCst));
    if over {
        failure = Some(format!("more than {} MiB of output", limit / 1024 / 1024));
    }
    Ran {
        ok: ok && !over,
        stdout,
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        failure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn output_and_input_pass_through() {
        let ran = run(shell("cat"), Some("hello"), Duration::from_secs(5));
        assert_eq!(ran.output(), Ok("hello".to_owned()));
    }

    #[test]
    fn stderr_is_the_reason() {
        let ran = run(shell("echo nope >&2; exit 3"), None, Duration::from_secs(5));
        assert_eq!(ran.output(), Err("nope".to_owned()));
    }

    #[test]
    fn what_a_lingering_job_holds_open_does_not_cost_the_output() {
        let started = Instant::now();
        let ran = run(shell("echo said; sleep 30 &"), None, Duration::from_secs(5));
        assert!(started.elapsed() < Duration::from_secs(4));
        assert_eq!(ran.output(), Ok("said\n".to_owned()));
    }

    #[test]
    fn output_past_the_limit_stops_the_program() {
        let started = Instant::now();
        let never = AtomicBool::new(false);
        let ran = run_within(shell("yes"), None, Duration::from_secs(30), &never, 1 << 20);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(!ran.ok);
        assert!(ran.stdout.len() <= 1 << 20);
        assert_eq!(ran.failure.as_deref(), Some("more than 1 MiB of output"));
    }

    #[test]
    fn a_program_is_stopped_when_told() {
        let stop = Arc::new(AtomicBool::new(false));
        let later = Arc::clone(&stop);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            later.store(true, Ordering::SeqCst);
        });
        let started = Instant::now();
        let ran = run_until(shell("exec sleep 30"), None, Duration::from_secs(30), &stop);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(ran.failure.as_deref(), Some("stopped"));
    }

    #[test]
    fn a_program_that_stays_is_stopped() {
        let started = Instant::now();
        let ran = run(shell("exec sleep 30"), None, Duration::from_millis(100));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(!ran.ok);
    }
}
