//! A column with `outside`: a press beyond its box (and beyond the popovers
//! it opened) tells its mod, and still reaches what was pressed.

use super::*;

const PANEL: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		logged(state, msg)
	end,
	view = function(state)
		return shell(state, ui.row { gap = "large",
			ui.column { class = "panel", width = 200, height = 100, outside = "outside",
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

#[gpui_kit::test]
fn a_press_outside_a_column_tells_it_and_goes_on(cx: &mut TestAppContext) {
    let mut window = open(cx, PANEL);
    window.click("panel");
    assert_eq!(window.log(), "");
    // Its popover's content floats beyond its box but is inside it.
    let (menu, panel) = (window.bounds("menu"), window.bounds("panel"));
    assert!(!panel.contains(&menu.center()), "{menu:?} in {panel:?}");
    window.click("menu");
    assert_eq!(window.log(), "");
    window.click("page");
    assert_eq!(window.log(), "|outside|page");
}
