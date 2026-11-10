//! One `pi --mode rpc` child: JSONL both ways, split on LF only, as pi's
//! protocol requires, with commands matched to their responses by id.
//!
//! Nothing here blocks but [`Rpc::send`], which waits for the answer and so
//! belongs on a worker's thread: writes go through a thread of their own, so
//! a full pipe never holds up whoever wrote.

use {
    crate::{runtime::Runtime, value::lock},
    serde_json::{Map, Value, json},
    std::{
        collections::HashMap,
        io::{BufRead, BufReader, Read, Write},
        path::Path,
        process::{Child, ChildStdin, ExitStatus, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
            mpsc::{self, Sender},
        },
        thread,
        time::{Duration, Instant},
    },
};

/// How long pi has for most commands.
pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
/// How long pi has to leave once its stdin closes, before it gets a signal.
const STOP_GRACE: Duration = Duration::from_millis(1500);
/// What is kept of what pi says on stderr.
const STDERR_KEPT: usize = 4000;
/// How long a pi that has gone has for its last lines to arrive.
const LAST_LINES: Duration = Duration::from_millis(200);

type Answer = Result<Value, String>;

/// What a pi was started with, and what it reports to.
pub(crate) struct Start<'a> {
    pub(crate) program: &'a Path,
    pub(crate) args: &'a [String],
    pub(crate) cwd: &'a str,
    pub(crate) runtime: &'a Runtime,
    /// Each record that is not an answer, in order.
    pub(crate) on_event: Box<dyn FnMut(Value) + Send>,
    /// Once, as it goes: why, and what it said on stderr.
    pub(crate) on_exit: Box<dyn FnOnce(String, String) + Send>,
}

/// A running pi.
pub(crate) struct Rpc {
    pending: Mutex<HashMap<String, Sender<Answer>>>,
    next_id: AtomicU64,
    input: Mutex<Option<Sender<String>>>,
    exited: AtomicBool,
    pid: u32,
    stderr: Mutex<String>,
    touched: Mutex<Instant>,
}

impl Rpc {
    /// Starts `pi --mode rpc` with `start.args`.
    pub(crate) fn spawn(start: Start<'_>) -> Result<Arc<Self>, String> {
        let mut command = start.runtime.command(start.program);
        command
            .args(["--mode", "rpc"])
            .args(start.args)
            .current_dir(start.cwd)
            .env("PI_SKIP_VERSION_CHECK", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| format!("pi did not start: {error}"))?;
        let (stdin, stdout, stderr) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = (stdin, stdout, stderr) else {
            return Err("pi started without its pipes".to_owned());
        };
        let (input, lines) = mpsc::channel();
        let rpc = Arc::new(Self {
            pending: Mutex::default(),
            next_id: AtomicU64::new(1),
            input: Mutex::new(Some(input)),
            exited: AtomicBool::new(false),
            pid: child.id(),
            stderr: Mutex::default(),
            touched: Mutex::new(Instant::now()),
        });
        thread::spawn(move || write_lines(stdin, &lines));
        let keeper = Arc::clone(&rpc);
        thread::spawn(move || keeper.keep_stderr(stderr));
        let (read_all, read_done) = mpsc::channel();
        let reader = Arc::clone(&rpc);
        let on_event = start.on_event;
        thread::spawn(move || {
            reader.read_lines(stdout, on_event);
            let _ = read_all.send(());
        });
        let waiter = Arc::clone(&rpc);
        let on_exit = start.on_exit;
        thread::spawn(move || {
            let reason = waited(child);
            let _ = read_done.recv_timeout(LAST_LINES);
            waiter.finish(&reason, on_exit);
        });
        Ok(rpc)
    }

