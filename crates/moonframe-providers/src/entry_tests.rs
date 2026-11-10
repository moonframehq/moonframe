use {
    crate::{Event, Handle, Native, ProviderOptions, Providers},
    moonframe_core::{
        Data,
        entries::{BuildPolicy, Entries, EntryState, RootPolicy},
    },
    std::{
        fs,
        sync::{Arc, mpsc},
        time::Duration,
    },
};

#[test]
fn runtime_entries_add_remove_and_bind_a_namespaced_native() {
    struct Echo;
    impl Native for Echo {
        fn start(&mut self, host: &Handle) -> Result<(), String> {
            assert_eq!(host.name(), "system:notes");
            host.publish(Data::from("started"));
            Ok(())
        }

        fn tick(&mut self, _: &Handle) {}

        fn receive(&mut self, host: &Handle, _: &str, value: Data) {
            host.publish(value);
        }
    }
    let root = tempfile::tempdir().unwrap();
    let entry = root.path().join("notes");
    fs::create_dir_all(entry.join("provider")).unwrap();
    fs::write(
        entry.join("provider/provider.toml"),
        "kind = 'native'\ninterval = 60",
    )
    .unwrap();
    let entries = Entries::default();
    entries
        .add_root("system", root.path(), RootPolicy::default())
        .unwrap();
    let (send, recv) = mpsc::channel();
    let mut natives = crate::Natives::new();
    natives.insert("system:notes".into(), Arc::new(|| Box::new(Echo)));
    let mut providers = Providers::start_with(
        root.path().into(),
        root.path().join("build"),
        Arc::new(move |e| {
            let _ = send.send(e);
        }),
        natives,
    );
    entries.add_entry("system", "notes", &entry).unwrap();
    providers.set_roots(entries.apply(root.path().into()));
    assert!(
        matches!(recv.recv_timeout(Duration::from_secs(5)).unwrap(), Event::Published { data: Data::String(text), .. } if text == "started")
    );
    providers
        .send("system:notes", "user:notes", Data::from("roundtrip"))
        .unwrap();
    assert!(
        matches!(recv.recv_timeout(Duration::from_secs(5)).unwrap(), Event::Published { data: Data::String(text), .. } if text == "roundtrip")
    );
    entries.remove_entry("system", "notes");
    providers.set_roots(entries.apply(root.path().into()));
    assert!(!providers.running("system:notes"));
    assert!(providers.send("system:notes", "shell", Data::Nil).is_err());
}

#[test]
fn root_policy_refuses_crates_and_disabled_processes_and_sockets() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("entry/provider");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("Cargo.toml"), "[package]\nname = 'must-not-build'").unwrap();
    // Even shipping a prebuilt next to the crate does not bypass the policy.
    fs::write(dir.join("provider.wasm"), "not a component").unwrap();
    let entries = Entries::default();
    entries
        .add_root("user", root.path(), RootPolicy::default())
        .unwrap();
    entries
        .add_entry("user", "entry", dir.parent().unwrap())
        .unwrap();
    for (manifest, expected) in [
        ("kind = 'wasm'", "PrebuiltOnly refuses crates"),
        (
            "kind = 'process'\ncommand = ['never-run']",
            "process providers are disabled",
        ),
        (
            "kind = 'socket'\nsocket = '/never-connect'",
            "socket providers are disabled",
        ),
    ] {
        fs::write(dir.join("provider.toml"), manifest).unwrap();
        let problems = crate::check(&entries.apply(root.path().into()));
        assert!(
            problems
                .iter()
                .any(|(name, error)| name == "user:entry" && error.to_string().contains(expected)),
            "{problems:?}"
        );
    }
    entries
        .add_root("user", root.path(), RootPolicy {
            build: BuildPolicy::Cargo,
            allow_process: true,
            allow_socket: true,
        })
        .unwrap();
    assert!(crate::check(&entries.apply(root.path().into())).is_empty());
}

#[test]
fn policy_errors_report_failed_without_running_anything() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("entry/provider");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("provider.toml"), "kind = 'wasm'").unwrap();
    fs::write(dir.join("Cargo.toml"), "[package]\nname = 'test'").unwrap();
    let entries = Entries::default();
    entries
        .add_root("user", root.path(), RootPolicy::default())
        .unwrap();
    entries
        .add_entry("user", "entry", dir.parent().unwrap())
        .unwrap();
    let (send, recv) = mpsc::channel();
    let providers = Providers::start_configured(
        entries.apply(root.path().into()),
        root.path().join("build"),
        Arc::new(|_| {}),
        Default::default(),
        None,
        ProviderOptions {
            on_entry_status: Some(Arc::new(move |s| {
                let _ = send.send(s);
            })),
            ..Default::default()
        },
    );
    let status = recv.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(status.name, "user:entry");
    assert_eq!(status.state, EntryState::Failed);
    assert!(status.error.unwrap().contains("PrebuiltOnly"));
    assert!(!providers.running("user:entry"));
}

