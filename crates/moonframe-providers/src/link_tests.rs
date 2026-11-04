//! Process and socket providers under load: how fast output arrives, what
//! a slow or absent program costs, and how they stop.

use {
    crate::{Event, Link, Providers},
    moonframe_core::Data,
    std::{
        fs,
        io::{BufRead, BufReader, Write},
        path::Path,
        sync::{Arc, Mutex, mpsc},
        time::{Duration, Instant},
    },
};

const LONG: Duration = Duration::from_secs(5);

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn events() -> (crate::Events, mpsc::Receiver<Event>) {
    let (sender, receiver) = mpsc::channel();
    let sender = Mutex::new(sender);
    (
        Arc::new(move |event| {
            let _ = sender.lock().unwrap().send(event);
        }),
        receiver,
    )
}

/// The next event that is a publish, with the time it took.
fn published(received: &mpsc::Receiver<Event>) -> (Data, Duration) {
    let started = Instant::now();
    loop {
        match received.recv_timeout(LONG).unwrap() {
            Event::Published { data, .. } => return (data, started.elapsed()),
            Event::Newest { newest, .. } => {
                return (newest.take().unwrap(), started.elapsed());
            },
            Event::Failed { problem, .. } => panic!("failed: {problem}"),
            _ => {},
        }
    }
}

fn link(received: &mpsc::Receiver<Event>, wanted: impl Fn(&Link) -> bool) {
    loop {
        match received.recv_timeout(LONG).unwrap() {
            Event::Link { link, .. } if wanted(&link) => return,
            Event::Failed { problem, .. } => panic!("failed: {problem}"),
            _ => {},
        }
    }
}

#[test]
fn output_is_published_as_it_is_printed_not_on_a_timer() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/quick/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'quick.sh']\n",
    );
    // The first line starts the clock, the second comes right after a
    // pause that ends between two of the old polls.
    write(
        root,
        "providers/quick/quick.sh",
        "echo 1\nsleep 0.15\necho 2\ncat >/dev/null\n",
    );
    let (events, received) = events();
    let _providers = Providers::start(root.into(), root.join("build"), events);
    published(&received);
    let (data, took) = published(&received);
    assert_eq!(data, Data::Integer(2));
    // The sleep, and nowhere near a further 100 ms poll on a loaded host.
    assert!(took < Duration::from_millis(240), "took {took:?}");
}

#[test]
fn a_latest_provider_leaves_the_app_only_the_newest() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/meter/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'meter.sh']\npublish = 'latest'\n",
    );
    write(
        root,
        "providers/meter/meter.sh",
        "i=0\nwhile [ $i -lt 500 ]; do i=$((i+1)); echo $i; done\ncat >/dev/null\n",
    );
    let (events, received) = events();
    let _providers = Providers::start(root.into(), root.join("build"), events);
    // Nothing takes the first value, so every later one replaces it
    // without a further event.
    std::thread::sleep(Duration::from_millis(400));
    let newest: Vec<crate::Newest> = received
        .try_iter()
        .filter_map(|event| match event {
            Event::Newest { newest, .. } => Some(newest),
            _ => None,
        })
        .collect();
    assert_eq!(newest.len(), 1);
    assert_eq!(newest[0].take(), Some(Data::Integer(500)));
    assert_eq!(newest[0].take(), None);
}

#[test]
fn dropped_messages_are_reported_and_a_stop_still_gets_through() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/stuck/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'stuck.sh']\nurgent = ['stop']\n",
    );
    // Reads nothing until it is told to by a file, with a full pipe.
    write(
        root,
        "providers/stuck/stuck.sh",
        "echo 1\nwhile [ ! -e go ]; do sleep 0.02; done\n\
         while read -r line; do case \"$line\" in *stop*) echo '\"stopped\"';; esac; done\n",
    );
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    published(&received);
    let big = "x".repeat(4096);
    for _ in 0..800 {
        providers
            .send("stuck", "ui", Data::from(big.as_str()))
            .unwrap();
    }
    link(&received, |link| link.dropped > 0);
    providers.send("stuck", "ui", Data::from("stop")).unwrap();
    fs::write(root.join("providers/stuck/go"), "").unwrap();
    let (data, _) = published(&received);
    assert_eq!(data, Data::from("stopped"));
}

#[test]
fn a_process_gets_its_grace_to_leave_when_the_app_quits() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/tidy/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'tidy.sh']\nstop-grace = 3.0\n",
    );
    // Takes its time once stdin closes; SIGKILL would lose the file.
    write(
        root,
        "providers/tidy/tidy.sh",
        "trap '' TERM\necho 1\ncat >/dev/null\nsleep 0.3\necho done > left\n",
    );
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    published(&received);
    providers.shutdown();
    assert!(root.join("providers/tidy/left").is_file());
}

