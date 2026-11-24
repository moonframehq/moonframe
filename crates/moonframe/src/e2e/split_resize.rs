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

#[gpui_kit::test]
fn letting_go_of_a_divider_tells_the_mod_the_sizes(cx: &mut TestAppContext) {
    let mut window = open(cx, SPLIT);
    let first = window.bounds("first");
    let boundary = window.bounds("second").left();
    let y = first.center().y;
    let to = first.left() + px(220.);
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
    let log = window.log();
    assert!(log.starts_with("|sized=220,"), "{log}");
    assert_eq!(log.matches('|').count(), 1, "sent once: {log}");
}
