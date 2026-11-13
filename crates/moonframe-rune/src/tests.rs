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

// keep-sorted start
mod borders;
mod carry;
mod fields;
mod font_size;
mod maps;
mod markdown;
mod splits;
mod terminals;
mod windows;
// keep-sorted end

use terminals::wait;

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
        .workspace()
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
fn views_read_what_draws_them() {
    let dir = folder("return mod { view = function(_, app) return ui.text(app.renderer) end }");
    let window = open(dir.path());
    assert!(shows(&window, "rune"));
}

#[test]
fn the_stylesheets_colors_go_over_the_theme() {
    let dir = folder(
        "return mod { view = function() return ui.column { class = 'x', grow = true } end }",
    );
    let window = open(dir.path());
    let workspace = window.state.workspace();
    let sheet = workspace.stylesheet();
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

/// The component browser on the real app, on `first`.
fn gallery(first: &str) -> Runtime<Ui> {
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app");
    let mut ui = Ui::open(app, None);
    ui.gallery = true;
    ui.workspace_mut().show_demos(true);
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
            .label
            .as_deref()
            .is_some_and(|label| label.ends_with("is broken"))
    });
    assert!(told, "a sign shows the problem");
}

#[test]
fn a_view_that_breaks_never_moves_the_layout() {
    // The status bar's view starts failing at runtime, with a traceback.
    let main = r#"
return mod {
	view = function()
		return ui.column {
			grow = true,
			ui.button { "Break", msg = "break", to = "bar" },
			ui.column { grow = true, ui.text { "body" } },
			ui.mount "bar",
		}
	end,
}
"#;
    let dir = folder(main);
    let bar = dir.path().join("mods/bar");
    std::fs::create_dir_all(&bar).unwrap();
    let source = r#"return mod {
	init = function(previous) return previous or { load = { cpu = 3 } } end,
	update = function(state, msg)
		if msg == "break" then state.load = nil end
	end,
	view = function(state)
		return ui.row { height = 26, shrink = false, ui.text { `cpu {state.load.cpu}` } }
	end,
}"#;
    std::fs::write(bar.join("main.luau"), source).unwrap();
    let mut window = open(dir.path());
    let at = |window: &Runtime<Ui>, text: &str| {
        rect_of(window, |element| element.text.as_deref() == Some(text))
    };
    let (body, status) = (at(&window, "body"), at(&window, "cpu 3"));
    let button = at(&window, "Break");
    click_at(&mut window, button);
    assert_eq!(at(&window, "body"), body, "the rest stays where it was");
    assert_eq!(at(&window, "cpu 3"), status, "the last good view stays");
    // A sign on the view's corner says it broke; what went wrong is its
    // tooltip, not text over the window.
    let sign = window
        .elements()
        .iter()
        .find(|element| element.key.as_deref() == Some("broken:bar"))
        .map(|element| (element.rect, element.label.clone(), element.help.clone()))
        .expect("the sign shows");
    let ([x, y, w, h], label, help) = sign;
    assert_eq!(label.as_deref(), Some("bar is broken"));
    let help = help.unwrap_or_default();
    assert!(help.starts_with("bar:") && help.contains("cpu"), "{help}");
    assert!(w <= 20. && h <= 20., "a small sign: {:?}", sign.0);
    assert!(
        y >= status[1] - 26. && y + h <= status[1] + 26.,
        "on the bar: {:?}",
        sign.0
    );
    assert!(x + w <= 800., "inside the window");
    let told = window.elements().iter().any(|element| {
        element
            .text
            .as_deref()
            .is_some_and(|text| text.contains("bar:"))
    });
    assert!(!told, "no problem text over the window");
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

