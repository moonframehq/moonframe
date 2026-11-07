//! The real components on a headless Rune window, through its hit testing.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use {
    super::*,
    rune::{Env, Key, KeyEvent, Mods, Phase, Runtime},
    std::path::Path,
    tempfile::TempDir,
};

const SHELL: &str = include_str!("../../moonframe-core/fixtures/both-backends.luau");
const FRAME: f64 = 1. / 60.;

/// An app folder holding `main`, with the real components and stylesheet.
fn folder(main: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    std::fs::write(dir.path().join("main.luau"), main).unwrap();
    std::fs::copy(app.join("style.luau"), dir.path().join("style.luau")).unwrap();
    std::os::unix::fs::symlink(app.join("components"), dir.path().join("components")).unwrap();
    dir
}

fn open(dir: &Path) -> Runtime<Ui> {
    let mut window = Runtime::new(
        Ui::open(dir.to_owned(), None),
        view,
        (800., 600.),
        Env::new(true, 2.),
    );
    settle(&mut window);
    window
}

/// Runs enough frames for handlers to land and the view to run again.
fn settle(window: &mut Runtime<Ui>) {
    for _ in 0..30 {
        window.frame(FRAME);
    }
}

fn shows(window: &Runtime<Ui>, text: &str) -> bool {
    window
        .elements()
        .iter()
        .any(|element| element.text.as_deref() == Some(text))
}

fn click_at(window: &mut Runtime<Ui>, [x, y, w, h]: [f32; 4]) {
    let (x, y) = (x + w / 2., y + h / 2.);
    window.pointer_down(x, y);
    window.pointer_up(x, y);
    settle(window);
}

fn rect_of(window: &Runtime<Ui>, found: impl Fn(&rune::ElementInfo) -> bool) -> [f32; 4] {
    window
        .elements()
        .iter()
        .find(|element| found(element))
        .map(|element| element.rect)
        .expect("drawn")
}

#[test]
fn one_luau_source_clicks_and_types() {
    let dir = folder(SHELL);
    let mut window = open(dir.path());
    assert!(!shows(&window, "The body shows"));

    let title = rect_of(&window, |element| {
        element.text.as_deref() == Some("Details")
    });
    click_at(&mut window, title);
    assert!(shows(&window, "The body shows"));

    let field = rect_of(&window, |element| element.role == "textbox");
    click_at(&mut window, field);
    for letter in ["A", "d", "a"] {
        window.key(KeyEvent {
            key: Key::Text(letter.to_owned()),
            mods: Mods::default(),
        });
        settle(&mut window);
    }
    assert!(shows(&window, "Hello Ada"));
}

#[test]
fn the_starter_app_loads_without_a_problem() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    let window = open(&app);
    let problems: Vec<String> = window
        .state
        .workspace
        .problems()
        .iter()
        .map(|(name, problem)| format!("{name}: {problem}"))
        .collect();
    assert!(problems.is_empty(), "{problems:?}");
    assert!(shows(&window, "Explorer"));
}

#[test]
fn an_element_left_out_says_so() {
    let dir = folder("return mod { view = function() return ui.browser { id = 'b' } end }");
    let window = open(dir.path());
    assert!(shows(&window, "ui.browser needs the GPUI backend"));
}

#[test]
fn a_real_shell_echoes_what_is_typed() {
    let dir = folder("return mod { view = function() return ui.terminal { id = 'shell' } end }");
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.waker = Some(Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 400.), Env::new(true, 2.));
    settle(&mut window);
    window.update(|ui, _| {
        ui.terminals
            .dirty
            .store(true, std::sync::atomic::Ordering::Release);
    });
    let tick = |window: &mut Runtime<Ui>| {
        std::thread::sleep(Duration::from_millis(50));
        window.update(|ui, tx| {
            poll(ui, 0., tx);
        });
        window.frame(FRAME);
    };
    let terminal = rect_of(&window, |element| element.role == "terminal");
    click_at(&mut window, terminal);
    for _ in 0..40 {
        tick(&mut window);
    }
    let line = "echo rune-$((6*7))";
    for letter in line.chars() {
        window.key(KeyEvent {
            key: Key::Text(letter.to_string()),
            mods: Mods::default(),
        });
        window.frame(FRAME);
    }
    window.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    let answered = |window: &Runtime<Ui>| {
        window.elements().iter().any(|element| {
            element
                .text
                .as_deref()
                .is_some_and(|text| text.trim() == "rune-42")
        })
    };
    for _ in 0..100 {
        tick(&mut window);
        if answered(&window) {
            return;
        }
    }
    let texts: Vec<String> = window
        .elements()
        .iter()
        .filter_map(|element| element.text.clone())
        .collect();
    panic!("the shell never answered: {texts:?}");
}