    fn read_lines(&self, stdout: impl Read, mut on_event: Box<dyn FnMut(Value) + Send>) {
        let mut stdout = BufReader::new(stdout);
        let mut line = Vec::new();
        loop {
            line.clear();
            match stdout.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => return,
                Ok(_) => {},
            }
            let text = String::from_utf8_lossy(&line);
            let text = text.strip_suffix('\n').unwrap_or(&text);
            let text = text.strip_suffix('\r').unwrap_or(text);
            if text.is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(text) {
                Ok(record) => {
                    if let Some(record) = self.answered(record) {
                        on_event(record);
                    }
                },
                Err(_) => {
                    let start = crate::value::first_chars(text, 200);
                    tracing::warn!("pilot: pi printed something else: {start}");
                },
            }
        }
    }

    /// Hands an answer to whoever waits for it; anything else comes back.
    fn answered(&self, record: Value) -> Option<Value> {
        let id = record["id"]
            .as_str()
            .filter(|_| record["type"] == "response");
        let waiting = id.and_then(|id| lock(&self.pending).remove(id));
        let Some(waiting) = waiting else {
            return Some(record);
        };
        let answer = if record["success"].as_bool().unwrap_or(false) {
            Ok(record["data"].clone())
        } else {
            let command = record["command"].as_str().unwrap_or("that");
            let otherwise = format!("{command} failed");
            Err(record["error"].as_str().map_or(otherwise, str::to_owned))
        };
        let _ = waiting.send(answer);
        None
    }

    fn keep_stderr(&self, stderr: impl Read) {
        let mut stderr = BufReader::new(stderr);
        let mut line = Vec::new();
        while matches!(stderr.read_until(b'\n', &mut line), Ok(count) if count > 0) {
            let mut kept = lock(&self.stderr);
            kept.push_str(&String::from_utf8_lossy(&line));
            let tail = crate::value::last_bytes(&kept, STDERR_KEPT).to_owned();
            *kept = tail;
            line.clear();
        }
    }

    fn finish(&self, reason: &str, on_exit: Box<dyn FnOnce(String, String) + Send>) {
        self.exited.store(true, Ordering::SeqCst);
        lock(&self.input).take();
        let waiting: Vec<Sender<Answer>> = lock(&self.pending).drain().map(|(_, s)| s).collect();
        for sender in waiting {
            let _ = sender.send(Err(format!("pi ended ({reason})")));
        }
        let said = lock(&self.stderr).trim().to_owned();
        on_exit(reason.to_owned(), said);
    }

    /// Whether it has gone.
    pub(crate) fn exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    /// When it last did anything.
    pub(crate) fn touched(&self) -> Instant {
        *lock(&self.touched)
    }

    /// Notes that it did something just now.
    pub(crate) fn touch(&self) {
        *lock(&self.touched) = Instant::now();
    }

    /// Writes `record` as a line, not waiting for anything.
    pub(crate) fn write(&self, record: &Value) {
        if let Some(input) = lock(&self.input).as_ref() {
            let _ = input.send(format!("{record}\n"));
        }
    }

    /// Sends a command now, not waiting: pi takes commands in the order
    /// they are written, so a change written here goes ahead of whatever is
    /// written after, whichever thread waits for the answers.
    pub(crate) fn ask(&self, kind: &str, fields: Value) -> Asked {
        let id = format!("c{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        let (answer, waiting) = mpsc::channel();
        let asked = Asked {
            kind: kind.to_owned(),
            id: id.clone(),
            waiting,
        };
        if self.exited() {
            return asked;
        }
        self.touch();
        lock(&self.pending).insert(id.clone(), answer);
        let mut record = Map::new();
        record.insert("id".to_owned(), json!(id));
        record.insert("type".to_owned(), json!(kind));
        if let Value::Object(fields) = fields {
            record.extend(fields);
        }
        self.write(&Value::Object(record));
        asked
    }

    /// Sends a command, and waits for its answer for up to `timeout`.
    pub(crate) fn send_within(
        &self,
        kind: &str,
        fields: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        let asked = self.ask(kind, fields);
        self.answer_within(asked, timeout)
    }

    /// Waits for the answer to `asked` for up to `timeout`.
    pub(crate) fn answer_within(&self, asked: Asked, timeout: Duration) -> Result<Value, String> {
        let answered = asked.waiting.recv_timeout(timeout);
        lock(&self.pending).remove(&asked.id);
        answered.unwrap_or_else(|_| {
            Err(if self.exited() {
                "pi is not running".to_owned()
            } else {
                format!("pi did not answer {}", asked.kind)
            })
        })
    }

    /// [`Rpc::send_within`], in the usual time.
    pub(crate) fn send(&self, kind: &str, fields: Value) -> Result<Value, String> {
        self.send_within(kind, fields, COMMAND_TIMEOUT)
    }

    /// Closing stdin is pi's orderly stop; a child that stays gets a signal.
    pub(crate) fn stop(self: &Arc<Self>) {
        if self.exited() {
            return;
        }
        lock(&self.input).take();
        let rpc = Arc::clone(self);
        thread::spawn(move || {
            thread::sleep(STOP_GRACE);
            if !rpc.exited() {
                terminate(rpc.pid);
            }
        });
    }
}

/// A command written, its answer still to come: [`Rpc::answer_within`].
pub(crate) struct Asked {
    kind: String,
    id: String,
    waiting: mpsc::Receiver<Answer>,
}

fn write_lines(mut stdin: ChildStdin, lines: &mpsc::Receiver<String>) {
    for line in lines {
        if stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.flush())
            .is_err()
        {
            return;
        }
    }
}

fn waited(mut child: Child) -> String {
    match child.wait() {
        Ok(status) => reason(status),
        Err(error) => error.to_string(),
    }
}

