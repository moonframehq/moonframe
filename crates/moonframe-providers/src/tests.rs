use {
    crate::{Capability, Event, Kind, Manifest, Providers, check, json},
    moonframe_core::Data,
    std::{
        fs,
        path::Path,
        sync::{Arc, Mutex, mpsc},
        time::Duration,
    },
};

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn manifests_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/sys/provider.toml",
        "kind = 'wasm'\ninterval = 2\ncapabilities = ['system']\n[settings]\nhistory = 8\n",
    );
    write(
        root,
        "providers/sys/Cargo.toml",
        "[package]\nname = 'sys'\n",
    );
    let manifest = Manifest::load("sys", &root.join("providers/sys")).unwrap();
    assert_eq!(manifest.kind, Kind::Wasm);
    assert_eq!(manifest.interval, Duration::from_secs(2));
    assert_eq!(manifest.capabilities, [Capability::System]);
    assert_eq!(manifest.settings, serde_json::json!({ "history": 8 }));

    let cases = [
        (
            "a",
            "kind = 'wasm'",
            "needs a provider.wasm, or a Cargo.toml",
        ),
        ("b", "kind = 'process'", "needs a `command`"),
        (
            "c",
            "kind = 'process'\ncommand = ['x']\ncapabilities = ['system']",
            "capabilities are for wasm",
        ),
        ("d", "kind = 'plugin'", "unknown kind `plugin`"),
        (
            "e",
            "kind = 'process'\ncommand = ['x']\ninterval = 0",
            "`interval` is",
        ),
        (
            "f",
            "kind = 'process'\ncommand = ['x']\ncapabilities = ['network']",
            "unknown variant",
        ),
        (
            "g",
            "kind = 'process'\ncommand = ['x']\ncolour = 1",
            "unknown field",
        ),
        ("Bad", "kind = 'process'\ncommand = ['x']", "use a-z"),
        ("h", "kind = 'static'", "needs a `file`"),
        (
            "i",
            "kind = 'static'\nfile = '../x.json'",
            "a file name in the provider's folder",
        ),
        ("j", "kind = 'static'\nfile = 'gone.json'", "no gone.json"),
        #[cfg(unix)]
        ("l", "kind = 'static'\nfile = 'linked.json'", "is a link"),
        (
            "k",
            "kind = 'process'\ncommand = ['x']\nfile = 'x.json'",
            "only a static provider",
        ),
    ];
    for (name, toml, _) in cases {
        write(root, &format!("providers/{name}/provider.toml"), toml);
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.join("providers/sys/Cargo.toml"),
        root.join("providers/l/linked.json"),
    )
    .unwrap();
    let problems = check(&root.into());
    for (name, _, expected) in cases {
        let problem = problems
            .iter()
            .find(|(found, _)| found == name)
            .map(|(_, problem)| problem.to_string());
        assert!(
            problem
                .as_deref()
                .is_some_and(|problem| problem.contains(expected)),
            "{name}: {problem:?}"
        );
    }
    assert!(!problems.iter().any(|(name, _)| name == "sys"));
}

