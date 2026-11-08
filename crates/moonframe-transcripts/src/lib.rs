//! The `history` native provider, Cinder's sessions (`examples/cinder`):
//! the coding-agent CLI sessions in `~/.claude/projects` (one folder per
//! project, a JSONL transcript per session), grouped by repository, and the
//! transcript of the one opened. It reads those files with the user's
//! access, read-only, and never writes anything.
//!
//! Publishes `app.history`:
//!
//! - `available`, `source` ("cli" or "sample"), `total` (sessions found)
//! - `groups`: `{ name, path, sessions = { id, title, branch, age, pr, gone }
//!   }`, most recent first; `pr` when the session opened a pull request,
//!   `gone` when its folder no longer exists
//! - `transcript`: the opened session, or absent: `{ id, title, project,
//!   branch, hidden (earlier turns left out), turns = { from ("user" or
//!   "assistant"), parts = { kind ("text" or "tools"), text, tools = { name,
//!   detail } } } }`
//!
//! Messages: `{ open = id }` opens a transcript, `{ close = true }` drops
//! it, `{ refresh = true }` looks again.
//!
//! With no such folder, or `CINDER_SAMPLE` set in the environment, it shares
//! sample sessions instead. Reading a large transcript takes a while, so
//! `Native::receive` only posts: one thread reads, looks again every 30
//! seconds, and publishes.

mod meta;
mod sample;
mod sessions;
mod text;
mod transcript;

use {
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, to_json},
    serde_json::Value,
    sessions::Sessions,
    std::{
        env,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, RecvTimeoutError, Sender},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    },
};

/// How often the sessions are looked at again.
const SCAN: Duration = Duration::from_secs(30);
/// How long dropping the provider waits for its worker to leave: within
/// what the host gives a native provider.
const LEAVING: Duration = Duration::from_millis(1500);

pub fn native() -> Box<dyn Native> {
    Box::new(Provider::default())
}

#[derive(Default)]
struct Provider {
    inbox: Option<Sender<Value>>,
    /// Set as the provider goes: its worker publishes nothing after.
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    /// Where to look and whether to share samples, instead of what the
    /// environment says: for tests.
    #[cfg(test)]
    looks_in: Option<(String, bool)>,
}

/// Where the CLI keeps its sessions, with forward slashes.
fn root() -> String {
    let home = env::var("HOME")
        .ok()
        .filter(|home| !home.is_empty())
        .or_else(|| env::var("USERPROFILE").ok())
        .unwrap_or_default();
    format!("{}/.claude/projects", home.replace('\\', "/"))
}

impl Native for Provider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let root = root();
        let sample = env::var_os("CINDER_SAMPLE").is_some_and(|value| !value.is_empty())
            || !Path::new(&root).is_dir();
        #[cfg(test)]
        let (root, sample) = self.looks_in.clone().unwrap_or((root, sample));
        let sessions = Sessions::new(root, sample, Arc::clone(&self.stopped));
        let (inbox, messages) = mpsc::channel();
        let host = host.clone();
        let stopped = Arc::clone(&self.stopped);
        self.worker = Some(thread::spawn(move || {
            serve(&host, &messages, sessions, &stopped);
        }));
        self.inbox = Some(inbox);
        Ok(())
    }

    /// The worker keeps its own time.
    fn tick(&mut self, _host: &Handle) {}

    fn receive(&mut self, _host: &Handle, _from: &str, message: Data) {
        if let Some(inbox) = &self.inbox {
            let _ = inbox.send(to_json(&message));
        }
    }
}

impl Drop for Provider {
    /// Stops the worker and waits a moment for it. One still reading a
    /// large transcript publishes nothing once it comes back.
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.inbox = None;
        if let Some(worker) = self.worker.take() {
            let started = Instant::now();
            while !worker.is_finished() && started.elapsed() < LEAVING {
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

/// The worker: publishes what changed, takes messages in order, and looks
/// again every [`SCAN`].
fn serve(host: &Handle, messages: &Receiver<Value>, mut sessions: Sessions, stopped: &AtomicBool) {
    sessions.scan();
    let mut scanned = Instant::now();
    let mut last = String::new();
    loop {
        let value = sessions.view();
        let line = value.to_string();
        if stopped.load(Ordering::SeqCst) {
            return;
        }
        if line != last {
            host.publish_json(&value);
            last = line;
        }
        match messages.recv_timeout(SCAN.saturating_sub(scanned.elapsed())) {
            Ok(message) => {
                sessions.receive(&message);
                // The rest already sent, before publishing once.
                while let Ok(message) = messages.try_recv() {
                    sessions.receive(&message);
                }
            },
            Err(RecvTimeoutError::Timeout) => {},
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if scanned.elapsed() >= SCAN {
            sessions.scan();
            scanned = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests;