#[test]
fn changing_root_policy_revokes_an_always_started_process() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("entry/provider");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("provider.toml"),
        "kind = 'process'\nstart = 'always'\ncommand = ['never-run']",
    )
    .unwrap();
    let entries = Entries::default();
    entries
        .add_root("user", root.path(), RootPolicy {
            allow_process: true,
            ..Default::default()
        })
        .unwrap();
    entries
        .add_entry("user", "entry", dir.parent().unwrap())
        .unwrap();
    let mut providers = Providers::start_configured(
        entries.apply(root.path().into()),
        root.path().join("build"),
        Arc::new(|_| {}),
        Default::default(),
        Some(Default::default()),
        Default::default(),
    );
    assert!(providers.running("user:entry"));
    entries
        .add_root("user", root.path(), RootPolicy::default())
        .unwrap();
    providers.set_roots(entries.apply(root.path().into()));
    assert!(!providers.running("user:entry"));
}

#[test]
fn a_bad_manifest_after_failed_start_reports_failed_not_stale() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("entry/provider");
    fs::create_dir_all(&dir).unwrap();
    // No compiled binding: this provider cannot become a working fallback.
    fs::write(dir.join("provider.toml"), "kind = 'native'").unwrap();
    let entries = Entries::default();
    entries
        .add_root("user", root.path(), RootPolicy::default())
        .unwrap();
    entries
        .add_entry("user", "entry", dir.parent().unwrap())
        .unwrap();
    let (send, recv) = mpsc::channel();
    let mut providers = Providers::start_configured(
        entries.apply(root.path().into()),
        root.path().join("build"),
        Arc::new(|_| {}),
        Default::default(),
        None,
        ProviderOptions {
            on_entry_status: Some(Arc::new(move |status| {
                let _ = send.send(status);
            })),
            ..Default::default()
        },
    );
    assert_eq!(
        recv.recv_timeout(Duration::from_secs(5)).unwrap().state,
        EntryState::Failed
    );
    fs::write(dir.join("provider.toml"), "kind = 'unknown'").unwrap();
    providers.reload();
    let status = recv.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(status.state, EntryState::Failed);
    assert!(status.error.is_some());
    providers.shutdown();
}

#[test]
fn check_entry_validates_the_manifest_capabilities_and_policy() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path();
    fs::write(
        entry.join("main.luau"),
        "return mod { view = function() return ui.text 'ok' end }",
    )
    .unwrap();
    fs::create_dir_all(entry.join("provider")).unwrap();
    let manifest = entry.join("provider/provider.toml");
    let policy = RootPolicy::default();

    fs::write(
        &manifest,
        "kind = \"wasm\"\ncapabilities = [\"hub.entities\"]\n",
    )
    .unwrap();
    let missing = crate::check_entry(entry, &policy, &with(&["hub.entities"]));
    assert!(missing[0].message.contains("provider.wasm"), "{missing:?}");
    fs::write(entry.join("provider").join(crate::PREBUILT), b"\0asm").unwrap();
    let ok = crate::check_entry(entry, &policy, &with(&["hub.entities"]));
    assert!(ok.is_empty(), "{ok:?}");
    let unknown = crate::check_entry(entry, &policy, &with(&[]));
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].file, manifest);
    assert!(unknown[0].message.contains("hub.entities"));

    fs::write(&manifest, "kind = \"process\"\ncommand = [\"true\"]\n").unwrap();
    let refused = crate::check_entry(entry, &policy, &with(&[]));
    assert!(
        refused[0]
            .message
            .contains("process providers are disabled")
    );
    let allowed = RootPolicy {
        allow_process: true,
        ..RootPolicy::default()
    };
    assert!(crate::check_entry(entry, &allowed, &with(&[])).is_empty());
}

fn with(capabilities: &[&str]) -> moonframe_core::entries::CheckOptions {
    moonframe_core::entries::CheckOptions {
        capabilities: capabilities.iter().map(|name| (*name).to_owned()).collect(),
        ..Default::default()
    }
}

#[test]
fn check_entry_draws_with_the_shipped_sample() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path();
    fs::write(
        entry.join("main.luau"),
        r#"return mod { view = function(_, app)
    local data = app["user:fuubar"]
    if not data then return ui.text "waiting" end
    return ui.column { ui.row { padding = 8, ui.text(data.title) } }
end }"#,
    )
    .unwrap();
    let options = moonframe_core::entries::CheckOptions {
        name: Some("user:fuubar".to_owned()),
        ..Default::default()
    };
    let policy = RootPolicy::default();
    assert!(crate::check_entry(entry, &policy, &options).is_empty());
    fs::create_dir_all(entry.join("provider")).unwrap();
    fs::write(entry.join("provider/sample.json"), r#"{"title": "hi"}"#).unwrap();
    let found = crate::check_entry(entry, &policy, &options);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].line, Some(4));
    assert!(found[0].message.contains("padding"));
    fs::write(entry.join("provider/sample.json"), "{ not json").unwrap();
    let found = crate::check_entry(entry, &policy, &options);
    assert!(found[0].file.ends_with("sample.json"), "{found:?}");
}
