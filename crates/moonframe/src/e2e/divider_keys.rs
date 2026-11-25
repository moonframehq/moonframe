//! A split's divider takes the keyboard once clicked, and a double-click
//! puts the panes back to their `reset` size, else their starting size.

use {
    super::*,
    gpui_kit::{MouseButton, MouseDownEvent, MouseUpEvent},
};

const SPLIT: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	view = function(state)
		return shell(state, ui.hsplit { id = "people",
			ui.pane { size = 200, min = 100, max = 400, ui.column { class = "list", grow = true, ui.text { "List" } } },
			ui.pane { ui.column { class = "person", grow = true, ui.text { "Person" } } },
		})
	end,
}
"##;

fn width(window: &mut Window<'_>) -> f32 {
    f32::from(window.bounds("list").size.width)
}

/// Where the divider is: on the list pane's right edge.
fn divider(window: &mut Window<'_>) -> Point<Pixels> {
    let list = window.bounds("list");
    point(list.right(), list.center().y)
}

#[gpui_kit::test]
fn a_divider_moves_with_the_keys_and_comes_back_on_a_double_click(cx: &mut TestAppContext) {
    let mut window = open(cx, SPLIT);
    let start = width(&mut window);
    assert!((start - 200.).abs() < 2., "{start}");
    let at = divider(&mut window);
    window.click_at(at);
    window.keys("right");
    assert!(
        (width(&mut window) - (start + 16.)).abs() < 1.,
        "{}",
        width(&mut window)
    );
    window.keys("shift-right");
    assert!(
        (width(&mut window) - (start + 80.)).abs() < 1.,
        "{}",
        width(&mut window)
    );
    window.keys("home");
    assert!(
        (width(&mut window) - 100.).abs() < 2.,
        "{}",
        width(&mut window)
    );
    window.keys("end");
    assert!(
        (width(&mut window) - 400.).abs() < 2.,
        "{}",
        width(&mut window)
    );
    let at = divider(&mut window);
    for event in 1..=2 {
        window.cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
            first_mouse: false,
        });
        window.cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
        });
    }
    window.settle();
    assert!(
        (width(&mut window) - start).abs() < 2.,
        "{}",
        width(&mut window)
    );
}

/// The same split in a dialog with a backdrop.
const DIALOG: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	view = function(state)
		return shell(state, ui.overlay { open = true, backdrop = true, width = 600, height = 300,
			content = ui.hsplit { id = "people",
				ui.pane { size = 200, min = 100, max = 400, ui.column { class = "list", grow = true, ui.text { "List" } } },
				ui.pane { ui.column { class = "person", grow = true, ui.text { "Person" } } },
			},
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_divider_in_a_dialog_takes_the_keys(cx: &mut TestAppContext) {
    let mut window = open(cx, DIALOG);
    let start = width(&mut window);
    let at = divider(&mut window);
    window.click_at(at);
    window.keys("right");
    assert!(
        (width(&mut window) - (start + 16.)).abs() < 1.,
        "{start} then {}",
        width(&mut window)
    );
}

/// The split telling its mod the sizes the user leaves it at.
const TOLD: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		logged(state, `{msg.msg} {msg.sizes[1]}`)
	end,
	view = function(state)
		return shell(state, ui.hsplit { id = "people", resize = "sized",
			ui.pane { size = 200, min = 100, max = 400, ui.column { class = "list", grow = true, ui.text { "List" } } },
			ui.pane { ui.column { class = "person", grow = true, ui.text { "Person" } } },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_divider_moved_by_a_key_tells_the_mod_the_sizes(cx: &mut TestAppContext) {
    let mut window = open(cx, TOLD);
    let at = divider(&mut window);
    window.click_at(at);
    window.keys("right");
    let start = width(&mut window).round() as i64;
    assert_eq!(window.log(), format!("|sized {start}"));
}

#[gpui_kit::test]
fn a_split_gone_lets_go_of_its_dividers(cx: &mut TestAppContext) {
    let source = r##"
return mod {
	init = function() return { log = "", shown = true } end,
	update = function(state, msg) state.shown = false end,
	view = function(state)
		return shell(state, ui.column { grow = true,
			ui.row { class = "close", msg = "close", ui.text { "Close" } },
			state.shown and ui.hsplit { id = "people",
				ui.pane { size = 200, ui.text { "List" } },
				ui.pane { ui.text { "Person" } },
			},
		})
	end,
}
"##;
    let mut window = open(cx, source);
    assert_eq!(crate::render::dividers_kept(), 1);
    window.click("close");
    assert_eq!(crate::render::dividers_kept(), 0);
}

#[gpui_kit::test]
fn a_double_click_puts_back_the_start_when_the_mod_keeps_the_sizes(cx: &mut TestAppContext) {
    let source = r##"
return mod {
	init = function() return { log = "" } end,
	update = function(state, msg) state.width = msg.sizes[1] end,
	view = function(state)
		return shell(state, ui.hsplit { id = "people", resize = "sized",
			ui.pane { size = state.width or 200, min = 100, max = 400,
				ui.column { class = "list", grow = true, ui.text { "List" } } },
			ui.pane { ui.column { class = "person", grow = true, ui.text { "Person" } } },
		})
	end,
}
"##;
    let mut window = open(cx, source);
    let start = width(&mut window);
    let at = divider(&mut window);
    window.click_at(at);
    window.keys("shift-right");
    assert!(
        (width(&mut window) - (start + 64.)).abs() < 1.,
        "{}",
        width(&mut window)
    );
    let at = divider(&mut window);
    for event in 1..=2 {
        window.cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
            first_mouse: false,
        });
        window.cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
        });
    }
    window.settle();
    assert!(
        (width(&mut window) - start).abs() < 2.,
        "{}",
        width(&mut window)
    );
}

