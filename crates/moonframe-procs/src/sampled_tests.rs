//! The readers for Linux and Windows, run on every platform, and the
//! provider driven from samples of the CPU times, as there.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use {
    super::*,
    moonframe_providers::Event,
    sample::Times,
    std::{path::Path, sync::Mutex},
};

const STAT: &str = "cpu  100 20 30 400 50 6 4 10 7 0\n\
cpu0 50 10 15 200 25 3 2 5 3 0\n\
intr 12345\n";

#[test]
fn proc_stat_counts_nice_as_user_and_waiting_as_idle() {
    assert_eq!(
        linux::times(STAT),
        Some(Times {
            user: 120,
            system: 40,
            idle: 450,
            total: 620,
        })
    );
    // An old kernel's four fields are enough; fewer, or words, are not.
    assert_eq!(
        linux::times("cpu 1 2 3 4\n"),
        Some(Times {
            user: 3,
            system: 3,
            idle: 4,
            total: 10,
        })
    );
    assert_eq!(linux::times("cpu 1 2 3\n"), None);
    assert_eq!(linux::times("cpu 1 2 x 4\n"), None);
    assert_eq!(linux::times("cpu0 1 2 3 4\n"), None, "only all CPUs");
}

#[test]
fn a_reading_splits_the_time_between_two_samples() {
    let before = Times {
        user: 100,
        system: 50,
        idle: 800,
        total: 1000,
    };
    let after = Times {
        user: 130,
        system: 60,
        idle: 855,
        total: 1100,
    };
    assert_eq!(
        sample::reading(before, (after, [0.5, 0.25, 1.0])),
        Some(Reading {
            user: 30,
            system: 10,
            idle: 55,
            load: [0.5, 0.25, 1.0],
        })
    );
    // Shares round to whole percents.
    let third = Times {
        user: 1,
        system: 0,
        idle: 2,
        total: 3,
    };
    let reading = sample::reading(Times::default(), (third, [0.0; 3])).unwrap();
    assert_eq!((reading.user, reading.idle), (33, 67));
    // No time passed, or counters went back: no reading, no panic.
    assert_eq!(sample::reading(before, (before, [0.0; 3])), None);
    assert_eq!(sample::reading(after, (before, [0.0; 3])), None);
}

const MEMINFO: &str = "MemTotal:       16000000 kB\n\
MemFree:         1000000 kB\n\
MemAvailable:    6000000 kB\n\
Active:          5000000 kB\n\
SwapTotal:       2000000 kB\n\
SwapFree:        1500000 kB\n\
Zswap:            300000 kB\n\
HugePages_Total:       0\n";

#[test]
fn proc_meminfo_counts_used_as_all_but_available() {
    let kib = 1024_i64;
    assert_eq!(
        linux::memory(MEMINFO),
        json!({
            "used": 10_000_000 * kib,
            "total": 16_000_000 * kib,
            "active": 5_000_000 * kib,
            "wired": 0,
            "compressed": 300_000 * kib,
            "swap": 500_000 * kib,
            "pressure": "normal",
        })
    );
    // An old kernel without MemAvailable: free memory is what is available.
    let old = linux::memory("MemTotal: 100 kB\nMemFree: 40 kB\n");
    assert_eq!(old["used"], 60 * kib);
    assert_eq!(old["compressed"], 0);
    assert_eq!(
        linux::memory(""),
        json!({ "used": 0, "total": 0, "active": 0, "wired": 0, "compressed": 0, "swap": 0, "pressure": "normal" })
    );
}

#[test]
fn a_name_the_kernel_cut_is_read_from_its_command() {
    // Fifteen bytes kept of `gnome-shell-calendar-server`.
    let cut = "gnome-shell-cal";
    assert_eq!(
        linux::name(cut, Some("/usr/libexec/gnome-shell-calendar-server")),
        "gnome-shell-calendar-server"
    );
    // One command line in one word, as some programs rewrite it.
    assert_eq!(
        linux::name(
            cut,
            Some("/usr/libexec/gnome-shell-calendar-server --flag /x")
        ),
        "gnome-shell-calendar-server"
    );
    // A short name is whole; a command that is no longer name stays out.
    assert_eq!(linux::name("bash", Some("/bin/bash-other")), "bash");
    assert_eq!(linux::name(cut, Some("sshd: someone@pts/0")), cut);
    assert_eq!(linux::name(cut, None), cut);
    // A folder of that name is not the file's.
    assert_eq!(
        linux::name(
            cut,
            Some("/opt/gnome-shell-calendar/bin/gnome-shell-calendar-server")
        ),
        "gnome-shell-calendar-server"
    );
    assert_eq!(linux::name(cut, Some("/opt/gnome-shell-cal-x/run")), cut);
}

#[test]
fn windows_times_take_the_idle_out_of_the_kernel_time() {
    assert_eq!(windows::times_of(70, 100, 50), Times {
        user: 50,
        system: 30,
        idle: 70,
        total: 150,
    });
}

#[test]
fn windows_memory_has_no_counts_it_does_not_keep() {
    assert_eq!(
        windows::memory(16 << 30, 6 << 30, 1 << 20),
        json!({
            "used": 6_i64 << 30,
            "total": 16_i64 << 30,
            "active": 0,
            "wired": 0,
            "compressed": 0,
            "swap": 1 << 20,
            "pressure": "normal",
        })
    );
    assert_eq!(windows::memory(10, 20, u64::MAX)["used"], 10);
    assert_eq!(windows::memory(10, 20, u64::MAX)["swap"], i64::MAX);
}