#[test]
fn the_stylesheets_colors_go_over_the_theme() {
    let dir = folder(
        "return mod { view = function() return ui.column { class = 'x', grow = true } end }",
    );
    let window = open(dir.path());
    let sheet = window.state.workspace.stylesheet();
    let palette = theme::Palette {
        dark: true,
        overrides: &sheet.overrides,
    };
    // `accent` is a `colors` entry of app/style.luau; `background` is not.
    let accent = moonframe_core::Token::Accent;
    assert_eq!(palette.token(accent).hex(), "#818cf8ff");
    assert_eq!(
        palette.token(moonframe_core::Token::Background).hex(),
        "#0a0a0aff"
    );
}

/// A shell counting what reaches it from its `keys`, a layer's, and a field.
const KEYED: &str = r#"
return mod {
	keys = { ["ctrl-k"] = "shell-key" },
	init = function(previous) return previous or { log = "", open = true } end,
	update = function(state, msg)
		if msg == "shell-key" then state.log ..= "S" end
		if msg == "layer-key" then state.log ..= "L"; state.open = false end
		if type(msg) == "table" and msg.msg == "left" then state.log ..= "B:" .. msg.value end
		if msg == "field-key" then state.log ..= "F" end
	end,
	view = function(state)
		return ui.column {
			grow = true,
			ui.text { "log:" .. state.log },
			ui.input { id = "a", focus = true, blur = "left", keys = { up = "field-key" } },
			ui.input { id = "b" },
			ui.overlay {
				open = state.open,
				keys = { ["ctrl-j"] = "layer-key" },
				content = ui.text { "over" },
			},
		}
	end,
}
"#;

fn ctrl(letter: &str) -> KeyEvent {
    KeyEvent {
        key: Key::Text(letter.to_owned()),
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
    }
}

fn logged(window: &Runtime<Ui>) -> String {
    window
        .elements()
        .iter()
        .find_map(|element| {
            element
                .text
                .as_deref()?
                .strip_prefix("log:")
                .map(str::to_owned)
        })
        .expect("a log")
}

#[test]
fn a_layer_takes_its_keys_first_then_mods_hear_theirs() {
    let dir = folder(KEYED);
    let mut window = open(dir.path());
    // Under an open overlay, a mod's key is not the layer's and still reaches it.
    window.update(|ui, tx| {
        assert!(ui.key(&ctrl("j"), tx));
    });
    settle(&mut window);
    assert_eq!(logged(&window), "L");
    assert!(!shows(&window, "over"));
    window.update(|ui, tx| {
        assert!(ui.key(&ctrl("k"), tx));
        assert!(!ui.key(&ctrl("q"), tx));
    });
    settle(&mut window);
    assert_eq!(logged(&window), "LS");
}

#[test]
fn a_field_takes_focus_when_asked_hears_its_keys_and_tells_of_blur() {
    let dir = folder(KEYED);
    let mut window = open(dir.path());
    window.update(|ui, tx| {
        ui.key(&ctrl("j"), tx);
    });
    settle(&mut window);
    let focused = |window: &Runtime<Ui>| {
        window
            .elements()
            .iter()
            .any(|element| element.role == "textbox" && element.focused)
    };
    assert!(focused(&window), "`focus` took the keyboard");
    window.key(KeyEvent {
        key: Key::Up,
        mods: Mods::default(),
    });
    settle(&mut window);
    assert_eq!(logged(&window), "LF");
    window.key(KeyEvent {
        key: Key::Text("z".to_owned()),
        mods: Mods::default(),
    });
    settle(&mut window);
    // `poll` runs every frame in a window; here it has to be asked.
    window.update(|ui, tx| {
        poll(ui, 0., tx);
    });
    let other = window
        .elements()
        .iter()
        .filter(|element| element.role == "textbox")
        .nth(1)
        .map(|element| element.rect)
        .expect("a second field");
    click_at(&mut window, other);
    window.update(|ui, tx| {
        poll(ui, 0., tx);
    });
    settle(&mut window);
    assert_eq!(logged(&window), "LFB:z");
}

