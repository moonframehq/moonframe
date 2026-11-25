//! A column with `outside`: a click beyond its box (and beyond the popovers
//! it opened) tells its mod, and still reaches what was clicked, even when
//! the panel closes and the page moves under the pointer.

use {super::*, gpui_kit::MouseButton};

const PANEL: &str = r##"
return mod {
	init = function()
		return { log = "", open = true }
	end,
	update = function(state, msg)
		if msg == "outside" then
			state.open = false
		end
		logged(state, msg)
	end,
	view = function(state)
		return shell(state, ui.row { gap = "large",
			state.open and ui.column { class = "panel", width = 200, height = 100, outside = "outside",
				ui.text { "panel" },
				ui.popover { open = true, side = "bottom",
					content = ui.column { class = "menu", width = 120, height = 200, ui.text { "menu" } },
					ui.text { "anchor" },
				},
			},
			ui.column { class = "page", width = 200, height = 100, msg = "page", ui.text { "page" } },
		})
	end,
}
"##;

impl Window<'_> {
    /// Presses at `from`, lets frames draw, and lets go at `to`.
    fn press_and_release(&mut self, from: Point<Pixels>, to: Point<Pixels>) {
        self.cx.simulate_mouse_move(from, None, Modifiers::none());
        self.cx
            .simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        self.settle();
        self.cx
            .simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        self.settle();
    }
}

#[gpui_kit::test]
fn a_click_outside_a_column_tells_it_and_goes_on(cx: &mut TestAppContext) {
    let mut window = open(cx, PANEL);
    window.click("panel");
    assert_eq!(window.log(), "");
    // Its popover's content floats beyond its box but is inside it.
    let (menu, panel) = (window.bounds("menu"), window.bounds("panel"));
    assert!(!panel.contains(&menu.center()), "{menu:?} in {panel:?}");
    window.click("menu");
    assert_eq!(window.log(), "");
    // A drag that began inside it is not a click outside.
    let page = window.bounds("page").center();
    window.press_and_release(panel.center(), page);
    assert_eq!(window.log(), "");
    // The click closes the panel, which moves the page, and still clicks it.
    window.press_and_release(page, page);
    assert_eq!(window.log(), "|outside|page");
    assert!(!window.shows("panel"));
}
