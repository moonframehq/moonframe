//! The `procs` native provider, for Lantern's process monitor: what runs on
//! this machine, with the user's access. On macOS it runs `iostat`, `ps`,
//! `vm_stat` and `sysctl`, as Lantern's procs.py did (`tools`); on Linux
//! and Windows it reads the same through `sysinfo` and the system's own
//! counters (`system`): `/proc` on Linux, Win32 on Windows.
//!
//! Publishes `app.procs` every two seconds, as each CPU reading comes, and
//! after each message:
//!
//! - `available`: false with `reason` when the CPU cannot be read
//! - `cpu`: busy share of all cores, 0 to 100; `user` and `system` split it
//! - `history`: the last 60 `cpu` readings, oldest first
//! - `load`: `[1m, 5m, 15m]` load averages (0s on Windows, which keeps none)
//! - `memory`: `{ used, total, active, wired, compressed, swap }` in bytes,
//!   and `pressure`: "normal", "warning" or "critical". Counted as Activity
//!   Monitor counts it on macOS; elsewhere used is all but what is
//!   available, and a count the system does not keep is 0 (wired on Linux;
//!   active, wired and compressed on Windows) and pressure "normal"
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
//! The processes and memory are read on a worker thread, and the CPU on one
//! of its own, so a message never waits for them.

// keep-sorted start
#[cfg(any(test, not(any(target_os = "linux", windows))))]
mod lines;
#[cfg(any(test, target_os = "linux"))]
mod linux;
#[cfg(any(test, not(any(target_os = "linux", windows))))]
mod read;
#[cfg(any(test, target_os = "linux", windows))]
mod sample;
#[cfg(any(target_os = "linux", windows))]
mod system;
mod table;
#[cfg(any(test, not(any(target_os = "linux", windows))))]
mod tools;
#[cfg(any(test, windows))]
mod windows;
// keep-sorted end

#[cfg(test)]
use read::MemoryText;
#[cfg(any(test, target_os = "linux", windows))]
use sample::Sampler;
use {
    moonframe_core::Data,
    moonframe_providers::{Handle, Native, to_json},
    serde_json::{Value, json},
    std::{
        process::Child,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, RecvTimeoutError, Sender},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant},
    },
    table::{Monitor, Proc, Reading},
};

/// How long a stopping provider waits for its worker.
const LEAVING: Duration = Duration::from_secs(1);
/// How often a waiting worker looks whether it was stopped.
const WAKE: Duration = Duration::from_millis(100);

pub fn native() -> Box<dyn Native> {
    #[cfg(any(target_os = "linux", windows))]
    return system::native();
    #[cfg(not(any(target_os = "linux", windows)))]
    return tools::native();
}

/// What one look at the system finds: its processes and `memory`.
type Look = Box<dyn FnMut() -> (Vec<Proc>, Value) + Send>;

/// Where the CPU readings come from.
enum Source {
    /// A command printing `iostat -n0` lines (macOS).
    #[cfg(any(test, not(any(target_os = "linux", windows))))]
    Iostat(Vec<String>),
    /// Samples of the CPU times, one `every` so long (Linux and Windows).
    #[cfg(any(test, target_os = "linux", windows))]
    Sampled { sampler: Sampler, every: Duration },
}

/// What the worker is given.
enum Work {
    Message(Value),
    Reading(Reading),
}

struct Provider {
    source: Option<Source>,
    look: Option<Look>,
    child: Option<Child>,
    inbox: Option<Sender<Work>>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Provider {
    fn with(source: Source, look: Look) -> Self {
        Self {
            source: Some(source),
            look: Some(look),
            child: None,
            inbox: None,
            stopped: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    /// Reading the CPU from `iostat`, run as this command.
    #[cfg(any(test, not(any(target_os = "linux", windows))))]
    fn new(iostat: &[&str], look: Look) -> Self {
        let command = iostat.iter().map(|part| (*part).to_owned()).collect();
        Self::with(Source::Iostat(command), look)
    }

    /// Reading the CPU from samples of its times, one `every` so long.
    #[cfg(any(test, target_os = "linux", windows))]
    fn sampled(sampler: Sampler, look: Look, every: Duration) -> Self {
        Self::with(Source::Sampled { sampler, every }, look)
    }

    /// Starts the CPU readings, sent to `readings`: false when the CPU
    /// cannot be read, which it then says.
    fn read_cpu(&mut self, host: &Handle, readings: Sender<Work>) -> Result<bool, String> {
        let unavailable = |reason: &str| {
            host.publish_json(&json!({ "available": false, "reason": reason }));
            Ok(false)
        };
        match self.source.take().ok_or("started twice")? {
            #[cfg(any(test, not(any(target_os = "linux", windows))))]
            Source::Iostat(command) => match tools::spawn(&command, readings)? {
                Some(child) => self.child = Some(child),
                None => return unavailable("iostat is not available"),
            },
            #[cfg(any(test, target_os = "linux", windows))]
            Source::Sampled { mut sampler, every } => {
                let Some((first, _)) = sampler() else {
                    return unavailable("the CPU cannot be read");
                };
                let stopped = Arc::clone(&self.stopped);
                thread::Builder::new()
                    .name("procs-cpu".into())
                    .spawn(move || sample::every(sampler, first, every, &readings, &stopped))
                    .map_err(|problem| problem.to_string())?;
            },
        }
        Ok(true)
    }
}

impl Native for Provider {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        let (inbox, work) = mpsc::channel();
        if !self.read_cpu(host, inbox.clone())? {
            return Ok(());
        }
        self.inbox = Some(inbox);
        let look = self.look.take().ok_or("started twice")?;
        let host = host.clone();
        let stopped = Arc::clone(&self.stopped);
        self.worker = Some(
            thread::Builder::new()
                .name("procs".into())
                .spawn(move || serve(&host, &work, look, &stopped))
                .map_err(|problem| problem.to_string())?,
        );
        Ok(())
    }

    fn tick(&mut self, _host: &Handle) {}

    fn receive(&mut self, _host: &Handle, _from: &str, message: Data) {
        // Without CPU readings it has nothing more to show, as procs.py had
        // not without `iostat`.
        if let Some(inbox) = &self.inbox {
            let _ = inbox.send(Work::Message(to_json(&message)));
        }
    }
}

impl Drop for Provider {
    /// Stops the CPU's reader (and `iostat`) and the worker, and waits a
    /// moment for the worker: a replacement must not have an old look
    /// published over its own.
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

/// The worker: publishes a first look, then one after each batch of
/// readings and messages, until the provider stops.
fn serve(host: &Handle, work: &Receiver<Work>, mut look: Look, stopped: &AtomicBool) {
    let mut monitor = Monitor::default();
    let mut publish = |monitor: &Monitor| {
        let (procs, memory) = look();
        if !stopped.load(Ordering::SeqCst) {
            host.publish_json(&monitor.snapshot(&procs, memory));
        }
    };
    publish(&monitor);
    loop {
        // The CPU's reader holds a sender too: the flag ends the wait.
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
mod sampled_tests;
#[cfg(test)]
mod tests;