#[cfg(unix)]
fn reason(status: ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    match status.signal() {
        Some(signal) => format!("stopped by signal {signal}"),
        None => format!("exit {}", status.code().unwrap_or(-1)),
    }
}

#[cfg(not(unix))]
fn reason(status: ExitStatus) -> String {
    format!("exit {}", status.code().unwrap_or(-1))
}

#[cfg(unix)]
#[allow(unsafe_code, reason = "sending a signal; see SAFETY")]
fn terminate(pid: u32) {
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        // SAFETY: `kill` takes plain integers. The child is not reaped until
        // its waiter sees it leave, and the caller only comes here when that
        // has not happened, so the id is still this child's.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
fn terminate(_pid: u32) {}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used)]
mod tests {
    use {super::*, crate::Config, std::fs};

    fn script(dir: &Path, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("pi");
        // A sibling test can fork while a Rust file write is open, keeping
        // that descriptor alive until exec and causing Linux ETXTBSY here.
        // Only the single-threaded shell opens the executable for writing.
        let status = std::process::Command::new("/bin/sh")
            .args(["-c", "printf '%s' \"$1\" > \"$2\"", "write-test-executable"])
            .arg(format!("#!/bin/sh\n{body}\n"))
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success(), "writing {}: {status}", path.display());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn start(
        program: &Path,
        cwd: &Path,
    ) -> (Arc<Rpc>, mpsc::Receiver<Value>, mpsc::Receiver<String>) {
        let config = Config {
            agent_dir: cwd.into(),
            data_dir: cwd.into(),
            home: cwd.into(),
            pi: None,
            login_shell: false,
            env: Vec::new(),
        };
        let runtime = Runtime::new(&config);
        let (events, heard) = mpsc::channel();
        let (exit, gone) = mpsc::channel();
        let rpc = Rpc::spawn(Start {
            program,
            args: &[],
            cwd: &cwd.to_string_lossy(),
            runtime: &runtime,
            on_event: Box::new(move |record| {
                let _ = events.send(record);
            }),
            on_exit: Box::new(move |reason, _| {
                let _ = exit.send(reason);
            }),
        })
        .unwrap();
        (rpc, heard, gone)
    }

    #[test]
    fn an_answer_finds_its_command_and_events_flow_past_it() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(
            dir.path(),
            r#"read line
printf '{"type":"note"}\r\n'
printf '{"type":"response","id":"c1","success":true,"data":{"n":1}}\n'
read line
printf '{"type":"response","id":"c2","success":false,"error":"no"}\n'
cat >/dev/null"#,
        );
        let (rpc, heard, _gone) = start(&program, dir.path());
        assert_eq!(rpc.send("one", json!({})).unwrap()["n"], 1);
        assert_eq!(rpc.send("two", json!({})).unwrap_err(), "no");
        let note = heard.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(note["type"], "note");
        rpc.stop();
    }

    #[test]
    fn a_line_break_in_a_string_does_not_split_a_record() {
        let dir = tempfile::tempdir().unwrap();
        // U+2028 is a line separator to some readers, and not to pi's protocol.
        let program = script(
            dir.path(),
            "printf '{\"type\":\"note\",\"text\":\"a\\342\\200\\250b\"}\\n'\ncat >/dev/null",
        );
        let (rpc, heard, _gone) = start(&program, dir.path());
        let note = heard.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(note["text"], "a\u{2028}b");
        rpc.stop();
    }

    #[test]
    fn a_pi_that_goes_fails_what_waits_for_it() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "read line\nexit 3");
        let (rpc, _heard, gone) = start(&program, dir.path());
        let problem = rpc.send("anything", json!({})).unwrap_err();
        assert!(problem.contains("pi ended (exit 3)"), "{problem}");
        assert_eq!(gone.recv_timeout(Duration::from_secs(5)).unwrap(), "exit 3");
        assert_eq!(
            rpc.send("again", json!({})).unwrap_err(),
            "pi is not running"
        );
    }

    #[test]
    fn closing_stdin_stops_it_and_one_that_stays_gets_a_signal() {
        let dir = tempfile::tempdir().unwrap();
        let polite = script(dir.path(), "cat >/dev/null");
        let (rpc, _heard, gone) = start(&polite, dir.path());
        rpc.stop();
        assert_eq!(gone.recv_timeout(Duration::from_secs(1)).unwrap(), "exit 0");

        // `exec` so the signal reaches the program that ignores stdin.
        let deaf = script(dir.path(), "exec sleep 30");
        let (rpc, _heard, gone) = start(&deaf, dir.path());
        let stopped = Instant::now();
        rpc.stop();
        let reason = gone.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(reason.contains("signal"), "{reason}");
        assert!(stopped.elapsed() >= Duration::from_millis(1400));
    }
}