#[test]
fn a_saved_edit_shows_without_a_restart() {
    let dir = folder("return mod { view = function() return ui.text { 'before' } end }");
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.watching = watch::watching(&ui, Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 600.), Env::new(true, 2.));
    settle(&mut window);
    assert!(shows(&window, "before"));
    // Events take a moment to start after a watch begins.
    std::thread::sleep(Duration::from_millis(300));
    let main = dir.path().join("main.luau");
    std::fs::write(
        main,
        "return mod { view = function() return ui.text { 'after' } end }",
    )
    .unwrap();
    for _ in 0..80 {
        std::thread::sleep(Duration::from_millis(50));
        window.update(|ui, tx| {
            poll(ui, 0., tx);
        });
        window.frame(FRAME);
        if shows(&window, "after") {
            return;
        }
    }
    panic!("the edit never showed");
}

#[test]
fn every_native_provider_of_the_starter_is_registered() {
    let registered = natives::natives();
    for name in [
        "acp", "chat", "docs", "git", "github", "gitlab", "shell", "speech", "usage",
    ] {
        assert!(registered.contains_key(name), "{name}");
    }
}

/// A window on `main`, with a picture beside it and pictures able to load.
fn with_picture(main: &str) -> (TempDir, Runtime<Ui>) {
    let dir = folder(main);
    let png = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon.png");
    std::fs::copy(png, dir.path().join("picture.png")).unwrap();
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.images = Some(ImageCache::new(Arc::new(|| {})));
    let mut window = Runtime::new(ui, view, (800., 600.), Env::new(true, 2.));
    settle(&mut window);
    (dir, window)
}

fn pictures(window: &Runtime<Ui>) -> usize {
    window.state.pictures.borrow().len()
}

#[test]
fn a_picture_in_the_apps_folder_loads() {
    let main = "return mod { view = function() return ui.image { src = 'picture.png', width = 64, height = 64 } end }";
    let (_dir, window) = with_picture(main);
    assert_eq!(pictures(&window), 1);
}

#[test]
fn a_picture_outside_the_apps_folders_is_refused() {
    let outside = tempfile::tempdir().unwrap();
    let file = outside.path().join("secret.png");
    let png = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icon.png");
    std::fs::copy(png, &file).unwrap();
    let main = format!(
        "return mod {{ view = function() return ui.image {{ src = {:?}, width = 64, height = 64 }} end }}",
        file.display().to_string(),
    );
    let (_dir, window) = with_picture(&main);
    assert_eq!(pictures(&window), 0);
}

#[test]
fn the_tab_bars_plus_menu_opens_a_terminal() {
    let main = r#"
local tabbed = use "tabbed"
return mod { view = function() return ui.column { grow = true, tabbed { id = "main" } } end }
"#;
    let dir = folder(main);
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.waker = Some(Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 600.), Env::new(true, 2.));
    settle(&mut window);
    let add = rect_of(&window, |element| {
        element
            .key
            .as_deref()
            .is_some_and(|key| key.ends_with("-anchor"))
    });
    click_at(&mut window, add);
    let offered: Vec<String> = window
        .elements()
        .iter()
        .filter_map(|element| element.text.clone())
        .collect();
    let terminal = rect_of(&window, |element| {
        element.text.as_deref() == Some("Terminal")
    });
    assert!(terminal[2] > 0., "{offered:?}");
    click_at(&mut window, terminal);
    let shown = window
        .elements()
        .iter()
        .any(|element| element.role == "terminal");
    assert!(shown, "no terminal after picking it: {offered:?}");
}

/// A real shell in a window, ready for typing.
fn shell_window(dir: &TempDir) -> Runtime<Ui> {
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.waker = Some(Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 400.), Env::new(true, 2.));
    settle(&mut window);
    let terminal = rect_of(&window, |element| element.role == "terminal");
    click_at(&mut window, terminal);
    wait(&mut window, 40);
    window
}

fn wait(window: &mut Runtime<Ui>, ticks: usize) {
    for _ in 0..ticks {
        std::thread::sleep(Duration::from_millis(50));
        window.update(|ui, tx| {
            poll(ui, 0., tx);
        });
        window.frame(FRAME);
    }
}

