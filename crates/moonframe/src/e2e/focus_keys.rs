//! A column's own keys, taken while focus is inside it: a decision deck
//! whose arrows dismiss and keep only once the reader is in it, ahead of
//! the page's keys, and never while its note field has focus.

use super::*;

const DECK: &str = r##"
return mod {
	keys = { right = "page" },
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		if type(msg) == "table" then
			logged(state, `typed {msg.value}`)
		else
			logged(state, msg)
			if msg == "note" then state.focus = true end
		end
	end,
	view = function(state)
		return shell(state, ui.column { class = "deck", keys = { left = "dismiss", right = "keep" },
			ui.row { class = "card", height = 40, ui.text { "Follow up with Ana" } },
			ui.row { class = "note", msg = "note", ui.text { "Note" } },
			ui.input { id = "note", change = "typed", focus = state.focus },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_column_takes_its_keys_while_focus_is_inside_it(cx: &mut TestAppContext) {
    let mut window = open(cx, DECK);
    // Focus is elsewhere: the page's keys.
    window.keys("right");
    assert_eq!(window.log(), "|page");
    window.click("card");
    window.keys("right");
    window.keys("left");
    assert_eq!(window.log(), "|page|keep|dismiss");
    // Its field has focus: the arrows move the caret there.
    window.click("note");
    window.keys("left");
    assert_eq!(window.log(), "|page|keep|dismiss|note");
}

/// The deck in a dialog with a backdrop.
const DIALOG: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		logged(state, msg)
	end,
	view = function(state)
		return shell(state, ui.overlay { open = true, backdrop = true, width = 400, height = 200,
			content = ui.column { class = "deck", keys = { left = "dismiss", right = "keep" },
				ui.row { class = "card", height = 40, ui.text { "Follow up with Ana" } },
			},
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_column_in_a_dialog_takes_its_keys(cx: &mut TestAppContext) {
    let mut window = open(cx, DIALOG);
    window.click("card");
    window.keys("right");
    assert_eq!(window.log(), "|keep");
}