/// A handle whose publications land in the returned list.
fn host() -> (Handle, Arc<Mutex<Vec<Value>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let events: moonframe_providers::Events = {
        let seen = Arc::clone(&seen);
        Arc::new(move |event| match event {
            Event::Published { data, .. } => seen.lock().unwrap().push(to_json(&data)),
            other => panic!("unexpected {other:?}"),
        })
    };
    (
        Handle::new("procs", Path::new("."), Data::Nil, events),
        seen,
    )
}

/// Waits for a publication that `wanted` accepts, after the first `after`.
fn awaited(seen: &Mutex<Vec<Value>>, after: usize, wanted: impl Fn(&Value) -> bool) -> Value {
    let started = Instant::now();
    loop {
        if let Some(found) = seen.lock().unwrap().iter().skip(after).find(|v| wanted(v)) {
            return found.clone();
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no such publication"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn looked() -> Look {
    Box::new(|| {
        let procs = vec![
            Proc {
                pid: 7,
                cpu: 12.5,
                mem: 100,
                name: "chrome".into(),
            },
            Proc {
                pid: 8,
                cpu: 0.0,
                mem: 50,
                name: "bash".into(),
            },
        ];
        (procs, windows::memory(1000, 400, 0))
    })
}

/// Samples whose CPU is 25% user and 15% system, the load rising.
fn sampler() -> Sampler {
    let mut taken = 0_u64;
    Box::new(move || {
        taken += 1;
        let times = Times {
            user: 25 * taken,
            system: 15 * taken,
            idle: 60 * taken,
            total: 100 * taken,
        };
        Some((times, [taken as f64, 0.5, 0.25]))
    })
}

#[test]
fn sampled_it_publishes_each_reading_and_each_message() {
    let (host, seen) = host();
    let every = Duration::from_millis(20);
    let mut provider = Provider::sampled(sampler(), looked(), every);
    provider.start(&host).unwrap();
    let first = awaited(&seen, 0, |_| true);
    assert_eq!(first["available"], true);
    assert_eq!(first["groups"][0]["name"], "chrome");
    assert_eq!(first["memory"]["used"], 400);
    let read = awaited(&seen, 0, |shown| shown["cpu"] == 40);
    assert_eq!(
        (read["user"].clone(), read["system"].clone()),
        (json!(25), json!(15))
    );
    assert_eq!(read["load"][1], json!(0.5));
    assert!(
        read["load"][0].as_f64().unwrap() >= 2.0,
        "the first only primes"
    );

    let count = seen.lock().unwrap().len();
    provider.receive(&host, "monitor", Data::from("anything"));
    let message = moonframe_providers::from_json(&json!({ "query": "bas" })).unwrap();
    provider.receive(&host, "monitor", message);
    let asked = awaited(&seen, count, |shown| shown["query"] == "bas");
    assert_eq!(asked["total"], 1);
    assert_eq!(asked["cpu"], 40, "a message keeps the readings");

    let started = Instant::now();
    drop(provider);
    assert!(
        started.elapsed() < Duration::from_millis(600),
        "stopping waited for the worker"
    );
    let count = seen.lock().unwrap().len();
    thread::sleep(every * 5);
    assert_eq!(
        seen.lock().unwrap().len(),
        count,
        "published after stopping"
    );
}

#[test]
fn sampled_without_cpu_times_it_says_so_once() {
    let (host, seen) = host();
    let mut provider = Provider::sampled(Box::new(|| None), looked(), Duration::from_millis(20));
    provider.start(&host).unwrap();
    provider.receive(&host, "monitor", Data::from("anything"));
    provider.tick(&host);
    drop(provider);
    assert_eq!(*seen.lock().unwrap(), [
        json!({ "available": false, "reason": "the CPU cannot be read" })
    ]);
}

/// The app's own provider on this machine: real processes, memory, and a
/// CPU reading two seconds in.
#[cfg(any(target_os = "linux", windows))]
#[test]
fn the_system_provider_reads_this_machine() {
    let (host, seen) = host();
    let mut provider = native();
    provider.start(&host).unwrap();
    let first = awaited(&seen, 0, |_| true);
    assert_eq!(first["available"], true, "{first}");
    assert!(first["total"].as_u64().unwrap() > 0, "no processes");
    let own = std::process::id().to_string();
    let message = moonframe_providers::from_json(&json!({ "query": own })).unwrap();
    provider.receive(&host, "monitor", message);
    let mine = awaited(&seen, 1, |shown| shown["query"] == own);
    assert_eq!(mine["groups"][0]["pid"], std::process::id(), "{mine}");
    assert!(mine["groups"][0]["mem"].as_i64().unwrap() > 0);
    let memory = &mine["memory"];
    assert!(memory["total"].as_i64().unwrap() > 0, "{memory}");
    assert!(memory["used"].as_i64().unwrap() > 0, "{memory}");
    let read = awaited(&seen, 0, |shown| {
        shown["history"].as_array().is_some_and(|h| !h.is_empty())
    });
    let cpu = read["cpu"].as_i64().unwrap();
    assert!((0..=100).contains(&cpu), "{read}");
    assert!(read["user"].as_i64().unwrap() + read["system"].as_i64().unwrap() <= cpu + 1);
    drop(provider);
}
