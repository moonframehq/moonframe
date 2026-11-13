//! Terminals on a real shell, in a pseudo-terminal of the test's own.

use super::*;

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

pub(super) fn wait(window: &mut Runtime<Ui>, ticks: usize) {
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
fn a_tab_in_a_cell_takes_one_cell_as_any_blank_does() {
    // Alacritty leaves the tab it was sent in the cell it started from.
    let cells: Vec<moonframe_terminal::Cell> = "a\t  b"
        .chars()
        .map(|ch| moonframe_terminal::Cell {
            ch,
            ..moonframe_terminal::Cell::default()
        })
        .collect();
    let (line, _) = terminal::row_text(&cells, rune::Color::WHITE);
    assert_eq!(line, "a   b");
}

#[test]
fn a_shell_that_exited_gives_its_slot_back_once_its_tab_has_gone() {
    let main = r#"
local tabbed = use "tabbed"
return mod {
	init = function(previous) return previous or { on = true } end,
	update = function(state, msg)
		if type(msg) == "table" and msg.exited then state.on = false end
	end,
	view = function(state)
		return ui.column { grow = true,
			state.on and ui.terminal { id = "one", backend = "pty", exit = { gone = true } } or ui.text { "closed" },
		}
	end,
}
"#;
    let dir = folder(main);
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.waker = Some(Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 400.), Env::new(true, 2.));
    settle(&mut window);
    let terminal = rect_of(&window, |element| element.role == "terminal");
    click_at(&mut window, terminal);
    wait(&mut window, 40);
    assert_eq!(window.state.terminals.views.borrow().len(), 1);
    type_line(&mut window, "exit");
    assert!(wait_for(&mut window, "closed"));
    wait(&mut window, 4);
    assert!(
        window.state.terminals.views.borrow().is_empty(),
        "the slot is free"
    );
}

#[test]
fn a_terminal_opened_in_a_tab_fills_its_box_too() {
    // Two tabs draw their terminals in the same place: the box does not move
    // as one takes over from the other, so nothing lays out afresh.
    let main = r#"
return mod {
	init = function(previous) return previous or { id = "one" } end,
	update = function(state, msg) if msg == "other" then state.id = "two" end end,
	view = function(state)
		return ui.column { grow = true,
			ui.button { "Other", msg = "other" },
			ui.terminal { id = state.id, backend = "pty" },
		}
	end,
}
"#;
    let dir = folder(main);
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.waker = Some(Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 600.), Env::new(true, 2.));
    settle(&mut window);
    wait(&mut window, 20);
    let button = rect_of(&window, |element| element.text.as_deref() == Some("Other"));
    click_at(&mut window, button);
    wait(&mut window, 20);
    let rect = rect_of(&window, |element| element.role == "terminal");
    let screen = window.state.terminals.screen("shell/two").unwrap();
    let height = window
        .state
        .workspace()
        .stylesheet()
        .mono
        .size
        .unwrap_or(13.)
        * 1.3;
    let wanted = ((rect[3] - 2. * terminal::PADDING) / height).floor() as u16;
    assert_eq!(screen.rows, wanted, "{rect:?}");
}