fn type_line(window: &mut Runtime<Ui>, line: &str) {
    for letter in line.chars() {
        window.key(KeyEvent {
            key: Key::Text(letter.to_string()),
            mods: Mods::default(),
        });
        window.frame(FRAME);
    }
    window.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    window.frame(FRAME);
}

/// Waits for a row reading `wanted`, trimmed, or containing it.
fn wait_for(window: &mut Runtime<Ui>, wanted: &str) -> bool {
    for _ in 0..120 {
        wait(window, 1);
        let found = window.elements().iter().any(|element| {
            element
                .text
                .as_deref()
                .is_some_and(|text| text.contains(wanted))
        });
        if found {
            return true;
        }
    }
    false
}

const TERMINAL: &str =
    "return mod { view = function() return ui.terminal { id = 'shell', backend = 'pty' } end }";

#[test]
fn dragging_over_a_shells_output_selects_its_text() {
    let dir = folder(TERMINAL);
    let mut window = shell_window(&dir);
    type_line(&mut window, "echo selected-$((6*7))");
    assert!(wait_for(&mut window, "selected-42"));
    let key = "shell/shell".to_owned();
    let metrics = render::Metrics {
        width: 8.,
        height: 16.,
    };
    let screen = window.state.terminals.screen(&key).unwrap();
    let row = (0..screen.rows)
        .find(|row| {
            let line: String = screen.row(*row).iter().map(|cell| cell.ch).collect();
            line.trim() == "selected-42"
        })
        .expect("the answer's row");
    let y = terminal::PADDING + (f32::from(row) + 0.5) * metrics.height;
    let terminals = &window.state.terminals;
    assert_eq!(
        terminals.select(&key, (9., y), (Phase::Down, 2), metrics),
        None
    );
    let chosen = terminals.select(&key, (9., y), (Phase::Up, 2), metrics);
    assert_eq!(
        chosen.as_deref(),
        Some("selected-42"),
        "a double click takes the word"
    );
}

#[test]
fn a_shell_that_exits_says_so_and_enter_starts_another() {
    let dir = folder(TERMINAL);
    let mut window = shell_window(&dir);
    type_line(&mut window, "exit");
    assert!(wait_for(&mut window, "press Enter to start another"));
    window.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    window.frame(FRAME);
    wait(&mut window, 40);
    type_line(&mut window, "echo again-$((6*7))");
    assert!(wait_for(&mut window, "again-42"));
}

/// The component browser on the real app, on `first`.
fn gallery(first: &str) -> Runtime<Ui> {
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    let mut ui = Ui::open(app, None);
    ui.gallery = true;
    ui.workspace.show_demos(true);
    ui.preview = Some(first.to_owned());
    let mut window = Runtime::new(ui, view, (1000., 800.), Env::new(true, 2.));
    settle(&mut window);
    window
}

#[test]
fn the_gallery_list_answers_while_a_demos_dialog_is_open() {
    let mut window = gallery("about");
    // `about` opens with its dialog showing, as the browser does on launch.
    assert!(shows(&window, "Moonframe"), "the dialog is open");
    let entry = rect_of(&window, |element| {
        element.key.as_deref() == Some("gallery-card")
    });
    click_at(&mut window, entry);
    assert_eq!(window.state.preview.as_deref(), Some("card"));
}

#[test]
fn a_demos_dialog_stays_inside_the_stage() {
    let window = gallery("about");
    let stage = rect_of(&window, |element| {
        element.key.as_deref() == Some("gallery-stage")
    });
    let title = rect_of(&window, |element| {
        element.text.as_deref() == Some("Moonframe")
    });
    assert!(title[0] >= stage[0], "{title:?} inside {stage:?}");
    let middle = stage[0] + stage[2] / 2.;
    let centre = title[0] + title[2] / 2.;
    assert!(
        (centre - middle).abs() < 2.,
        "centred on the stage: {centre} {middle}"
    );
}

#[test]
fn a_demos_dialog_keeps_its_own_clicks_and_closes_from_outside() {
    let mut window = gallery("about");
    let title = rect_of(&window, |element| {
        element.text.as_deref() == Some("Moonframe")
    });
    click_at(&mut window, title);
    assert!(
        shows(&window, "Moonframe"),
        "a click in the dialog keeps it open"
    );
    let stage = rect_of(&window, |element| {
        element.key.as_deref() == Some("gallery-stage")
    });
    let corner = [stage[0] + stage[2] - 30., stage[1] + stage[3] - 30., 2., 2.];
    click_at(&mut window, corner);
    assert!(!shows(&window, "Moonframe"), "a click beside it closes it");
}

