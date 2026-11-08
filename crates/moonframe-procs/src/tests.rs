//! The readers on what the tools print, the monitor's grouping, and the
//! provider driven as Lantern's monitor mod drives it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use {
    super::*,
    moonframe_providers::Event,
    std::{path::Path, sync::Mutex},
};

fn proc(pid: i64, cpu: f64, mem: i64, name: &str) -> Proc {
    Proc {
        pid,
        cpu,
        mem,
        name: name.to_owned(),
    }
}

#[test]
fn iostat_lines_give_readings_and_headers_none() {
    assert_eq!(read::iostat("    cpu    load average"), None);
    assert_eq!(read::iostat(" us sy id   1m   5m   15m"), None);
    assert_eq!(
        read::iostat("  7  3 90  2.31 2.05 1.98\n"),
        Some(Reading {
            user: 7,
            system: 3,
            idle: 90,
            load: [2.31, 2.05, 1.98],
        })
    );
    // Only the start has to match; `1.2.3` is no number.
    assert!(read::iostat("1 2 97 3 4 5 extra").is_some());
    assert_eq!(read::iostat("1 2 97 1.2.3 4 5"), None);
    assert_eq!(read::iostat("1 2 97 3 4"), None);
}

#[test]
fn ps_lines_give_processes_named_by_their_last_part() {
    let text = "    1   0.0  13024 /sbin/launchd\n\
                 412  12.5 204800 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome Helper  \n\
                  77   1.0      8 zsh\n\
                 not a process\n\
                  78   1.0      8 /ends/in/slash/\n\
                  79   1.0      8\n\
                  80  1.2.3     8 bad\n";
    assert_eq!(read::processes(text), vec![
        proc(1, 0.0, 13024 * 1024, "launchd"),
        proc(412, 12.5, 204800 * 1024, "Google Chrome Helper"),
        proc(77, 1.0, 8 * 1024, "zsh"),
    ]);
}

const VM_STAT: &str = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
Pages free:                               12345.\n\
Pages active:                            100.\n\
Pages wired down:                          50.\n\
Pages purgeable:                           10.\n\
Pages occupied by compressor:              20.\n\
Anonymous pages:                          200.\n\
\"Translation faults\":                  99.\n";

#[test]
fn memory_is_counted_as_activity_monitor_does() {
    let page = 16384;
    let memory = read::memory(&MemoryText {
        memsize: Some("17179869184\n".into()),
        vm_stat: Some(VM_STAT.into()),
        swap: Some("total = 2048.00M  used = 1.50M  free = 2046.50M  (encrypted)\n".into()),
        pressure: Some("2\n".into()),
    });
    assert_eq!(
        memory,
        json!({
            "used": (200 - 10 + 50 + 20) * page,
            "total": 17179869184_i64,
            "active": 100 * page,
            "wired": 50 * page,
            "compressed": 20 * page,
            "swap": 1572864,
            "pressure": "warning",
        })
    );
}

#[test]
fn memory_without_its_tools_is_zero_and_normal() {
    assert_eq!(
        read::memory(&MemoryText::default()),
        json!({ "used": 0, "total": 0, "active": 0, "wired": 0, "compressed": 0, "swap": 0, "pressure": "normal" })
    );
    // Used is never more than all there is; a page size of 0 means 16 KiB.
    let memory = read::memory(&MemoryText {
        memsize: Some("1000".into()),
        vm_stat: Some("(page size of 0 bytes)\nAnonymous pages: 1.\n".into()),
        swap: None,
        pressure: Some("4".into()),
    });
    assert_eq!(memory["used"], 1000);
    assert_eq!(memory["pressure"], "critical");
}

fn procs() -> Vec<Proc> {
    vec![
        proc(10, 1.25, 300, "Safari"),
        proc(11, 30.0, 100, "Chrome"),
        proc(12, 5.0, 900, "Chrome"),
        proc(13, 0.0, 50, "zsh"),
        proc(14, 0.0, 50, "bash"),
    ]
}

#[test]
fn groups_are_the_busiest_first_then_by_name() {
    let monitor = Monitor::default();
    let shown = monitor.snapshot(&procs(), json!({}));
    let names: Vec<&str> = shown["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Chrome", "Safari", "bash", "zsh"]);
    assert_eq!(
        shown["groups"][0],
        json!({ "name": "Chrome", "pid": 11, "count": 2, "cpu": 35, "mem": 1000, "open": false })
    );
    // Halves round up, to one decimal.
    assert_eq!(shown["groups"][1]["cpu"], json!(1.3));
    assert_eq!(shown["total"], 5);
    assert_eq!(shown["sort"], "cpu");
    assert_eq!(shown["query"], "");
    assert_eq!(shown["load"], json!([0, 0, 0]));
}

