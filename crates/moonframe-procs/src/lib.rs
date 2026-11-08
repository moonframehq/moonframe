//! The `procs` native provider, for Lantern's process monitor: what runs on
//! this machine. It runs `ps`, `iostat`, `vm_stat` and `sysctl` with the
//! user's access (macOS), as Lantern's procs.py did.
//!
//! Publishes `app.procs` as each `iostat -n0 -w2` line comes (every two
//! seconds) and after each message:
//!
//! - `available`: false with `reason` when `iostat` cannot run
//! - `cpu`: busy share of all cores, 0 to 100; `user` and `system` split it
//! - `history`: the last 60 `cpu` readings, oldest first
//! - `load`: `[1m, 5m, 15m]` load averages
//! - `memory`: `{ used, total, active, wired, compressed, swap }` in bytes,
//!   and `pressure`: "normal", "warning" or "critical"
//! - `groups`: processes grouped by name, the busiest first (or the largest
//!   with sort = "memory"), at most 80, matching `query`: `{ name, pid (the
//!   busiest member), count, cpu (% of one core, summed), mem (bytes), open,
//!   children (when open: up to 12 { pid, name, cpu, mem }) }`
//! - `total`: how many processes match; `query`, `sort` as asked
//!
//! Messages, any combination of keys: `query = "<text>"` keeps processes
//! whose name or pid contains it; `sort = "cpu" | "memory"`; `expand =
//! "<name>"` shows or hides that group's members.
//!
//! `ps` and the memory tools run on a worker thread, so a message never
//! waits for them.

mod read;
mod table;

use {
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, to_json},
    read::{MemoryText, Proc, Reading},
    serde_json::{Value, json},
    std::{
        io::Read,
        process::{Child, ChildStdout, Command, Stdio},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, RecvTimeoutError, Sender},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    },
    table::Monitor,
};

/// The most of a command's output read.
const MAX_OUTPUT: u64 = 8 << 20;
/// The longest `iostat` line kept; a longer one is dropped.
const MAX_LINE: usize = 64 << 10;
/// How long a stopping provider waits for its worker.
const LEAVING: Duration = Duration::from_secs(1);
/// How often a waiting worker looks whether it was stopped.
const WAKE: Duration = Duration::from_millis(100);

pub fn native() -> Box<dyn Native> {
    Box::new(Provider::new(
        &["iostat", "-n0", "-w2"],
        Box::new(|| (system_processes(), system_memory())),
    ))
}

/// What one look at the system finds: its processes and `memory`.
type Look = Box<dyn Fn() -> (Vec<Proc>, Value) + Send>;

/// A command's output, or `None` when it cannot run or fails.
fn run(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = Vec::new();
    let read = child
        .stdout
        .take()
        .map(|stdout| stdout.take(MAX_OUTPUT).read_to_end(&mut out));
    // Past the bound it may be stuck writing: it is stopped, not read on.
    if out.len() as u64 >= MAX_OUTPUT {
        let _ = child.kill();
    }
    let status = child.wait().ok()?;
    (status.success() && matches!(read, Some(Ok(_))))
        .then(|| String::from_utf8_lossy(&out).into_owned())
}

fn system_processes() -> Vec<Proc> {
    read::processes(&run("ps", &["-Axo", "pid=,pcpu=,rss=,comm="]).unwrap_or_default())
}

fn system_memory() -> Value {
    read::memory(&MemoryText {
        memsize: run("sysctl", &["-n", "hw.memsize"]),
        vm_stat: run("vm_stat", &[]),
        swap: run("sysctl", &["-n", "vm.swapusage"]),
        pressure: run("sysctl", &["-n", "kern.memorystatus_vm_pressure_level"]),
    })
}

/// What the worker is given.
enum Work {
    Message(Value),
    Reading(Reading),
}