#[gpui_kit::test]
fn a_double_click_puts_a_pane_back_to_its_reset_size(cx: &mut TestAppContext) {
    // A notes app's sidebar opens at 256 by default; the user's 320 was
    // kept from the last launch and is passed back as its `size`.
    let source = r##"
return mod {
	init = function() return { log = "", width = 320 } end,
	update = function(state, msg) state.width = msg.sizes[1] end,
	view = function(state)
		return shell(state, ui.hsplit { id = "notes", resize = "sized",
			ui.pane { size = state.width, reset = 256, min = 100, max = 500,
				ui.column { class = "list", grow = true, ui.text { "Notes" } } },
			ui.pane { ui.column { class = "person", grow = true, ui.text { "Note" } } },
		})
	end,
}
"##;
    let mut window = open(cx, source);
    let start = width(&mut window);
    assert!((start - 320.).abs() < 2., "{start}");
    let at = divider(&mut window);
    for event in 1..=2 {
        window.cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
            first_mouse: false,
        });
        window.cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
        });
    }
    window.settle();
    assert!(
        (width(&mut window) - 256.).abs() < 2.,
        "{}",
        width(&mut window)
    );
}

#[gpui_kit::test]
fn a_double_click_puts_a_trailing_pane_back_to_its_reset_size(cx: &mut TestAppContext) {
    // Both panes come back at kept sizes; only the right sidebar has a
    // `reset`, which a double-click goes back to over the left's start.
    let source = r##"
return mod {
	init = function() return { log = "" } end,
	update = function() end,
	view = function(state)
		return shell(state, ui.column { width = 920, height = 200,
			ui.hsplit { id = "notes", resize = "sized",
				ui.pane { size = 600, ui.column { class = "list", grow = true, ui.text { "Notes" } } },
				ui.pane { size = 320, reset = 256, min = 100,
					ui.column { class = "person", grow = true, ui.text { "Note" } } },
			},
		})
	end,
}
"##;
    let mut window = open(cx, source);
    let side = |window: &mut Window<'_>| f32::from(window.bounds("person").size.width);
    assert!(
        (side(&mut window) - 320.).abs() < 2.,
        "{}",
        side(&mut window)
    );
    let at = divider(&mut window);
    for event in 1..=2 {
        window.cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
            first_mouse: false,
        });
        window.cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: event,
        });
    }
    window.settle();
    assert!(
        (side(&mut window) - 256.).abs() < 2.,
        "{}",
        side(&mut window)
    );
}