#[test]
fn messages_sort_filter_and_open_groups() {
    let mut monitor = Monitor::default();
    monitor.receive(&json!({ "sort": "memory", "expand": "Chrome" }));
    let shown = monitor.snapshot(&procs(), json!({}));
    assert_eq!(shown["sort"], "memory");
    let chrome = &shown["groups"][0];
    assert_eq!(chrome["pid"], 12, "the largest member, sorting by memory");
    assert_eq!(chrome["open"], true);
    assert_eq!(
        chrome["children"],
        json!([
            { "pid": 12, "name": "Chrome", "cpu": 5, "mem": 900 },
            { "pid": 11, "name": "Chrome", "cpu": 30, "mem": 100 },
        ])
    );
    // A group of one never opens; a second expand closes.
    monitor.receive(&json!({ "expand": "Safari", "sort": "bogus" }));
    let shown = monitor.snapshot(&procs(), json!({}));
    assert_eq!(shown["sort"], "memory");
    assert_eq!(shown["groups"][1]["open"], false);
    monitor.receive(&json!({ "expand": "Chrome" }));
    assert!(
        monitor.snapshot(&procs(), json!({}))["groups"][0]
            .get("children")
            .is_none()
    );

    monitor.receive(&json!({ "query": "SAF" }));
    let shown = monitor.snapshot(&procs(), json!({}));
    assert_eq!(shown["total"], 1);
    assert_eq!(shown["query"], "SAF");
    // A number matches pids, and is kept a number.
    monitor.receive(&json!({ "query": 12.0 }));
    let shown = monitor.snapshot(&procs(), json!({}));
    assert_eq!(shown["query"], json!(12));
    assert_eq!(shown["total"], 1);
    assert_eq!(shown["groups"][0]["pid"], 12);
}

#[test]
fn messages_of_other_shapes_change_nothing() {
    let mut monitor = Monitor::default();
    // An empty Luau table comes as `{}`; bools and tables are no query.
    for message in [
        json!({}),
        json!([]),
        json!("cpu"),
        json!({ "query": true, "expand": false }),
        json!({ "query": {} }),
    ] {
        monitor.receive(&message);
    }
    let shown = monitor.snapshot(&procs(), json!({}));
    assert_eq!(shown["query"], "");
    assert_eq!(shown["total"], 5);
    // A number names a group by its text.
    monitor.receive(&json!({ "expand": 7.0 }));
    let numbered = [proc(1, 0.0, 1, "7"), proc(2, 0.0, 1, "7")];
    assert_eq!(
        monitor.snapshot(&numbered, json!({}))["groups"][0]["open"],
        true
    );
}

#[test]
fn readings_fill_the_cpu_and_keep_the_last_sixty() {
    let mut monitor = Monitor::default();
    for at in 0..70 {
        monitor.read(Reading {
            user: 5,
            system: 2,
            idle: 100 - at % 100,
            load: [1.5, 2.0, 0.25],
        });
    }
    let shown = monitor.snapshot(&[], json!({}));
    let history = shown["history"].as_array().unwrap();
    assert_eq!(history.len(), 60);
    assert_eq!(history[0], 10);
    assert_eq!(history[59], 69);
    assert_eq!(shown["cpu"], 69);
    assert_eq!(shown["user"], 5);
    assert_eq!(shown["system"], 2);
    assert_eq!(shown["load"], json!([1.5, 2, 0.25]));
    assert_eq!(shown["groups"], json!([]));
    assert_eq!(shown["total"], 0);
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

/// Waits for the `count`th publication.
#[cfg(unix)]
fn nth(seen: &Mutex<Vec<Value>>, count: usize) -> Value {
    let started = Instant::now();
    loop {
        if let Some(value) = seen.lock().unwrap().get(count - 1) {
            return value.clone();
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no publication {count}"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn looked() -> Look {
    Box::new(|| (procs(), json!({ "pressure": "normal" })))
}

#[test]
fn without_iostat_it_says_so_once() {
    let (host, seen) = host();
    let mut provider = Provider::new(&["moonframe-no-such-iostat"], looked());
    provider.start(&host).unwrap();
    provider.receive(&host, "monitor", Data::from("anything"));
    provider.tick(&host);
    drop(provider);
    assert_eq!(*seen.lock().unwrap(), [
        json!({ "available": false, "reason": "iostat is not available" })
    ]);
}

#[cfg(unix)]
#[test]
fn it_publishes_each_reading_after_the_first_and_each_message() {
    let (host, seen) = host();
    // As iostat prints: headers, the since-boot line, then one per tick.
    let script = "printf '    cpu    load average\\n us sy id   1m   5m   15m\\n 9 9 1 9 9 9\\n'; \
                  sleep 0.2; printf '  6  4 90  1.50 1.25 1.00\\n'; sleep 30";
    let mut provider = Provider::new(&["sh", "-c", script], looked());
    provider.start(&host).unwrap();
    let first = nth(&seen, 1);
    assert_eq!(first["available"], true);
    assert_eq!(first["cpu"], 0);
    assert_eq!(first["history"], json!([]));
    assert_eq!(first["memory"], json!({ "pressure": "normal" }));
    let read = nth(&seen, 2);
    assert_eq!(read["cpu"], 10);
    assert_eq!(read["history"], json!([10]));
    assert_eq!(read["load"], json!([1.5, 1.25, 1]));

    provider.receive(&host, "monitor", json_data(json!({ "query": "chr" })));
    let asked = nth(&seen, 3);
    assert_eq!(asked["query"], "chr");
    assert_eq!(asked["total"], 2);
    assert_eq!(
        asked["history"],
        json!([10]),
        "a message keeps the readings"
    );

    let started = Instant::now();
    drop(provider);
    // The script's `sleep` keeps the pipe open: the worker must not wait
    // for it, only for its flag.
    assert!(
        started.elapsed() < Duration::from_millis(600),
        "stopping waited for the worker"
    );
    let count = seen.lock().unwrap().len();
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        seen.lock().unwrap().len(),
        count,
        "published after stopping"
    );
}

#[cfg(unix)]
fn json_data(value: Value) -> Data {
    moonframe_providers::from_json(&value).unwrap()
}