#[test]
fn json_crosses_both_ways_within_bounds() {
    let data =
        json::data(r#"{"cpu": 0.5, "history": [1, 2.5], "name": "x", "on": true, "none": null}"#)
            .unwrap();
    let Data::Map(fields) = &data else {
        panic!("not a map");
    };
    assert_eq!(fields["cpu"], Data::Number(0.5));
    assert_eq!(
        fields["history"],
        Data::List(vec![Data::Integer(1), Data::Number(2.5)])
    );
    assert_eq!(json::data(&json::json(&data).to_string()).unwrap(), data);
    assert!(json::data("{").is_err());
    assert!(json::data(&"[".repeat(40)).is_err());
    let deep = format!("{}1{}", "[".repeat(20), "]".repeat(20));
    assert!(
        json::data(&deep)
            .unwrap_err()
            .to_string()
            .contains("deeper")
    );
}

fn events() -> (crate::Events, mpsc::Receiver<Event>) {
    let (sender, receiver) = mpsc::channel();
    let sender = Mutex::new(sender);
    let events: crate::Events = Arc::new(move |event| {
        let _ = sender.lock().unwrap().send(event);
    });
    (events, receiver)
}

#[test]
fn a_process_publishes_lines_and_answers_messages() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/echo/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'echo.sh']\n",
    );
    write(
        root,
        "providers/echo/echo.sh",
        "echo '{\"ready\": true}'\nwhile read -r line; do echo \"{\\\"got\\\": $line}\"; done\n",
    );
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    assert_eq!(providers.names().collect::<Vec<_>>(), ["echo"]);
    let next = || received.recv_timeout(Duration::from_secs(5)).unwrap();
    let Event::Published { provider, data } = next() else {
        panic!("not published");
    };
    assert_eq!(provider, "echo");
    assert_eq!(
        data,
        Data::Map([("ready".to_owned(), Data::Bool(true))].into())
    );

    providers
        .send("echo", "explorer", Data::from("hi"))
        .unwrap();
    let Event::Published { data, .. } = next() else {
        panic!("not published");
    };
    let expected = Data::Map(
        [(
            "got".to_owned(),
            Data::Map(
                [
                    ("from".to_owned(), Data::from("explorer")),
                    ("message".to_owned(), Data::from("hi")),
                ]
                .into(),
            ),
        )]
        .into(),
    );
    assert_eq!(data, expected);
    assert!(providers.send("nobody", "explorer", Data::Nil).is_err());
}

#[test]
fn a_process_that_exits_or_prints_garbage_fails() {
    for (script, expected) in [("exit 3", "exited"), ("echo 'not json'", "invalid JSON")] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "providers/bad/provider.toml",
            "kind = 'process'\ncommand = ['sh', 'bad.sh']\n",
        );
        write(
            root,
            "providers/bad/bad.sh",
            &format!("{script}\nsleep 5\n"),
        );
        let (events, received) = events();
        let _providers = Providers::start(root.into(), root.join("build"), events);
        let event = received.recv_timeout(Duration::from_secs(5)).unwrap();
        let Event::Failed { problem, .. } = event else {
            panic!("{script}: did not fail");
        };
        assert!(
            problem.to_string().contains(expected),
            "{script}: {problem}"
        );
    }
}

#[test]
fn a_wasm_provider_with_a_build_hook_is_not_built() {
    for hook in ["build.rs", ".cargo/config.toml"] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "providers/sys/provider.toml", "kind = 'wasm'\n");
        write(
            root,
            "providers/sys/Cargo.toml",
            "[package]\nname = 'sys'\n",
        );
        write(root, &format!("providers/sys/{hook}"), "");
        let (events, received) = events();
        let _providers = Providers::start(root.into(), root.join("build"), events);
        let Event::Failed { problem, .. } = received.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("{hook}: built");
        };
        assert!(
            problem.to_string().contains("cannot have"),
            "{hook}: {problem}"
        );
    }
}

#[test]
fn a_wasm_provider_with_a_hook_elsewhere_is_not_built() {
    let package = "[package]\nname = 'sys'\n";
    for (case, manifest, files) in [
        (
            "package.build",
            "[package]\nname = 'sys'\nbuild = 'src/gen.rs'\n",
            &[][..],
        ),
        (
            "build-dependencies",
            "[package]\nname = 'sys'\n[build-dependencies]\ncc = '1'\n",
            &[],
        ),
        (
            "a target's build-dependencies",
            "[package]\nname = 'sys'\n[target.'cfg(unix)'.build-dependencies]\ncc = '1'\n",
            &[],
        ),
        (
            "a local crate's build script",
            "[package]\nname = 'sys'\n[dependencies]\nlocal = { path = 'local' }\n",
            &[("local/Cargo.toml", package), ("local/build.rs", "")][..],
        ),
        (
            "a patched-in crate's build script",
            "[package]\nname = 'sys'\n[patch.crates-io]\nserde = { path = 'local' }\n",
            &[("local/Cargo.toml", package), ("local/build.rs", "")][..],
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "providers/sys/provider.toml", "kind = 'wasm'\n");
        write(root, "providers/sys/Cargo.toml", manifest);
        for (path, text) in files {
            write(root, &format!("providers/sys/{path}"), text);
        }
        let (events, received) = events();
        let _providers = Providers::start(root.into(), root.join("build"), events);
        let Event::Failed { problem, .. } = received.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("{case}: built");
        };
        assert!(
            problem.to_string().contains("cannot have"),
            "{case}: {problem}"
        );
    }
}