#[test]
fn a_process_that_ignores_everything_is_killed_in_the_end() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/deaf/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'deaf.sh']\nstop-grace = 0.05\n",
    );
    write(
        root,
        "providers/deaf/deaf.sh",
        "trap '' TERM\necho 1\nwhile :; do sleep 0.05; done\n",
    );
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    published(&received);
    let started = Instant::now();
    providers.shutdown();
    assert!(started.elapsed() < Duration::from_secs(2));
}

/// The next problem, skipping the rest.
fn failed(received: &mpsc::Receiver<Event>) -> String {
    loop {
        if let Event::Failed { problem, .. } = received.recv_timeout(LONG).unwrap() {
            return problem.to_string();
        }
    }
}

/// The next publish, past any failures.
fn next_data(received: &mpsc::Receiver<Event>) -> Data {
    loop {
        if let Event::Published { data, .. } = received.recv_timeout(LONG).unwrap() {
            return data;
        }
    }
}

#[test]
fn a_process_that_goes_starts_again_when_it_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/flaky/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'flaky.sh']\nrestart = 'on-failure'\nstart = 'always'\n",
    );
    // Dies on its first run, stays up on its second.
    write(
        root,
        "providers/flaky/flaky.sh",
        "if [ -e ran ]; then echo 2; cat >/dev/null; else touch ran; exit 3; fi\n",
    );
    let (events, received) = events();
    let _providers = Providers::start(root.into(), root.join("build"), events);
    assert!(failed(&received).contains("exited"));
    assert_eq!(next_data(&received), Data::Integer(2));
}

#[test]
fn a_process_that_goes_stays_gone_until_asked() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/once/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'once.sh']\nstart = 'always'\n",
    );
    write(
        root,
        "providers/once/once.sh",
        "if [ -e ran ]; then echo 2; cat >/dev/null; else touch ran; exit 3; fi\n",
    );
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    failed(&received);
    // Longer than an `on-failure` provider's first wait.
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(800) {
        assert!(
            !matches!(
                received.recv_timeout(Duration::from_millis(100)),
                Ok(Event::Published { .. })
            ),
            "it started again by itself"
        );
    }
    providers.restart("once").unwrap();
    assert_eq!(next_data(&received), Data::Integer(2));
    assert!(providers.restart("nothing").is_err());
}

#[test]
fn a_process_says_when_it_is_there_and_when_it_went() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/brief/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'brief.sh']\nstart = 'always'\n",
    );
    write(root, "providers/brief/brief.sh", "sleep 0.2\nexit 1\n");
    let (events, received) = events();
    let _providers = Providers::start(root.into(), root.join("build"), events);
    link(&received, |link| link.connected == Some(true));
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < LONG, "never said it went");
        if let Event::Link { link, .. } = received.recv_timeout(LONG).unwrap()
            && link.connected == Some(false)
        {
            return;
        }
    }
}

#[test]
fn a_restart_waits_for_the_old_process_to_leave() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/slow/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'slow.sh']\nstop-grace = 3.0\nstart = 'always'\n",
    );
    // Holds `busy`, as an engine holds its scanner; a second one beside it
    // would find it and say so.
    write(
        root,
        "providers/slow/slow.sh",
        "if [ -e busy ]; then echo 0; else echo 1; fi\ntouch busy\ncat >/dev/null\nsleep 0.3\nrm busy\n",
    );
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    assert_eq!(next_data(&received), Data::Integer(1));
    providers.restart("slow").unwrap();
    assert_eq!(next_data(&received), Data::Integer(1));
}

#[cfg(unix)]
#[test]
fn stopping_reaches_what_the_process_started() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/wrapper/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'wrapper.sh']\nstop-grace = 0.05\n",
    );
    // A launcher and an engine that takes no notice of stdin closing.
    write(
        root,
        "providers/wrapper/wrapper.sh",
        "sleep 60 &\necho $! > engine\necho 1\nwait\n",
    );
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    published(&received);
    let engine = fs::read_to_string(root.join("providers/wrapper/engine")).unwrap();
    let engine: libc::pid_t = engine.trim().parse().unwrap();
    providers.shutdown();
    // SAFETY: `kill` with signal 0 only asks whether the process exists.
    #[allow(unsafe_code)]
    let alive = unsafe { libc::kill(engine, 0) } == 0;
    assert!(!alive, "the engine outlived its launcher");
}