struct Provider {
    iostat: Vec<String>,
    look: Option<Look>,
    child: Option<Child>,
    inbox: Option<Sender<Work>>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Provider {
    fn new(iostat: &[&str], look: Look) -> Self {
        Self {
            iostat: iostat.iter().map(|part| (*part).to_owned()).collect(),
            look: Some(look),
            child: None,
            inbox: None,
            stopped: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }
}

impl Native for Provider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let (program, args) = self.iostat.split_first().ok_or("no iostat command")?;
        let spawned = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(_) => {
                host.publish_json(
                    &json!({ "available": false, "reason": "iostat is not available" }),
                );
                return Ok(());
            },
        };
        let (inbox, work) = mpsc::channel();
        if let Some(stdout) = child.stdout.take() {
            let readings = inbox.clone();
            thread::Builder::new()
                .name("procs-iostat".into())
                .spawn(move || read_iostat(stdout, &readings))
                .map_err(|problem| problem.to_string())?;
        }
        self.child = Some(child);
        self.inbox = Some(inbox);
        let look = self.look.take().ok_or("started twice")?;
        let host = host.clone();
        let stopped = Arc::clone(&self.stopped);
        self.worker = Some(
            thread::Builder::new()
                .name("procs".into())
                .spawn(move || serve(&host, &work, &look, &stopped))
                .map_err(|problem| problem.to_string())?,
        );
        Ok(())
    }

    fn tick(&mut self, _host: &Handle) {}

    fn receive(&mut self, _host: &Handle, _from: &str, message: Data) {
        // Without `iostat` it has nothing more to show, as procs.py had not.
        if let Some(inbox) = &self.inbox {
            let _ = inbox.send(Work::Message(to_json(&message)));
        }
    }
}

impl Drop for Provider {
    /// Stops `iostat` and the worker, and waits a moment for the worker: a
    /// replacement must not have an old look published over its own.
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.inbox = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(worker) = self.worker.take() {
            let started = Instant::now();
            while !worker.is_finished() && started.elapsed() < LEAVING {
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

/// Sends each of `iostat`'s readings until it ends, skipping the first,
/// which averages since boot.
fn read_iostat(mut stdout: ChildStdout, readings: &Sender<Work>) {
    let mut pending = Vec::new();
    let mut chunk = [0; 4096];
    let mut first = true;
    loop {
        let got = match stdout.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(got) => got,
        };
        pending.extend_from_slice(&chunk[..got]);
        while let Some(end) = pending.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            let Some(reading) = read::iostat(&String::from_utf8_lossy(&line)) else {
                continue;
            };
            if std::mem::take(&mut first) {
                continue;
            }
            if readings.send(Work::Reading(reading)).is_err() {
                return;
            }
        }
        if pending.len() > MAX_LINE {
            pending.clear();
        }
    }
}

/// The worker: publishes a first look, then one after each batch of
/// readings and messages, until the provider stops.
fn serve(host: &Handle, work: &Receiver<Work>, look: &Look, stopped: &AtomicBool) {
    let mut monitor = Monitor::default();
    let publish = |monitor: &Monitor| {
        let (procs, memory) = look();
        if !stopped.load(Ordering::SeqCst) {
            host.publish_json(&monitor.snapshot(&procs, memory));
        }
    };
    publish(&monitor);
    loop {
        // `iostat`'s reader holds a sender too: the flag ends the wait.
        let next = match work.recv_timeout(WAKE) {
            Ok(next) => next,
            Err(RecvTimeoutError::Timeout) if !stopped.load(Ordering::SeqCst) => continue,
            Err(_) => return,
        };
        let mut next = Some(next);
        // What came in meanwhile is shown in the one look.
        while let Some(item) = next.take().or_else(|| work.try_recv().ok()) {
            match item {
                Work::Message(message) => monitor.receive(&message),
                Work::Reading(reading) => monitor.read(reading),
            }
        }
        if stopped.load(Ordering::SeqCst) {
            return;
        }
        publish(&monitor);
    }
}

#[cfg(test)]
mod tests;
