//! A scrolling column that appears with `scroll_to`, as the content of an
//! overlay opening does, shows that child at once.

use super::*;

/// A sheet of 101 turns that opens on the 35th.
const SHEET: &str = r##"
return mod {
	init = function()
		return { log = "", open = false }
	end,
	update = function(state, msg)
		if msg == "open" then state.open = true end
	end,
	view = function(state)
		local turns = {}
		for i = 1, 101 do
			table.insert(turns, ui.row { class = if i == 35 then "quoted" else "turn", height = 40, shrink = false, ui.text { `turn {i}` } })
		end
		return shell(state, ui.column { grow = true,
			ui.row { class = "opener", msg = "open", ui.text { "Open" } },
			ui.overlay { open = state.open, place = "right", width = 300,
				content = ui.column { class = "turns", scroll = true, grow = true, scroll_to = 35, table.unpack(turns) } },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_column_opening_in_an_overlay_shows_the_child_it_asks_for(cx: &mut TestAppContext) {
    let mut window = open(cx, SHEET);
    window.click("opener");
    let (column, quoted) = (window.bounds("turns"), window.bounds("quoted"));
    assert!(
        quoted.top() >= column.top() && quoted.bottom() <= column.bottom(),
        "turn 35 at {quoted:?}, outside the column at {column:?}"
    );
}