#[test]
fn about_says_the_version_and_what_draws_it() {
    let about = moonframe_core::About::default();
    assert_eq!(dock::shown_version(&about), identity().version);
    assert_eq!(dock::credits(&about, "Rune"), "Backend: Rune");
    let described = moonframe_core::About {
        description: Some("Notes".to_owned()),
        ..about.clone()
    };
    assert_eq!(dock::credits(&described, "GPUI"), "Notes\n\nBackend: GPUI");
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
                        justify = "between",
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
    // A mod with nothing to say right now: it wraps too, but hides nothing.
    let quiet = dir.path().join("mods").join("zz-quiet");
    std::fs::create_dir_all(&quiet).unwrap();
    std::fs::write(
        quiet.join("main.luau"),
        r#"return mod { region = "status", view = function() return ui.row {} end }"#,
    )
    .unwrap();
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

#[test]
fn a_chosen_appearance_stands_against_the_system() {
    let dir = folder("return mod { view = function() return ui.text { 'x' } end }");
    let mut ui = Ui::open(dir.path().to_owned(), None);
    let file = dir.path().join("settings.toml");
    ui.settings = Some(file.clone());
    let mut window = Runtime::new(ui, view, (400., 300.), Env::new(true, 2.));
    settle(&mut window);
    let dark = |window: &mut Runtime<Ui>| {
        window.update(|ui, tx| {
            poll(ui, 0., tx);
        });
        window.frame(FRAME);
        window.state.dark
    };
    assert!(dark(&mut window), "the system is dark");
    window.update(|ui, _| ui.set_appearance(Appearance::Light));
    settle(&mut window);
    assert!(
        !dark(&mut window),
        "light stands, though the system is dark"
    );
    assert!(!dark(&mut window), "and goes on standing");
    assert_eq!(
        settings::get(&file, settings::APPEARANCE).as_deref(),
        Some("light"),
        "and is kept"
    );
    window.update(|ui, _| ui.set_appearance(Appearance::System));
    settle(&mut window);
    assert!(dark(&mut window), "`system` follows it again");
}

#[test]
fn toggling_the_theme_chooses_the_other_one() {
    let dir = folder("return mod { view = function() return ui.text { 'x' } end }");
    let mut window = open(dir.path());
    window.update(|ui, tx| {
        ui.act("shell", &Action::Command(Command::ToggleTheme), tx);
    });
    assert_eq!(window.state.shared.appearance.get(), Appearance::Light);
    window.update(|ui, tx| {
        poll(ui, 0., tx);
    });
    assert!(!window.state.dark, "and it stands");
}

#[test]
fn regions_short_of_room_each_keep_a_mod() {
    let (_dir, window) = bar(320.);
    let [left, top, width, _] = drawn(&window, "first widget of the left");
    let [x, right_top, ..] = drawn(&window, "branch on the right");
    assert_eq!(top, right_top, "the branch left the bar");
    assert!(x >= left + width.min(1.), "the branch is at {x}");
    assert!(x + 1. < 320., "the branch is past the window at {x}");
}

#[test]
fn a_spread_mod_gives_up_one_widget_at_a_time() {
    let dir = folder(
        r#"return mod {
            view = function()
                return ui.column {
                    grow = true,
                    ui.row {
                        height = 30,
                        shrink = false,
                        ui.region { name = "status", direction = "row", gap = "large" },
                    },
                }
            end,
        }"#,
    );
    let meters = dir.path().join("mods/meters");
    std::fs::create_dir_all(&meters).unwrap();
    let source = r#"return mod {
        region = "status",
        view = function()
            return ui.row {
                spread = true,
                ui.row { shrink = false, ui.text { "the first meter of the bar" } },
                ui.row { shrink = false, ui.text { "the second meter of the bar" } },
            }
        end,
    }"#;
    std::fs::write(meters.join("main.luau"), source).unwrap();
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (260., 200.),
        Env::new(true, 2.),
    );
    settle(&mut window);
    let [_, first, ..] = drawn(&window, "the first meter of the bar");
    let [_, second, ..] = drawn(&window, "the second meter of the bar");
    assert!(first < 30., "the first left the bar at {first}");
    assert!(second >= 30., "the second stayed on the bar at {second}");
}

/// Whether the bar's chip says `count` items are hidden.
fn hidden_chip(window: &Runtime<Ui>) -> Option<[f32; 4]> {
    window
        .elements()
        .iter()
        .find(|element| {
            element
                .text
                .as_deref()
                .is_some_and(|text| text.starts_with('+'))
        })
        .map(|element| element.rect)
}

#[test]
fn a_bar_with_room_says_nothing_of_hidden_items() {
    let (_dir, window) = bar(1000.);
    assert!(hidden_chip(&window).is_none());
}

