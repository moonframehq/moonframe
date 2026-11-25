//! An open sheet's plain-letter key: T closes it while nothing types, and
//! types a T while its field has focus.

use super::*;

const SHEET: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		if type(msg) == "table" then
			logged(state, `typed {msg.value}`)
		else
			logged(state, msg)
			state.focus = true
		end
	end,
	view = function(state)
		return shell(state, ui.overlay { open = true, place = "right", width = 300, keys = { t = "close" },
			content = ui.column { class = "sheet",
				ui.input { id = "note", change = "note", focus = state.focus },
			} })
	end,
}
"##;

#[gpui_kit::test]
fn an_overlay_takes_a_letter_only_while_nothing_types(cx: &mut TestAppContext) {
    let mut window = open(cx, SHEET);
    window.keys("t");
    assert_eq!(window.log(), "|close");
    // Closing gave the field focus: now T is typed there.
    window.keys("t");
    assert_eq!(window.log(), "|close|typed t");
}
