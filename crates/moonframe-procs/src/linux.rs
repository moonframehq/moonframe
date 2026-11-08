//! What Linux's `/proc` says, read: the CPU times in `/proc/stat`, memory
//! in `/proc/meminfo`, and a process's full name when the kernel cut it.
//! Each reads text a test can give.

use {
    crate::sample::Times,
    serde_json::{Value, json},
    std::collections::HashMap,
};

/// The bytes the kernel keeps of a process's name (`comm`).
const COMM: usize = 15;

/// The first line of `/proc/stat`, `cpu  user nice system idle iowait irq
/// softirq steal ...`, in ticks: nice counts as user, interrupts as system
/// and waiting on I/O as idle, as `top` splits them.
pub(crate) fn times(stat: &str) -> Option<Times> {
    let line = stat.lines().find(|line| line.starts_with("cpu "))?;
    let ticks: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .map(|field| field.parse().ok())
        .collect::<Option<_>>()?;
    if ticks.len() < 4 {
        return None;
    }
    let at = |index: usize| ticks.get(index).copied().unwrap_or(0);
    // Guest time is already in user and nice: the first eight are all.
    let total = (0..8).map(at).fold(0, u64::saturating_add);
    Some(Times {
        user: at(0).saturating_add(at(1)),
        system: at(2).saturating_add(at(5)).saturating_add(at(6)),
        idle: at(3).saturating_add(at(4)),
        total,
    })
}

/// `/proc/meminfo`'s `<name>: <count> kB` lines, in bytes.
fn fields(meminfo: &str) -> HashMap<&str, i64> {
    meminfo
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once(':')?;
            let mut words = rest.split_whitespace();
            let count: i64 = words.next()?.parse().ok()?;
            let scale = match words.next() {
                Some("kB") => 1024,
                None => 1,
                Some(_) => return None,
            };
            Some((name, count.saturating_mul(scale)))
        })
        .collect()
}

/// `memory` from `/proc/meminfo`: used is all but what is available, as
/// `free` counts it; compressed is the zswap pool when there is one. Linux
/// counts no wired memory and has no pressure level like macOS's: wired is
/// 0 and pressure "normal", which the monitor shows as no warning.
pub(crate) fn memory(meminfo: &str) -> Value {
    let found = fields(meminfo);
    let get = |name: &str| found.get(name).copied().unwrap_or(0);
    let total = get("MemTotal");
    let available = found
        .get("MemAvailable")
        .copied()
        .unwrap_or_else(|| get("MemFree"));
    json!({
        "used": total.saturating_sub(available).clamp(0, total.max(0)),
        "total": total,
        "active": get("Active"),
        "wired": 0,
        "compressed": get("Zswap"),
        "swap": get("SwapTotal").saturating_sub(get("SwapFree")).max(0),
        "pressure": "normal",
    })
}

/// A process's name: the kernel keeps 15 bytes of it, so a name that long
/// is taken whole from its command's first word (`argv[0]`, which a
/// program may have rewritten as its whole command line): the file name
/// there that starts with it, up to a space.
pub(crate) fn name(comm: &str, command: Option<&str>) -> String {
    if comm.len() >= COMM
        && let Some(first) = command
        && let Some(whole) = first
            .match_indices(comm)
            .filter(|(at, _)| *at == 0 || first[..*at].ends_with('/'))
            .filter_map(|(at, _)| first[at..].split_whitespace().next())
            .find(|whole| whole.len() > comm.len() && !whole.contains('/'))
    {
        return whole.to_owned();
    }
    comm.to_owned()
}
