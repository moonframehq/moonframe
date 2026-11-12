//! Mods in windows of their own: two Rune windows, each with its own `Ui`,
//! drawing from one workspace.

use {super::*, crate::windows};

/// A shell that shows a count, and a mod `counter` whose button adds to it
/// and which says whether its window is in front.
fn app() -> TempDir {
    let dir = folder(
        r#"return mod {
            init = function(previous) return previous or { count = 0 } end,
            update = function(state, msg)
                if msg == "add" then state.count += 1 end
            end,
            view = function(state)
                return ui.column { ui.text { `count {state.count}` } }
            end,
        }"#,
    );
    let counter = dir.path().join("mods/counter");
    std::fs::create_dir_all(&counter).unwrap();
    let source = r#"return mod {
        title = "Counter",
        window = { size = { width = 300, height = 200 } },
        view = function(state, app)
            return ui.column {
                ui.button { "Add", msg = "add", to = "shell" },
                ui.text { `active {app.window.active}` },
            }
        end,
    }"#;
    std::fs::write(counter.join("main.luau"), source).unwrap();
    dir
}

/// The main window and the counter's, drawing from one workspace.
fn windows(dir: &Path) -> (Runtime<Ui>, Runtime<Ui>) {
    let main = Ui::open(dir.to_owned(), None);
    let side = Ui::for_window(&main, "counter");
    let mut main = Runtime::new(main, view, (800., 600.), Env::new(true, 2.));
    let mut side = Runtime::new(side, view, (300., 200.), Env::new(true, 2.));
    settle(&mut main);
    settle(&mut side);
    (main, side)
}

/// One poll, as Rune runs it when a window is woken, then the frames to draw.
fn polled(window: &mut Runtime<Ui>) {
    window.update(|ui, tx| {
        poll(ui, 0., tx);
    });
    settle(window);
}

#[test]
fn a_mods_window_draws_the_mod_and_its_clicks_reach_the_main_window() {
    let dir = app();
    let (mut main, mut side) = windows(dir.path());
    assert!(shows(&side, "Add"), "the window draws its mod");
    assert!(!shows(&side, "count 0"), "and not the shell");
    assert!(shows(&main, "count 0"));
    let add = rect_of(&side, |element| element.text.as_deref() == Some("Add"));
    click_at(&mut side, add);
    // The window that changed the workspace wakes the others, whose poll
    // finds the change and draws it.
    polled(&mut side);
    assert!(drawn_again(&mut main), "the main window hears of the click");
    settle(&mut main);
    assert!(shows(&main, "count 1"), "the main window draws the click");
    assert!(!drawn_again(&mut main), "and only once");
    assert!(!drawn_again(&mut side), "nor does its own change come back");
}

/// Whether a poll would draw the window again.
fn drawn_again(window: &mut Runtime<Ui>) -> bool {
    let again = Rc::new(Cell::new(false));
    let told = Rc::clone(&again);
    window.update(move |ui, tx| told.set(poll(ui, 0., tx)));
    again.get()
}

#[test]
fn a_mods_window_tells_its_mod_when_it_is_in_front_and_when_it_closes() {
    let dir = app();
    let (mut main, mut side) = windows(dir.path());
    assert!(shows(&side, "active true"), "the main window's state");
    side.update(|ui, tx| {
        ui.opened = true;
        windows::focused(ui, false, tx);
    });
    polled(&mut side);
    assert!(shows(&side, "active false"), "its own window's state");
    side.update(windows::closed);
    polled(&mut side);
    polled(&mut main);
    assert!(shows(&side, "active true"), "back to the main window's");
    assert!(!side.state.opened);
    // A window that is closed is not brought back by losing focus.
    side.update(|ui, tx| windows::focused(ui, false, tx));
    polled(&mut side);
    assert!(shows(&side, "active true"));
}

#[test]
fn every_mod_gets_a_window_sized_as_it_asks() {
    let dir = app();
    let mut main = Ui::open(dir.path().to_owned(), None);
    let spec = rune::AppSpec::new("test", (800., 600.), unused_init, view);
    let mut app = rune::App::new(spec);
    windows::add(&mut app, &mut main);
    assert!(main.mod_windows.contains_key("counter"));
    assert!(!main.mod_windows.contains_key(moonframe_core::SHELL));
    // The main window's handle, then each mod's.
    assert_eq!(main.shared.handles().len(), 1 + main.mod_windows.len());
    let asked = moonframe_core::ModWindowSize {
        size: Some((300., 200.)),
        ..Default::default()
    };
    assert_eq!(windows::fitted(&asked), ((300., 200.), (280., 200.)));
    let unsaid = moonframe_core::ModWindowSize::default();
    assert_eq!(windows::fitted(&unsaid), ((420., 560.), (280., 240.)));
    let wide = moonframe_core::ModWindowSize {
        min_size: Some((640., 480.)),
        ..Default::default()
    };
    assert_eq!(windows::fitted(&wide), ((640., 560.), (640., 480.)));
}

#[test]
fn only_a_windows_own_commands_stay_in_a_mods_window() {
    for command in [Command::CloseWindow, Command::Minimize, Command::Maximize] {
        assert!(windows::window_command(command), "{command:?}");
    }
    for command in [
        Command::Quit,
        Command::Inspect,
        Command::ToggleTheme,
        Command::Reload,
    ] {
        assert!(!windows::window_command(command), "{command:?}");
    }
}
