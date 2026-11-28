//! A scrolling column put back where `measure` told it was, with
//! `scroll_to = { y, seq }`: a reader reopened where it was left.

use super::*;

/// Twenty rows 40 high in a column 100 high: 800 of content.
const READER: &str = r##"
return mod {
	init = function()
		return { log = "", seq = 0, y = 0 }
	end,
	update = function(state, msg)
		if msg == "back" then
			state.seq += 1
			state.to = { y = state.kept, seq = state.seq }
		elseif msg == "keep" then
			state.kept = state.y
		elseif msg == "far" then
			state.seq += 1
			state.to = { y = 5000, seq = state.seq }
		else
			state.y = msg.y
			logged(state, `y={msg.y}`)
		end
	end,
	view = function(state)
		local rows = {}
		for i = 1, 20 do
			table.insert(rows, ui.row { height = 40, ui.text { `row {i}` } })
		end
		return shell(state, ui.column {
			ui.row { class = "keep", msg = "keep", ui.text { "Keep" } },
			ui.row { class = "back", msg = "back", ui.text { "Back" } },
			ui.row { class = "far", msg = "far", ui.text { "Far" } },
			ui.column { class = "reader", height = 100, gap = "none", scroll = true, measure = "read",
				scroll_to = state.to, table.unpack(rows) },
		})
	end,
}
"##;

fn last(window: &mut Window<'_>) -> String {
    let log = window.log();
    log.rsplit('|').next().unwrap_or_default().to_owned()
}

#[gpui_kit::test]
fn a_column_goes_back_to_the_y_it_told(cx: &mut TestAppContext) {
    let mut window = open(cx, READER);
    window.wheel("reader", 240.0);
    assert_eq!(last(&mut window), "y=240");
    window.click("keep");
    window.wheel("reader", -200.0);
    assert_eq!(last(&mut window), "y=40");
    window.click("back");
    assert_eq!(last(&mut window), "y=240");
    // Past the end: as far as the content goes.
    window.click("far");
    assert_eq!(last(&mut window), "y=700");
}
