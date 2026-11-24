//! A split's `resize`: letting go of a divider tells the mod each pane's
//! size, for it to keep and pass back as the panes' `size` next launch.

use {super::*, gpui_kit::MouseButton};

const SPLIT: &str = r##"
return mod {
	init = function() return { log = "" } end,
	update = function(state, msg)
		logged(state, `{msg.msg}={table.concat(msg.sizes, ",")}`)
	end,
	view = function(state)
		return shell(state, ui.column { width = 400, height = 100,
			ui.hsplit { id = "panes", resize = "sized",
				ui.pane { size = 150, ui.column { class = "first", grow = true } },
				ui.pane { ui.column { class = "second", grow = true } },
			},
		})
	end,
}
"##;

/// Drags the divider after `first` until that pane is `width` wide.
fn drag_divider(window: &mut Window, width: f32) {
    let first = window.bounds("first");
    let boundary = window.bounds("second").left();
    let y = first.center().y;
    let to = first.left() + px(width);
    let cx = &mut window.cx;
    cx.simulate_mouse_down(
        point(boundary - px(2.), y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(
        point(boundary + px(10.), y),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    cx.simulate_mouse_move(point(to, y), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(point(to, y), MouseButton::Left, Modifiers::none());
    window.settle();
}

#[gpui_kit::test]
fn letting_go_of_a_divider_tells_the_mod_the_sizes(cx: &mut TestAppContext) {
    let mut window = open(cx, SPLIT);
    drag_divider(&mut window, 220.);
    let log = window.log();
    assert!(log.starts_with("|sized=220,"), "{log}");
    assert_eq!(log.matches('|').count(), 1, "sent once: {log}");
}

/// Keeps the sizes a split reports and draws the split from them; `reopen`
/// draws it afresh under another id, as the next launch would.
const KEPT: &str = r##"
return mod {
	init = function() return { log = "", again = false } end,
	update = function(state, msg)
		if msg == "reopen" then
			state.again = true
		else
			state.sizes = msg.sizes
			logged(state, `{msg.msg}={table.concat(msg.sizes, ",")}`)
		end
	end,
	view = function(state)
		local first = if state.sizes then state.sizes[1] else 150
		return shell(state, ui.column { width = 400, height = 120,
			ui.column { class = "reopen", height = 20, msg = "reopen", ui.text { "Reopen" } },
			ui.hsplit { id = if state.again then "again" else "panes", resize = "sized",
				ui.pane { size = first, ui.column { class = "first", grow = true } },
				ui.pane { ui.column { class = "second", grow = true } },
			},
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_split_drawn_from_the_kept_sizes_opens_as_it_was_left(cx: &mut TestAppContext) {
    let mut window = open(cx, KEPT);
    drag_divider(&mut window, 220.);
    assert!(window.log().starts_with("|sized=220,"), "{}", window.log());
    window.click("reopen");
    let width = f32::from(window.bounds("first").size.width);
    assert!((width - 220.).abs() <= 1., "the first pane is {width} wide");
}