#[test]
fn a_bar_short_of_room_says_how_many_items_are_hidden() {
    let (_dir, mut window) = bar(320.);
    settle(&mut window);
    assert!(
        shows(&window, "+1"),
        "{:?}",
        window
            .elements()
            .iter()
            .filter_map(|e| e.text.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_hidden_chip_covers_no_item_that_shows() {
    let (_dir, mut window) = bar(320.);
    settle(&mut window);
    let chip = hidden_chip(&window).expect("a chip");
    let shown = drawn(&window, "first widget of the left");
    assert!(
        shown[0] + shown[2] <= chip[0] + 0.5,
        "{shown:?} runs under {chip:?}"
    );
}

#[test]
fn the_hidden_count_follows_the_window_both_ways() {
    let (_dir, mut window) = bar(1000.);
    assert!(hidden_chip(&window).is_none());
    window.set_viewport((320., 200.), 2.);
    settle(&mut window);
    assert!(hidden_chip(&window).is_some(), "narrowed, nothing said");
    window.set_viewport((1000., 200.), 2.);
    settle(&mut window);
    assert!(hidden_chip(&window).is_none(), "widened, chip stayed");
}

#[test]
fn the_hidden_count_holds_still_once_laid_out() {
    let (_dir, mut window) = bar(320.);
    settle(&mut window);
    let chip = hidden_chip(&window);
    for _ in 0..10 {
        window.frame(FRAME);
        assert_eq!(hidden_chip(&window), chip, "the chip flickers");
    }
}

#[test]
fn a_component_with_no_demo_shows_its_example() {
    let window = gallery("tab-view");
    assert!(shows(&window, "A tab's content"));
}

#[test]
fn a_static_example_sends_nothing_when_clicked() {
    let mut window = gallery("tab-view");
    let page = rect_of(&window, |element| {
        element.key.as_deref() == Some("gallery-static")
    });
    click_at(&mut window, page);
    assert!(shows(&window, "A tab's content"));
}

#[test]
fn the_gallery_page_says_how_to_use_a_component() {
    let window = gallery("tab-view");
    assert!(shows(&window, "Tab view"));
    assert!(shows(&window, "Use it in a mod"));
}

#[test]
fn a_search_narrows_the_gallery() {
    let mut window = gallery("tab-view");
    window
        .state
        .fields
        .get_mut()
        .entry("gallery:search".to_owned())
        .or_default()
        .edit
        .replace_text("no component has this name".to_owned());
    window.state.preview = None;
    window.invalidate();
    settle(&mut window);
    assert!(shows(
        &window,
        "No components match “no component has this name”."
    ));
}

#[test]
fn a_row_that_hugs_keeps_its_own_width_in_a_column() {
    let dir = folder(
        r#"return mod {
            view = function()
                return ui.column {
                    ui.row { hug = true, ui.text "short" },
                    ui.row { ui.text "wide" },
                }
            end,
        }"#,
    );
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (800., 200.),
        Env::new(true, 2.),
    );
    settle(&mut window);
    let elements = window.elements();
    let parent_of = |said: &str| {
        let text = elements
            .iter()
            .find(|element| element.text.as_deref() == Some(said))
            .unwrap();
        elements[text.parent.unwrap()].rect[2]
    };
    assert!(parent_of("short") < 200., "{}", parent_of("short"));
    assert!(parent_of("wide") > 600., "{}", parent_of("wide"));
}

/// A track that says the value a drag sets.
fn slider_window(width: f32) -> (TempDir, Runtime<Ui>) {
    let dir = folder(
        r#"return mod {
            init = function() return { value = 0 } end,
            update = function(state, msg)
                state.value = msg.value or state.value
                return state
            end,
            view = function(state)
                return ui.column {
                    ui.row {
                        height = 20,
                        width = 200,
                        shrink = false,
                        pointer = { set = true },
                        range = { min = 0, max = 100, step = 1, value = state.value },
                    },
                    ui.text { `value {state.value}` },
                }
            end,
        }"#,
    );
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (width, 200.),
        Env::new(true, 2.),
    );
    settle(&mut window);
    (dir, window)
}

#[test]
fn dragging_a_track_sets_its_value() {
    let (_dir, mut window) = slider_window(400.);
    assert!(shows(&window, "value 0"));
    let track = rect_of(&window, |element| {
        (element.rect[2] - 200.).abs() < 0.5 && (element.rect[3] - 20.).abs() < 0.5
    });
    let y = track[1] + 10.;
    window.pointer_down(track[0] + 20., y);
    settle(&mut window);
    assert!(shows(&window, "value 10"), "a press sets it");
    window.pointer_move(track[0] + 150., y);
    settle(&mut window);
    assert!(shows(&window, "value 75"), "a drag follows");
    // Past the end of the track it stays at the end.
    window.pointer_move(track[0] + 390., y);
    window.pointer_up(track[0] + 390., y);
    settle(&mut window);
    assert!(shows(&window, "value 100"), "it stops at the end");
}
