//! What the system tools print, read: `iostat`'s lines, `ps`'s processes,
//! and memory from `sysctl` and `vm_stat`. Each reads text a test can give,
//! with the patterns procs.py used.

use {
    serde_json::{Value, json},
    std::collections::HashMap,
};

/// One process `ps` lists.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Proc {
    pub(crate) pid: i64,
    /// % of one core.
    pub(crate) cpu: f64,
    /// Resident memory, bytes.
    pub(crate) mem: i64,
    pub(crate) name: String,
}

/// One `iostat -n0` line: the CPU split and the load averages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Reading {
    pub(crate) user: i64,
    pub(crate) system: i64,
    pub(crate) idle: i64,
    pub(crate) load: [f64; 3],
}

/// The ASCII digits `text` starts with, and what follows.
fn digits(text: &str) -> (&str, &str) {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    text.split_at(end)
}

/// The digits and dots `text` starts with (`[\d.]+`), and what follows.
fn decimal(text: &str) -> (&str, &str) {
    let end = text
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(text.len());
    text.split_at(end)
}

/// `text` after at least one whitespace character (`\s+`), else `None`.
fn spaced(text: &str) -> Option<&str> {
    let rest = text.trim_start();
    (rest.len() < text.len()).then_some(rest)
}

/// A whole number; `None` for anything else.
fn whole(text: &str) -> Option<i64> {
    (!text.is_empty()).then(|| text.parse().ok()).flatten()
}

/// A number of digits and dots that is one; `None` for `.` or `1.2.3`.
fn float(text: &str) -> Option<f64> {
    (!text.is_empty()).then(|| text.parse().ok()).flatten()
}

/// A line `us sy id 1m 5m 15m` (`iostat -n0`), or `None` for its headers.
pub(crate) fn iostat(line: &str) -> Option<Reading> {
    let mut rest = line.trim_start();
    let mut whole_numbers = [0; 3];
    for slot in &mut whole_numbers {
        let (number, after) = digits(rest);
        *slot = whole(number)?;
        rest = spaced(after)?;
    }
    let mut load = [0.0; 3];
    for (at, slot) in load.iter_mut().enumerate() {
        let (number, after) = decimal(rest);
        *slot = float(number)?;
        // The last needs nothing after it.
        if at < 2 {
            rest = spaced(after)?;
        }
    }
    let [user, system, idle] = whole_numbers;
    Some(Reading {
        user,
        system,
        idle,
        load,
    })
}

/// One line of `ps -Axo pid=,pcpu=,rss=,comm=`, or `None`.
fn process(line: &str) -> Option<Proc> {
    let (pid, rest) = digits(line.trim_start());
    let pid = whole(pid)?;
    let (cpu, rest) = decimal(spaced(rest)?);
    let cpu = float(cpu)?;
    let (rss, rest) = digits(spaced(rest)?);
    let rss = whole(rss)?;
    let comm = spaced(rest)?.trim_end();
    // The command's last part: `ps` gives a path.
    let name = comm.rsplit('/').next().unwrap_or(comm);
    if comm.is_empty() || name.is_empty() {
        return None;
    }
    Some(Proc {
        pid,
        cpu,
        mem: rss.saturating_mul(1024),
        name: name.to_owned(),
    })
}

/// Every process `ps` printed, in its order.
pub(crate) fn processes(text: &str) -> Vec<Proc> {
    text.split('\n').filter_map(process).collect()
}

/// A `sysctl -n` number, or `default` when it gave none.
pub(crate) fn as_int(text: Option<&str>, default: i64) -> i64 {
    text.and_then(|text| text.trim().parse().ok())
        .unwrap_or(default)
}

/// `vm_stat`'s page size, 16 KiB when it says none.
fn page_size(vm: &str) -> i64 {
    let size = vm
        .match_indices("page size of ")
        .find_map(|(at, found)| {
            let (number, rest) = digits(&vm[at + found.len()..]);
            rest.starts_with(" bytes").then(|| whole(number)).flatten()
        })
        .unwrap_or(0);
    if size == 0 {
        16384
    } else {
        size
    }
}

/// `vm_stat`'s counts by name: lines `<name>: <count>.`
fn pages(vm: &str) -> HashMap<&str, i64> {
    let mut found = HashMap::new();
    for line in vm.split('\n') {
        // The first colon after which only spaces and a count follow.
        let count = line
            .match_indices(':')
            .filter(|(at, _)| *at > 0)
            .find_map(|(at, _)| {
                let (number, rest) = digits(spaced(&line[at + 1..])?);
                let rest = rest.strip_prefix('.').unwrap_or(rest);
                rest.is_empty()
                    .then(|| Some((&line[..at], whole(number)?)))
                    .flatten()
            });
        if let Some((name, count)) = count {
            found.insert(name, count);
        }
    }
    found
}

/// Swap in use, in bytes, from `sysctl -n vm.swapusage`.
fn swap_used(text: &str) -> i64 {
    text.match_indices("used = ")
        .find_map(|(at, found)| {
            let (number, rest) = decimal(&text[at + found.len()..]);
            if number.is_empty() || !rest.starts_with('M') {
                return None;
            }
            float(number)
        })
        // Bytes as Python's `int()`, which drops the fraction.
        .map_or(0, |megabytes| (megabytes * 1024.0 * 1024.0) as i64)
}

/// What the memory tools printed, `None` for one that failed.
#[derive(Default)]
pub(crate) struct MemoryText {
    pub(crate) memsize: Option<String>,
    pub(crate) vm_stat: Option<String>,
    pub(crate) swap: Option<String>,
    pub(crate) pressure: Option<String>,
}

/// `memory`: `{ used, total, active, wired, compressed, swap, pressure }`.
pub(crate) fn memory(text: &MemoryText) -> Value {
    let total = as_int(text.memsize.as_deref(), 0);
    let vm = text.vm_stat.as_deref().unwrap_or("");
    let page = page_size(vm);
    let counts = pages(vm);
    let bytes = |name: &str| counts.get(name).copied().unwrap_or(0).saturating_mul(page);
    let active = bytes("Pages active");
    let wired = bytes("Pages wired down");
    let compressed = bytes("Pages occupied by compressor");
    let anonymous = bytes("Anonymous pages");
    let purgeable = bytes("Pages purgeable");
    // As Activity Monitor counts it: app memory, wired, compressed.
    let mut used = anonymous - purgeable + wired + compressed;
    if total != 0 && used > total {
        used = total;
    }
    let swap = swap_used(text.swap.as_deref().unwrap_or(""));
    let level = as_int(text.pressure.as_deref(), 1);
    let pressure = if level >= 4 {
        "critical"
    } else if level >= 2 {
        "warning"
    } else {
        "normal"
    };
    json!({
        "used": used,
        "total": total,
        "active": active,
        "wired": wired,
        "compressed": compressed,
        "swap": swap,
        "pressure": pressure,
    })
}
