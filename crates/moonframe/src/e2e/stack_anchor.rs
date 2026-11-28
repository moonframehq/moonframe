//! A scrolling column keeps what is being read in place: scrolled down, its
//! first keyed row showing stays where it is on screen as rows arrive above
//! it; at the top, arrivals join the top.

use super::*;

const FEED: &str = r##"
return mod {
	init = function()
		local items = {}
		for i = 1, 30 do items[i] = `i{i}` end
		return { log = "", items = items, added = 0 }
	end,
	update = function(state, msg)
		if msg == "add" then
			for _ = 1, 3 do
				state.added += 1
				table.insert(state.items, 1, `new{state.added}`)
			end
		end
	end,
	view = function(state)
		local rows = {}
		for _, item in state.items do
			table.insert(rows, ui.row { key = item, class = if item == "i12" then "target" else nil,
				height = 40, ui.text { item } })
		end
		return shell(state, ui.column {
			ui.row { class = "add", msg = "add", ui.text "Add" },
			ui.column { class = "feed", height = 200, gap = "none", scroll = true, table.unpack(rows) },
		})
	end,
}
"##;

#[gpui_kit::test]
fn rows_arriving_above_leave_the_row_being_read_where_it_is(cx: &mut TestAppContext) {
    let mut window = open(cx, FEED);
    window.wheel("feed", 420.0);
    let before = window.bounds("target");
    window.click("add");
    let after = window.bounds("target");
    assert!(
        (f32::from(after.top()) - f32::from(before.top())).abs() < 1.,
        "{before:?} {after:?}"
    );
    // Read on, and more arrive: it stays where the wheel put it.
    window.wheel("feed", 30.0);
    let read_on = window.bounds("target");
    window.click("add");
    let after = window.bounds("target");
    assert!(
        (f32::from(after.top()) - f32::from(read_on.top())).abs() < 1.,
        "{read_on:?} {after:?}"
    );
}

#[gpui_kit::test]
fn at_the_top_rows_arriving_join_the_top(cx: &mut TestAppContext) {
    let mut window = open(cx, FEED);
    let before = window.bounds("target");
    window.click("add");
    let after = window.bounds("target");
    // Three rows of 40 came above it.
    assert!(
        (f32::from(after.top()) - f32::from(before.top()) - 120.).abs() < 1.,
        "{before:?} {after:?}"
    );
}