#[test]
fn a_problem_keeps_the_view_and_floats_over_it() {
    // A send to a mod that isn't there is the sender's problem, not the end of its view.
    let main = r#"
return mod {
	update = function(state, msg)
		return state, { { to = "nowhere", msg = "hello" } }
	end,
	view = function() return ui.column { grow = true, ui.button { "Send", msg = "go" }, ui.text { "kept" } } end,
}
"#;
    let dir = folder(main);
    let mut window = open(dir.path());
    let button = rect_of(&window, |element| element.text.as_deref() == Some("Send"));
    click_at(&mut window, button);
    assert!(shows(&window, "kept"), "the view stays");
    let told = window.elements().iter().any(|element| {
        element
            .text
            .as_deref()
            .is_some_and(|text| text.contains("no mod named"))
    });
    assert!(told, "the problem shows");
}

#[test]
fn the_shown_terminal_takes_keys_without_a_click() {
    let dir = folder(TERMINAL);
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.waker = Some(Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 400.), Env::new(true, 2.));
    settle(&mut window);
    wait(&mut window, 40);
    for letter in "echo typed-$((6*7))".chars() {
        let event = KeyEvent {
            key: Key::Text(letter.to_string()),
            mods: Mods::default(),
        };
        window.update(|ui, tx| {
            assert!(ui.key(&event, tx), "{letter}");
        });
        window.frame(FRAME);
    }
    let enter = KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    };
    window.update(|ui, tx| {
        assert!(ui.key(&enter, tx));
    });
    assert!(wait_for(&mut window, "typed-42"));
}

#[test]
fn a_control_character_reaches_the_shell() {
    let dir = folder(TERMINAL);
    let mut window = shell_window(&dir);
    for letter in "sleep 30".chars() {
        window.key(KeyEvent {
            key: Key::Text(letter.to_string()),
            mods: Mods::default(),
        });
        window.frame(FRAME);
    }
    window.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    wait(&mut window, 10);
    // What macOS can send for ⌃C.
    window.key(KeyEvent {
        key: Key::Text("\u{3}".to_owned()),
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
    });
    window.frame(FRAME);
    wait(&mut window, 10);
    type_line(&mut window, "echo after-$((6*7))");
    assert!(wait_for(&mut window, "after-42"), "⌃C stopped the sleep");
}

#[test]
fn the_grid_fills_its_box_once_the_cell_is_measured() {
    let dir = folder(TERMINAL);
    let mut window = shell_window(&dir);
    wait(&mut window, 10);
    let rect = rect_of(&window, |element| element.role == "terminal");
    let screen = window.state.terminals.screen("shell/shell").unwrap();
    let cell = window.state.cell_width.get().expect("a measured cell");
    let wanted = ((rect[2] - 2. * terminal::PADDING) / cell).floor() as u16;
    assert_eq!(screen.cols, wanted, "cell {cell} in {rect:?}");
}

#[test]
fn inspect_picks_what_is_under_the_click_and_tells_the_mod() {
    let main = r#"
return mod {
	subscribe = { "app/inspect" },
	init = function(previous) return previous or { said = "none" } end,
	update = function(state, msg)
		if type(msg) == "table" and msg.topic == "app/inspect" then
			state.said = tostring(msg.value.picking) .. ":" .. tostring(msg.value.element)
		end
	end,
	view = function(state)
		return ui.column { grow = true,
			ui.text { "said:" .. state.said },
			ui.row { class = "target", command = "inspect", ui.text { "start" } },
			ui.text { "the target", class = "picked-one" },
		}
	end,
}
"#;
    let dir = folder(main);
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.waker = Some(Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 400.), Env::new(true, 2.));
    settle(&mut window);
    let start = rect_of(&window, |element| element.text.as_deref() == Some("start"));
    click_at(&mut window, start);
    assert!(
        shows(&window, "said:true:nil"),
        "the mod hears picking started"
    );
    let target = rect_of(&window, |element| {
        element.text.as_deref() == Some("the target")
    });
    click_at(&mut window, target);
    wait(&mut window, 6);
    assert!(
        shows(&window, "said:false:ui.text .picked-one"),
        "{:?}",
        window
            .elements()
            .iter()
            .filter_map(|e| e.text.clone())
            .collect::<Vec<_>>()
    );
}

