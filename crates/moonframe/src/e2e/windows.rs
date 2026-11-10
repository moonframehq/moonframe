//! More than one main window: each has its own mods, and they share the
//! app's providers.

use {
    super::folder,
    crate::{
        app::{Hub, Launch, MoonframeApp, main_windows},
        backends::Backends,
        hosted::Registry,
        open_main_window,
    },
    gpui_kit::{Entity, TestAppContext},
    moonframe_core::{Data, Workspace},
    moonframe_packages::paths::Folders,
    moonframe_providers::{Handle, Native, Natives, native::Factory},
    std::{sync::Arc, time::Duration},
};

/// A shell that asks the provider `echo` on `ask` and `publish:<n>`, logs
/// its replies, and shows what it publishes.
const SHELL: &str = r#"
return mod {
	init = function(previous) return previous or { log = "" } end,
	update = function(state, msg)
		if msg == "ask" or (type(msg) == "string" and string.sub(msg, 1, 8) == "publish:") then
			return state, { { to = "provider:echo", msg = msg } }
		elseif type(msg) == "table" and msg.reply then
			logged(state, "reply=" .. msg.reply)
		end
	end,
	view = function(state, app)
		return shell(state, ui.text { "echo:" .. tostring(app.echo) })
	end,
}
"#;

/// Answers `ask` to the mod that sent it, and publishes `n` for
/// `publish:<n>`.
struct Echo;

impl Native for Echo {
    fn start(&mut self, _: &Handle) -> Result<(), String> {
        Ok(())
    }

    fn tick(&mut self, _: &Handle) {}

    fn receive(&mut self, host: &Handle, from: &str, message: Data) {
        let Data::String(text) = message else {
            return;
        };
        match text.strip_prefix("publish:") {
            Some(n) => host.publish(Data::from(n)),
            None => host.send(from, Data::map([("reply", Data::from(text))])),
        }
    }
}

/// Two main windows on the shell, sharing the provider `echo`: the one
/// opened first, then the other.
fn two_windows(cx: &mut TestAppContext) -> (tempfile::TempDir, [Entity<MoonframeApp>; 2]) {
    windows_with_entries(cx, Default::default())
}

fn windows_with_entries(
    cx: &mut TestAppContext,
    entries: moonframe_core::entries::Entries,
) -> (tempfile::TempDir, [Entity<MoonframeApp>; 2]) {
    cx.executor().allow_parking();
    let dir = folder(SHELL);
    let provider = dir.path().join("providers/echo");
    std::fs::create_dir_all(&provider).unwrap();
    std::fs::write(
        provider.join("provider.toml"),
        "kind = 'native'\ninterval = 50\n",
    )
    .unwrap();
    let echo: Factory = Arc::new(|| Box::new(Echo));
    let folders = Folders {
        builtin: dir.path().to_owned(),
        host: None,
        user: None,
        profile: None,
        bundled: None,
    };
    let apps = cx.update(|cx| {
        gpui_kit::init(cx);
        crate::actions::bind_keys(cx);
        let workspace = Workspace::open(dir.path(), false);
        Hub::install(
            &workspace,
            Natives::from([("echo".to_owned(), echo)]),
            Launch {
                entries,
                providers: Default::default(),
                folders,
                backends: Arc::new(Backends::for_app("e2e")),
                views: Registry::default(),
                chrome: workspace.chrome(),
            },
            cx,
        );
        open_main_window(cx);
        let first = main_windows(cx).pop().unwrap();
        open_main_window(cx);
        let second = main_windows(cx)
            .into_iter()
            .find(|app| *app != first)
            .unwrap();
        [first, second]
    });
    (dir, apps)
}

/// Sends `msg` to the shell in `app`, as one of its controls would.
fn tell(cx: &mut TestAppContext, app: &Entity<MoonframeApp>, msg: &str) {
    app.update(cx, |app, cx| app.deliver("shell", Data::from(msg), cx));
}

/// What the shell in `app` shows.
fn shown(cx: &mut TestAppContext, app: &Entity<MoonframeApp>) -> String {
    app.read_with(cx, |app, _| {
        format!(
            "{:?}",
            app.workspace.slot("shell").and_then(|slot| slot.view())
        )
    })
}

/// Waits for the provider's thread until `done` holds, or fails.
fn wait_until(
    cx: &mut TestAppContext,
    what: &str,
    mut done: impl FnMut(&mut TestAppContext) -> bool,
) {
    for _ in 0..200 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

#[gpui_kit::test]
fn a_provider_reply_reaches_only_the_window_that_asked(cx: &mut TestAppContext) {
    let (_dir, [first, second]) = two_windows(cx);
    let replies = |cx: &mut TestAppContext, app| shown(cx, app).matches("reply=ask").count();
    // Each asks in turn; only the asker hears its answer.
    for (asker, other, expected) in [(&second, &first, [0, 1]), (&first, &second, [1, 1])] {
        tell(cx, asker, "ask");
        wait_until(cx, "the reply", |cx| replies(cx, asker) > 0);
        // Time for a stray copy to arrive in the other window.
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
        assert_eq!(
            [replies(cx, &first), replies(cx, &second)],
            expected,
            "{}",
            shown(cx, other)
        );
    }
}

#[gpui_kit::test]
fn a_second_window_hears_providers_after_the_first_closes(cx: &mut TestAppContext) {
    let (_dir, [first, second]) = two_windows(cx);
    // Both show what the provider publishes.
    tell(cx, &first, "publish:1");
    wait_until(cx, "both windows to show 1", |cx| {
        [&first, &second]
            .iter()
            .all(|app| shown(cx, app).contains("echo:1"))
    });
    // The window opened first goes.
    let gone = first.read_with(cx, |app, _| app.main_window_id());
    drop(first);
    cx.update(|cx| {
        let window = cx
            .windows()
            .into_iter()
            .find(|window| window.window_id() == gone)
            .unwrap();
        window
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
    });
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| main_windows(cx).len()), 1);
    tell(cx, &second, "publish:2");
    wait_until(cx, "the second window to show 2", |cx| {
        shown(cx, &second).contains("echo:2")
    });
    tell(cx, &second, "ask");
    wait_until(cx, "the reply", |cx| {
        shown(cx, &second).contains("reply=ask")
    });
}

#[gpui_kit::test]
fn runtime_entry_handle_updates_every_open_window(cx: &mut TestAppContext) {
    use moonframe_core::entries::{Entries, RootPolicy};
    let entries = Entries::default();
    let external = tempfile::tempdir().unwrap();
    std::fs::write(
        external.path().join("main.luau"),
        "return mod { view = function() return ui.text 'external entry' end }",
    )
    .unwrap();
    entries
        .add_root("user", external.path(), RootPolicy::default())
        .unwrap();
    let (_dir, apps) = windows_with_entries(cx, entries.clone());
    let present = |cx: &mut TestAppContext, yes: bool| {
        apps.iter().all(|app| {
            app.read_with(cx, |app, _| {
                app.workspace.slot("user:notes").is_some() == yes
            })
        })
    };
    assert!(present(cx, false));
    let handle = entries.clone();
    let path = external.path().to_owned();
    std::thread::spawn(move || handle.add_entry("user", "notes", path).unwrap())
        .join()
        .unwrap();
    wait_until(cx, "entry in both windows", |cx| present(cx, true));
    entries.remove_entry("user", "notes");
    wait_until(cx, "entry removed from both windows", |cx| {
        present(cx, false)
    });
}
