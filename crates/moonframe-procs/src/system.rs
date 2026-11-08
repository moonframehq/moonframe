//! The readings on Linux and Windows: processes and the load averages
//! through `sysinfo`, the CPU split from the system's CPU times (sampled
//! every two seconds, as `iostat -w2` prints), and memory from
//! `/proc/meminfo` on Linux and `sysinfo` on Windows.

use {
    crate::{
        Look, Provider,
        sample::{self, Sample, Times},
        table::Proc,
    },
    moonframe_providers::Native,
    serde_json::Value,
    std::time::Instant,
    sysinfo::{
        MINIMUM_CPU_UPDATE_INTERVAL, ProcessRefreshKind, ProcessesToUpdate, System, ThreadKind,
    },
};

pub(crate) fn native() -> Box<dyn Native> {
    let provider = match System::new() {
        Ok(system) => Provider::sampled(Box::new(sample), look(system), sample::EVERY),
        // No process can be listed: it says it is not available.
        Err(_) => Provider::sampled(
            Box::new(|| None),
            Box::new(|| (Vec::new(), Value::Null)),
            sample::EVERY,
        ),
    };
    Box::new(provider)
}

fn sample() -> Option<Sample> {
    Some((times()?, load()))
}

/// The `[1m, 5m, 15m]` load averages; Windows keeps none, so 0s there.
fn load() -> [f64; 3] {
    System::load_average().map_or([0.0; 3], |load| [load.one, load.five, load.fifteen])
}

#[cfg(target_os = "linux")]
fn times() -> Option<Times> {
    crate::linux::times(&std::fs::read_to_string("/proc/stat").ok()?)
}

#[cfg(windows)]
fn times() -> Option<Times> {
    crate::windows::times()
}

#[cfg(target_os = "linux")]
fn memory(_system: &mut System) -> Value {
    crate::linux::memory(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default())
}

#[cfg(windows)]
fn memory(system: &mut System) -> Value {
    system.refresh_memory();
    crate::windows::memory(
        system.total_memory(),
        system.used_memory(),
        system.used_swap(),
    )
}

/// What `sysinfo` refreshes of each process: its CPU (a share of the time
/// since the last refresh), its memory, and on Linux its command, for a
/// name the kernel cut. Threads are left out, so none counts twice.
fn refreshed() -> ProcessRefreshKind {
    let kind = ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .without_tasks();
    #[cfg(target_os = "linux")]
    let kind = kind.with_cmd(sysinfo::UpdateKind::OnlyIfNotSet);
    kind
}

#[cfg(target_os = "linux")]
fn name(process: &sysinfo::Process) -> String {
    let command = process.cmd().first().map(|first| first.to_string_lossy());
    crate::linux::name(&process.name().to_string_lossy(), command.as_deref())
}

#[cfg(windows)]
fn name(process: &sysinfo::Process) -> String {
    process.name().to_string_lossy().into_owned()
}

/// Every process but threads, by pid, so equals keep one order.
fn processes(system: &System) -> Vec<Proc> {
    let mut procs: Vec<Proc> = system
        .processes()
        .values()
        .filter(|process| process.thread_kind() != Some(ThreadKind::Userland))
        .map(|process| Proc {
            pid: i64::from(process.pid().as_u32()),
            cpu: f64::from(process.cpu_usage()),
            mem: i64::try_from(process.memory()).unwrap_or(i64::MAX),
            name: name(process),
        })
        .collect();
    procs.sort_by_key(|proc| proc.pid);
    procs
}

/// Looks at the processes and memory. A look sooner than `sysinfo` can
/// measure CPU again (a message right after a reading) shows the last
/// processes rather than a share of too short a time.
fn look(mut system: System) -> Look {
    let mut procs = Vec::new();
    let mut last: Option<Instant> = None;
    Box::new(move || {
        if last.is_none_or(|at| at.elapsed() >= MINIMUM_CPU_UPDATE_INTERVAL) {
            system.refresh_processes_specifics(ProcessesToUpdate::All, true, refreshed());
            procs = processes(&system);
            last = Some(Instant::now());
        }
        (procs.clone(), memory(&mut system))
    })
}
