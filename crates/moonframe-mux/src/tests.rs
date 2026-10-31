use {
    crate::{Tmux, tmux::session_name},
    moonframe_terminal::{Backend as _, Engine, Open, Screen, Session, Size},
    std::{
        process::Command,
        sync::Arc,
        time::{Duration, Instant},
    },
};

fn text(screen: &Screen) -> String {
    (0..screen.rows)
        .map(|row| {
            screen
                .row(row)
                .iter()
                .map(|cell| cell.ch)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn wait_for(session: &Session, needle: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if text(&session.screen()).contains(needle) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn keys_name_tmux_sessions() {
    assert_eq!(session_name("terminal/main"), "terminal+main");
    assert_eq!(session_name("my mod/a.b:c"), "my_mod+a_b_c");
}

/// A tmux terminal runs, and a second attach after the first let go shows
/// what the first printed: the session outlived its view.
#[test]
fn a_tmux_terminal_outlives_its_view() {
    let Some(tmux) = Tmux::find() else {
        eprintln!("tmux is not installed; skipped");
        return;
    };
    let socket = format!("moonframe-test-{}", std::process::id());
    let tmux = tmux.on_socket(&socket);
    let open = Open {
        key: "test/outlives".into(),
        cwd: None,
        size: Size::default(),
    };
    let engine = [Engine::Alacritty, Engine::Ghostty]
        .into_iter()
        .find(|engine| engine.available())
        .unwrap();

    let first = Session::start(
        tmux.open(&open).unwrap(),
        engine,
        Size::default(),
        Arc::new(|| {}),
    )
    .unwrap();
    first.write(b"echo moonframe-$((40 + 2))\r");
    let printed = wait_for(&first, "moonframe-42");
    drop(first);

    let second = Session::start(
        tmux.open(&open).unwrap(),
        engine,
        Size::default(),
        Arc::new(|| {}),
    )
    .unwrap();
    let kept = wait_for(&second, "moonframe-42");
    second.write(b"echo again-$((1 + 1))\r");
    let typed = wait_for(&second, "again-2");
    let screen = text(&second.screen());
    drop(second);

    let _ = Command::new("tmux")
        .args(["-L", &socket, "kill-server"])
        .status();
    assert!(printed, "the first view never showed its output");
    assert!(kept, "the second view lost the first's output:\n{screen}");
    assert!(typed, "the second view takes no keys:\n{screen}");
}

/// The same for herdr-core, on a core session of the test's own.
#[cfg(feature = "herdr-core")]
#[test]
fn a_herdr_core_terminal_outlives_its_view() {
    let session = format!("moonframe-test-{}", std::process::id());
    let Some(core) = crate::HerdrCore::connect_session(Some(&session)) else {
        eprintln!("herdr-core is not installed; skipped");
        return;
    };
    let open = Open {
        key: "test/outlives".into(),
        cwd: None,
        size: Size::default(),
    };
    let engine = [Engine::Alacritty, Engine::Ghostty]
        .into_iter()
        .find(|engine| engine.available())
        .unwrap();

    let first = Session::start(
        core.open(&open).unwrap(),
        engine,
        Size::default(),
        Arc::new(|| {}),
    )
    .unwrap();
    first.write(b"echo moonframe-$((40 + 2))\r");
    let printed = wait_for(&first, "moonframe-42");
    drop(first);

    let second = Session::start(
        core.open(&open).unwrap(),
        engine,
        Size::default(),
        Arc::new(|| {}),
    )
    .unwrap();
    let kept = wait_for(&second, "moonframe-42");
    second.write(b"echo again-$((1 + 1))\r");
    let typed = wait_for(&second, "again-2");
    let screen = text(&second.screen());
    drop(second);

    stop_core(&session, &["test.outlives"]);
    assert!(printed, "the first view never showed its output");
    assert!(kept, "the second view lost the first's output:\n{screen}");
    assert!(typed, "the second view takes no keys:\n{screen}");
}

/// Ends `terminals` and the core of test session `session`, and removes
/// its state.
#[cfg(feature = "herdr-core")]
fn stop_core(session: &str, terminals: &[&str]) {
    if let Some(binary) = crate::herdr_core::installed() {
        let kills = terminals.iter().map(|terminal| vec!["kill", terminal]);
        for args in kills.chain([vec!["server", "stop"]]) {
            let _ = Command::new(&binary)
                .args(args)
                .env("HC_SESSION", session)
                .output();
        }
    }
    let endpoint = herdr_core_client::Endpoint::resolve(|name| match name {
        "HC_SESSION" => Some(session.into()),
        _ => std::env::var_os(name),
    });
    if let Ok(endpoint) = endpoint
        && let Some(state) = endpoint.socket.parent()
        && state.ends_with(session)
    {
        let _ = std::fs::remove_dir_all(state);
    }
}

/// The document round-trips through herdr-core, and a store opened later
/// writes over what an earlier one saved.
#[cfg(feature = "herdr-core")]
#[test]
fn herdr_core_keeps_the_document() {
    use {
        crate::{CoreStore, Document, Store as _},
        serde_json::json,
    };
    let session = format!("moonframe-doc-{}", std::process::id());
    let Some(core) = crate::HerdrCore::connect_session(Some(&session)) else {
        eprintln!("herdr-core is not installed; skipped");
        return;
    };
    let core = Arc::new(core);
    let first = CoreStore::new(core.clone(), "moonframe");
    let loaded = first.load();
    let mut document = Document::default();
    document
        .mods
        .insert("shell".into(), json!({ "left": false }));
    let saved = first.save(&document);

    let second = CoreStore::new(core, "moonframe");
    let reloaded = second.load();
    document
        .mods
        .insert("shell".into(), json!({ "left": true }));
    let resaved = second.save(&document).and_then(|()| second.load());

    stop_core(&session, &[]);
    assert_eq!(loaded, Ok(None));
    assert_eq!(saved, Ok(()));
    assert_eq!(
        reloaded.unwrap().unwrap().mods["shell"],
        json!({ "left": false })
    );
    assert_eq!(resaved, Ok(Some(document)));
}
