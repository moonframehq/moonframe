//! The readings on macOS, from its tools: `iostat -n0 -w2` for the CPU
//! split and the load averages, `ps` for processes, and `vm_stat` and
//! `sysctl` for memory counted as Activity Monitor counts it.

use {
    crate::{
        Look, Provider, Work, lines,
        read::{self, MemoryText},
        table::Proc,
    },
    moonframe_providers::Native,
    serde_json::Value,
    std::{
        io::Read,
        process::{Child, ChildStdout, Command, Stdio},
        sync::mpsc::Sender,
        thread,
    },
};

/// The most of a command's output read.
const MAX_OUTPUT: u64 = 8 << 20;
/// The longest `iostat` line kept; a longer one is dropped.
const MAX_LINE: usize = 64 << 10;

#[cfg_attr(
    test,
    allow(dead_code, reason = "the app's provider; tests drive their own")
)]
pub(crate) fn native() -> Box<dyn Native> {
    let look: Look = Box::new(|| (system_processes(), system_memory()));
    Box::new(Provider::new(&["iostat", "-n0", "-w2"], look))
}

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

/// Starts `iostat` with its reader sending to `readings`: `None` when it
/// cannot run.
pub(crate) fn spawn(iostat: &[String], readings: Sender<Work>) -> Result<Option<Child>, String> {
    let Some((program, args)) = iostat.split_first() else {
        return Err("no iostat command".into());
    };
    let spawned = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = spawned else {
        return Ok(None);
    };
    if let Some(stdout) = child.stdout.take() {
        thread::Builder::new()
            .name("procs-iostat".into())
            .spawn(move || read_iostat(stdout, &readings))
            .map_err(|problem| problem.to_string())?;
    }
    Ok(Some(child))
}

/// Sends each of `iostat`'s readings until it ends, skipping the first,
/// which averages since boot.
fn read_iostat(mut stdout: ChildStdout, readings: &Sender<Work>) {
    let mut lines = lines::Lines::new(MAX_LINE);
    let mut chunk = [0; 4096];
    let mut first = true;
    loop {
        let got = match stdout.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(got) => got,
        };
        for line in lines.feed(&chunk[..got]) {
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
    }
}