#[test]
fn a_wasm_provider_cannot_name_its_toolchain_by_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "providers/sys/provider.toml", "kind = 'wasm'\n");
    write(
        root,
        "providers/sys/Cargo.toml",
        "[package]\nname = 'sys'\n",
    );
    write(
        root,
        "providers/sys/rust-toolchain.toml",
        "[toolchain]\npath = './tc'\n",
    );
    let (events, received) = events();
    let _providers = Providers::start(root.into(), root.join("build"), events);
    let Event::Failed { problem, .. } = received.recv_timeout(Duration::from_secs(5)).unwrap()
    else {
        panic!("built");
    };
    assert!(problem.to_string().contains("by path"), "{problem}");
}

/// A process that never reads its messages holds up nothing: what it
/// prints is still published, however many messages it is sent.
#[test]
fn a_process_that_never_reads_its_messages_still_publishes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/deaf/provider.toml",
        "kind = 'process'\ncommand = ['sh', 'deaf.sh']\n",
    );
    write(
        root,
        "providers/deaf/deaf.sh",
        "echo '{\"n\": 1}'\nsleep 2\necho '{\"n\": 2}'\nsleep 30\n",
    );
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    let next = || received.recv_timeout(Duration::from_secs(10)).unwrap();
    let Event::Published { .. } = next() else {
        panic!("not published");
    };
    // Far more than a pipe holds: writing it all at once would block.
    let big = Data::from("x".repeat(4096));
    for _ in 0..2_000 {
        providers.send("deaf", "explorer", big.clone()).unwrap();
    }
    let Event::Published { data, .. } = next() else {
        panic!("not published");
    };
    assert_eq!(data, Data::Map([("n".to_owned(), Data::Integer(2))].into()));
}

#[test]
fn log_lines_are_cut_and_the_rest_dropped() {
    let mut text = "a".repeat(crate::lines::MAX_LOG_LINE * 100);
    text.push_str("\nnext\r\nlast");
    let mut lines = Vec::new();
    crate::lines::log_lines(text.as_bytes(), |line| lines.push(line));
    assert_eq!(
        lines.len(),
        3,
        "{:?}",
        lines.iter().map(String::len).collect::<Vec<_>>()
    );
    assert_eq!(lines[0].len(), crate::lines::MAX_LOG_LINE);
    assert_eq!(lines[1..], ["next", "last"]);
}

#[test]
fn a_mod_list_counts_a_mods_components_but_not_its_instances_as_it() {
    let settings = Data::Map(
        [(
            "writers".to_owned(),
            Data::List(vec![Data::from("editor"), Data::Integer(3)]),
        )]
        .into(),
    );
    let writers = crate::Mods::from_settings(&settings, "writers");
    for from in ["editor", "editor/markdown-editor#main"] {
        assert!(writers.allows(from), "{from}");
    }
    // Any mod may mount `editor#2`, so it doesn't speak for `editor`.
    for from in [
        "editor#2",
        "editor#2/tabs#a",
        "editorx",
        "edit",
        "other/editor",
        "other#editor",
        "",
        "3",
    ] {
        assert!(!writers.allows(from), "{from}");
    }
    assert!(!crate::Mods::from_settings(&settings, "starters").allows("editor"));
    assert!(!crate::Mods::from_settings(&Data::Nil, "writers").allows("editor"));
}

#[test]
fn a_reload_restarts_only_providers_that_changed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // Each start prints a new number, so a restart shows as a new value.
    for name in ["a", "b"] {
        write(
            root,
            &format!("providers/{name}/provider.toml"),
            "kind = 'process'\ncommand = ['sh', 'p.sh']\n",
        );
        write(
            root,
            &format!("providers/{name}/p.sh"),
            "echo \"$$\"\nsleep 30\n",
        );
    }
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    let started = || {
        let Event::Published { provider, .. } =
            received.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("not published");
        };
        provider
    };
    let mut first = [started(), started()];
    first.sort();
    assert_eq!(first, ["a", "b"]);

    write(root, "providers/b/p.sh", "echo \"$$\"\nsleep 31\n");
    providers.reload();
    assert_eq!(started(), "b");
    assert!(
        received.recv_timeout(Duration::from_millis(300)).is_err(),
        "an unchanged provider restarted"
    );

    fs::remove_dir_all(root.join("providers/a")).unwrap();
    providers.reload();
    assert_eq!(providers.names().collect::<Vec<_>>(), ["b"]);
}

