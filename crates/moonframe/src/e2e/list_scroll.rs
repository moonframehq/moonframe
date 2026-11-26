//! A long list scrolled from its mod: to a row, back to a `y` its `range`
//! told, and kept on the row being read as rows arrive above.

use {super::*, std::time::Duration};

/// Rows 20 high in a list 100 high, keyed, that grows at the top.
const FEED: &str = r##"
return mod {
	init = function()
		local items = {}
		for i = 1, 1000 do items[i] = `i{i}` end
		return { log = "", items = items, added = 0, seq = 0 }
	end,
	update = function(state, msg)
		if msg == "add" then
			for _ = 1, 3 do
				state.added += 1
				table.insert(state.items, 1, `new{state.added}`)
			end
		elseif msg == "row" then
			state.seq += 1
			state.to = { row = 50, seq = state.seq }
		elseif msg == "top" then
			state.seq += 1
			state.to = { y = 0, seq = state.seq }
		else
			logged(state, `{msg.first}-{msg.last} y={msg.y} {state.items[msg.first]}`)
		end
	end,
	view = function(state)
		return shell(state, ui.column {
			ui.row { class = "add", msg = "add", ui.text "Add" },
			ui.row { class = "row", msg = "row", ui.text "Row" },
			ui.row { class = "top", msg = "top", ui.text "Top" },
			ui.column { class = "box",
				ui.list { id = "feed", count = #state.items, row_height = 20, height = 100, range = "rows",
					scroll_to = state.to,
					row = function(i) return ui.row { key = state.items[i], ui.text { state.items[i] } } end },
			},
		})
	end,
}
"##;

fn later(window: &mut Window<'_>) -> String {
    window
        .cx
        .executor()
        .advance_clock(Duration::from_millis(200));
    window.settle();
    let log = window.log();
    log.split('|').next_back().unwrap_or_default().to_owned()
}

#[gpui_kit::test]
fn a_list_goes_to_a_row_and_back_to_a_y(cx: &mut TestAppContext) {
    let mut window = open(cx, FEED);
    assert_eq!(later(&mut window), "1-5 y=0 i1");
    window.click("row");
    // Row 50 comes up to the bottom edge: 50 * 20 - 100.
    assert_eq!(later(&mut window), "46-50 y=900 i46");
    window.click("top");
    assert_eq!(later(&mut window), "1-5 y=0 i1");
}

#[gpui_kit::test]
fn rows_arriving_above_keep_the_row_being_read_in_place(cx: &mut TestAppContext) {
    let mut window = open(cx, FEED);
    window.wheel("box", 200.0);
    assert_eq!(later(&mut window), "11-15 y=200 i11");
    window.click("add");
    // Three rows came above: the same rows show, three places further down.
    assert_eq!(later(&mut window), "14-18 y=260 i11");
}

#[gpui_kit::test]
fn at_the_top_rows_arriving_join_the_top(cx: &mut TestAppContext) {
    let mut window = open(cx, FEED);
    assert_eq!(later(&mut window), "1-5 y=0 i1");
    let log = window.log();
    window.click("add");
    // Still rows 1 to 5 at the top (the new ones): nothing moved to tell.
    later(&mut window);
    assert_eq!(window.log(), log);
}

#[gpui_kit::test]
fn a_padded_list_brings_the_row_wholly_into_view(cx: &mut TestAppContext) {
    let padded = FEED.replace(
        "height = 100, range",
        "height = 100, style = { padding = { 20, 0, 0, 0 } }, range",
    );
    let mut window = open(cx, &padded);
    later(&mut window);
    window.click("row");
    // Row 50 ends 20 lower, under the padding: 1020 - 100.
    assert_eq!(later(&mut window), "46-50 y=920 i46");
}

/// A list that opens on row 50, with nothing else to draw it again.
const OPENS_ON: &str = r##"
return mod {
	init = function() return { log = "" } end,
	view = function(state)
		return shell(state, ui.column { class = "box",
			ui.list { count = 1000, row_height = 20, height = 100, scroll_to = 50,
				row = function(i) return ui.row { class = if i == 50 then "fifty" else nil, ui.text { `row {i}` } } end },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_list_opens_on_the_row_it_asks_for(cx: &mut TestAppContext) {
    let mut window = open(cx, OPENS_ON);
    let (list, row) = (window.bounds("box"), window.bounds("fifty"));
    assert!(
        row.top() >= list.top() && row.bottom() <= list.bottom() + px(1.),
        "{row:?} {list:?}"
    );
}