/// What of a terminal's grid is fading or gliding this frame.
fn ghosts(window: &Runtime<Ui>) -> Vec<String> {
    window
        .elements()
        .iter()
        .filter(|element| {
            let in_grid = element
                .key
                .as_deref()
                .is_some_and(|key| key.contains("-row") || key.contains("-cursor"));
            in_grid && (element.exiting || element.moving || element.opacity < 1.)
        })
        .map(|element| format!("{:?} {:?}", element.key, element.text))
        .collect()
}

#[test]
fn a_terminal_changes_at_once_with_no_ghosts() {
    let dir = folder(TERMINAL);
    let mut window = shell_window(&dir);
    type_line(&mut window, "echo one; echo two; echo three");
    assert!(wait_for(&mut window, "three"));
    let mut seen = Vec::new();
    for line in ["printf abc", "clear", "echo after-clear"] {
        for letter in line.chars() {
            window.key(KeyEvent {
                key: Key::Text(letter.to_string()),
                mods: Mods::default(),
            });
            for _ in 0..3 {
                wait(&mut window, 1);
                seen.extend(ghosts(&window));
            }
        }
        window.key(KeyEvent {
            key: Key::Enter,
            mods: Mods::default(),
        });
        for _ in 0..12 {
            wait(&mut window, 1);
            seen.extend(ghosts(&window));
        }
    }
    assert!(
        seen.is_empty(),
        "{} ghosts, such as {:?}",
        seen.len(),
        seen.first()
    );
}

#[test]
fn about_says_the_version_and_what_draws_it() {
    let about = moonframe_core::About::default();
    assert_eq!(dock::shown_version(&about), identity().version);
    assert!(dock::built_on("Rune").ends_with(" · Rune"));
    let named = moonframe_core::About {
        version: Some("1.2.3".to_owned()),
        ..about
    };
    assert_eq!(dock::shown_version(&named), "1.2.3");
}

/// A bar of two regions, as the starter's status bar, in a window `width` wide.
fn bar(width: f32) -> (TempDir, Runtime<Ui>) {
    let dir = folder(
        r#"return mod {
            view = function()
                return ui.column {
                    grow = true,
                    ui.row {
                        height = 30,
                        shrink = false,
                        gap = "large",
                        ui.region { name = "status", direction = "row", gap = "large" },
                        ui.region { name = "status-right", direction = "row", justify = "end" },
                    },
                }
            end,
        }"#,
    );
    for (name, region, label) in [
        ("first", "status", "first widget of the left"),
        ("second", "status", "second widget of the left"),
        ("branch", "status-right", "branch on the right"),
    ] {
        let folder = dir.path().join("mods").join(name);
        std::fs::create_dir_all(&folder).unwrap();
        let source = format!(
            r#"return mod {{
                region = "{region}",
                view = function() return ui.row {{ shrink = false, ui.text {{ "{label}" }} }} end,
            }}"#
        );
        std::fs::write(folder.join("main.luau"), source).unwrap();
    }
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (width, 200.),
        Env::new(true, 2.),
    );
    settle(&mut window);
    (dir, window)
}

fn drawn(window: &Runtime<Ui>, label: &str) -> [f32; 4] {
    rect_of(window, |element| element.text.as_deref() == Some(label))
}

#[test]
fn a_row_region_with_room_keeps_every_mod_and_pushes_the_next_right() {
    let (_dir, window) = bar(900.);
    let [_, top, ..] = drawn(&window, "first widget of the left");
    let [second, same_line, ..] = drawn(&window, "second widget of the left");
    let [x, _, w, _] = drawn(&window, "branch on the right");
    assert_eq!(top, same_line);
    assert!(second > 0.);
    assert!((x + w - 900.).abs() < 1., "ends at {}", x + w);
}

#[test]
fn a_mod_that_does_not_fit_leaves_the_row_rather_than_covering_its_neighbour() {
    let (_dir, window) = bar(320.);
    let [_, top, ..] = drawn(&window, "first widget of the left");
    let [_, below, ..] = drawn(&window, "second widget of the left");
    let [x, ..] = drawn(&window, "branch on the right");
    let [left, _, width, _] = drawn(&window, "first widget of the left");
    assert!(
        below >= top + 30.,
        "the second stayed on the bar at {below}"
    );
    assert!(x >= left + width, "the branch is over the first at {x}");
}