#[test]
fn a_higher_layer_replaces_a_provider_of_the_same_name() {
    let dir = tempfile::tempdir().unwrap();
    let (builtin, user) = (dir.path().join("app"), dir.path().join("user"));
    for (root, says) in [(&builtin, "app"), (&user, "user")] {
        write(
            root,
            "providers/p/provider.toml",
            "kind = 'process'\ncommand = ['sh', 'p.sh']\n",
        );
        write(
            root,
            "providers/p/p.sh",
            &format!("echo '\"{says}\"'\nsleep 30\n"),
        );
    }
    write(&builtin, "providers/q/provider.toml", "kind = 'nope'\n");
    let roots = moonframe_core::Roots::new([builtin, user]);
    assert_eq!(crate::names(&roots), ["p", "q"]);
    let (events, received) = events();
    let _providers = Providers::start(roots, dir.path().join("build"), events);
    let mut published = None;
    while let Ok(event) = received.recv_timeout(Duration::from_secs(5)) {
        if let Event::Published { data, .. } = event {
            published = Some(data);
            break;
        }
    }
    assert_eq!(published, Some(Data::from("user")));
}

/// Echoes each message back to the mod that sent it, and panics on "boom".
struct Echo;

impl crate::Native for Echo {
    fn start(&mut self, host: &crate::Handle) -> Result<(), String> {
        host.publish(Data::from("ready"));
        Ok(())
    }

    fn tick(&mut self, _: &crate::Handle) {}

    fn receive(&mut self, host: &crate::Handle, from: &str, message: Data) {
        assert!(message != Data::from("boom"), "asked to fail");
        host.send(from, message);
    }
}

fn echo() -> Box<dyn crate::Native> {
    Box::new(Echo)
}

#[test]
fn a_native_provider_publishes_and_answers_the_mod_that_asked() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/echo/provider.toml",
        "kind = 'native'\ninterval = 5\n",
    );
    write(root, "providers/gone/provider.toml", "kind = 'native'\n");
    let (events, received) = events();
    let factory: crate::native::Factory = Arc::new(echo);
    let natives = crate::Natives::from([("echo".to_owned(), factory)]);
    let mut providers = Providers::start_with(root.into(), root.join("build"), events, natives);
    let mut seen = Vec::new();
    let mut next = || {
        let event = received.recv_timeout(Duration::from_secs(5)).unwrap();
        seen.push(format!("{event:?}"));
        event
    };
    // Both start; their order is the threads'.
    let (mut ready, mut missing) = (false, false);
    while !(ready && missing) {
        match next() {
            Event::Published { provider, data } if provider == "echo" => {
                assert_eq!(data, Data::from("ready"));
                ready = true;
            },
            Event::Failed { provider, problem } if provider == "gone" => {
                assert!(
                    problem.to_string().contains("no native `gone`"),
                    "{problem}"
                );
                missing = true;
            },
            other => panic!("unexpected {other:?}"),
        }
    }

    providers.send("echo", "editor", Data::from("hi")).unwrap();
    let Event::Sent { provider, to, msg } = next() else {
        panic!("no answer: {seen:?}");
    };
    assert_eq!(
        (provider.as_str(), to.as_str(), msg),
        ("echo", "editor", Data::from("hi"))
    );

    // A panic stops that provider with a reason; the test, like the app, goes on.
    providers
        .send("echo", "editor", Data::from("boom"))
        .unwrap();
    let Event::Failed { provider, problem } = next() else {
        panic!("no failure: {seen:?}");
    };
    assert_eq!(provider, "echo");
    assert!(problem.to_string().contains("asked to fail"), "{problem}");
}