#[cfg(unix)]
#[test]
fn a_process_that_closes_its_output_is_still_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/quiet/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'quiet.sh']\nstop-grace = 0.05\nstart = 'always'\n",
    );
    write(
        root,
        "providers/quiet/quiet.sh",
        "echo $$ > pid\nexec 1>&-\nwhile :; do sleep 0.05; done\n",
    );
    let (events, received) = events();
    let _providers = Providers::start(root.into(), root.join("build"), events);
    assert!(failed(&received).contains("closed its output"));
    let pid = fs::read_to_string(root.join("providers/quiet/pid")).unwrap();
    let pid: libc::pid_t = pid.trim().parse().unwrap();
    let started = Instant::now();
    // SAFETY: `kill` with signal 0 only asks whether the process exists.
    #[allow(unsafe_code)]
    while unsafe { libc::kill(pid, 0) } == 0 {
        assert!(started.elapsed() < LONG, "it was left running");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
mod socket {
    use {super::*, std::os::unix::net::UnixListener};

    fn provider(root: &Path, extra: &str) -> std::path::PathBuf {
        let path = root.join("engine.sock");
        write(
            root,
            "providers/engine/provider.toml",
            &format!(
                "kind = 'socket'\nsocket = '{}'\n{extra}",
                path.to_str().unwrap()
            ),
        );
        path
    }

    #[test]
    fn it_waits_for_the_program_says_hello_and_connects_again() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let path = provider(root, "");
        let (events, received) = events();
        let mut providers = Providers::start(root.into(), root.join("build"), events);
        link(&received, |link| link.connected == Some(false));

        // Sent with no one there: dropped and counted, not kept for later.
        providers.send("engine", "ui", Data::from("old")).unwrap();
        link(&received, |link| link.dropped == 1);

        let listener = UnixListener::bind(&path).unwrap();
        let (stream, _) = listener.accept().unwrap();
        link(&received, |link| link.connected == Some(true));
        let mut lines = BufReader::new(stream.try_clone().unwrap());
        let mut hello = String::new();
        lines.read_line(&mut hello).unwrap();
        assert_eq!(hello, "{\"from\":\"\",\"message\":{\"connected\":true}}\n");

        writeln!(&stream, "{{\"state\": \"idle\"}}").unwrap();
        let (data, _) = published(&received);
        assert!(matches!(data, Data::Map(_)));

        providers.send("engine", "ui", Data::from("start")).unwrap();
        let mut sent = String::new();
        lines.read_line(&mut sent).unwrap();
        assert_eq!(sent, "{\"from\":\"ui\",\"message\":\"start\"}\n");

        // The program restarts: the link drops and comes back by itself.
        drop(lines);
        drop(stream);
        link(&received, |link| link.connected == Some(false));
        let (_stream, _) = listener.accept().unwrap();
        link(&received, |link| link.connected == Some(true));
    }

    #[test]
    fn the_app_leaving_does_not_touch_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let path = provider(root, "");
        let listener = UnixListener::bind(&path).unwrap();
        let (events, received) = events();
        let mut providers = Providers::start(root.into(), root.join("build"), events);
        let (stream, _) = listener.accept().unwrap();
        link(&received, |link| link.connected == Some(true));
        let started = Instant::now();
        providers.shutdown();
        assert!(started.elapsed() < Duration::from_millis(100));
        // It sees the link end and goes on listening.
        let mut lines = BufReader::new(stream);
        let mut text = String::new();
        lines.read_line(&mut text).unwrap();
        text.clear();
        assert_eq!(lines.read_line(&mut text).unwrap(), 0);
    }
}

#[test]
fn a_manifest_says_what_goes_with_what() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let load = |text: &str| {
        write(root, "provider.toml", text);
        crate::Manifest::load("x", root)
    };
    let refused = |text: &str, part: &str| {
        let problem = load(text).unwrap_err().to_string();
        assert!(problem.contains(part), "{problem}");
    };
    refused("kind = 'socket'\n", "needs a `socket`");
    refused("kind = 'native'\nsocket = 'a'\n", "only a socket");
    refused("kind = 'native'\nurgent = ['stop']\n", "`urgent`");
    refused("kind = 'native'\nstop-grace = 1.0\n", "`stop-grace`");
    refused(
        "kind = 'process'\ncommand = ['a']\nstop-grace = 99.0\n",
        "0 to 30",
    );
    let manifest = load("kind = 'socket'\nsocket = '~/a'\nlatest = ['gain']\n").unwrap();
    assert_eq!(manifest.latest, ["gain"]);
}