#[test]
fn a_native_manifest_takes_no_command_or_capabilities() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "a/provider.toml",
        "kind = 'native'\ncommand = ['x']\n",
    );
    write(
        root,
        "b/provider.toml",
        "kind = 'native'\ncapabilities = ['system']\n",
    );
    write(
        root,
        "c/provider.toml",
        "kind = 'native'\n[settings]\nlimit = 3\n",
    );
    assert!(Manifest::load("a", &root.join("a")).is_err());
    assert!(Manifest::load("b", &root.join("b")).is_err());
    let manifest = Manifest::load("c", &root.join("c")).unwrap();
    assert_eq!(manifest.kind, Kind::Native);
    assert_eq!(manifest.settings["limit"], 3);
}

#[test]
fn providers_start_once_wanted_unless_they_always_start() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for name in ["echo", "github", "gitlab", "eager"] {
        let start = if name == "eager" {
            "start = 'always'\n"
        } else {
            ""
        };
        write(
            root,
            &format!("providers/{name}/provider.toml"),
            &format!("kind = 'native'\ninterval = 5\n{start}"),
        );
    }
    let (events, _received) = events();
    let natives: crate::Natives = ["echo", "github", "gitlab", "eager"]
        .into_iter()
        .map(|name| {
            let factory: crate::native::Factory = Arc::new(echo);
            (name.to_owned(), factory)
        })
        .collect();
    let mut providers = Providers::start_wanting(
        root.into(),
        root.join("build"),
        events,
        natives,
        Some(std::collections::BTreeSet::new()),
    );
    let running = |providers: &Providers| {
        ["echo", "github", "gitlab", "eager"]
            .into_iter()
            .filter(|name| providers.running(name))
            .collect::<Vec<_>>()
    };
    assert_eq!(running(&providers), ["eager"]);

    providers.want(Some(["git*".to_owned()].into()));
    assert_eq!(running(&providers), ["github", "gitlab", "eager"]);

    // A message starts the one it is for.
    providers.send("echo", "editor", Data::from("hi")).unwrap();
    assert_eq!(running(&providers), ["echo", "github", "gitlab", "eager"]);

    // Wanting less stops nothing.
    providers.want(Some(std::collections::BTreeSet::new()));
    assert_eq!(running(&providers).len(), 4);
}

#[test]
fn a_static_provider_publishes_its_file_on_messages_and_changes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "providers/sample/provider.toml",
        "kind = 'static'\nfile = 'sample.json'\n",
    );
    write(root, "providers/sample/sample.json", r#"{"n": 1}"#);
    let (events, received) = events();
    let mut providers = Providers::start(root.into(), root.join("build"), events);
    let next = || match received.recv_timeout(Duration::from_secs(5)).unwrap() {
        Event::Published { provider, data } => (provider, data),
        other => panic!("{other:?}"),
    };
    let one = Data::map([("n", Data::Integer(1))]);
    assert_eq!(next(), ("sample".to_owned(), one.clone()));
    // A message publishes it again, so a mod's refresh hears back.
    providers
        .send("sample", "main", Data::from("refresh"))
        .unwrap();
    assert_eq!(next(), ("sample".to_owned(), one));

    write(root, "providers/sample/sample.json", r#"{"n": 22}"#);
    providers.reload();
    let two = Data::map([("n", Data::Integer(22))]);
    assert_eq!(next(), ("sample".to_owned(), two));

    let mut fails = |text: &str, expected: &str| {
        write(root, "providers/sample/sample.json", text);
        providers.reload();
        match received.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::Failed { provider, problem } => {
                assert_eq!(provider, "sample");
                assert!(problem.to_string().contains(expected), "{problem}");
            },
            other => panic!("{other:?}"),
        }
    };
    fails("{", "sample.json");
    // Refused after reading one byte past the limit, not the whole file.
    fails(
        &format!("\"{}\"", "x".repeat(2 * json::MAX_JSON)),
        "more than",
    );
}

#[cfg(unix)]
#[test]
fn a_static_file_swapped_for_a_link_is_refused_at_read() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "own.json", "{}");
    write(root, "outside.json", r#"{"secret": 1}"#);
    assert_eq!(crate::fixed::read(&root.join("own.json")).unwrap(), "{}");
    std::os::unix::fs::symlink(root.join("outside.json"), root.join("linked.json")).unwrap();
    assert!(crate::fixed::read(&root.join("linked.json")).is_err());
}
