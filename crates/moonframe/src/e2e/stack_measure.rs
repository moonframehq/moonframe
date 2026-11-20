//! A scrolling list that says, with `measure`, how far it is scrolled and
//! which of its rows show: what a list needs to keep where it was read.

use super::*;

/// Twenty rows 40 high in a list 100 high: 800 of content.
const LIST: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		logged(state, `y={msg.y} first={msg.first} last={msg.last} fit={msg.fit} content={msg.content_height} height={msg.height}`)
	end,
	view = function(state)
		local rows = {}
		for i = 1, 20 do
			table.insert(rows, ui.row { height = 40, shrink = false, ui.text { `row {i}` } })
		end
		return shell(state, ui.column { class = "list", height = 100, gap = "none", scroll = true, measure = "list", table.unpack(rows) })
	end,
}
"##;

#[gpui_kit::test]
fn a_list_says_how_far_it_is_scrolled_and_which_rows_show(cx: &mut TestAppContext) {
    let mut window = open(cx, LIST);
    let log = window.log();
    assert_eq!(
        log.split('|').next_back(),
        Some("y=0 first=1 last=3 fit=2 content=800 height=100"),
        "{log}"
    );
    window.wheel("list", 200.0);
    let log = window.log();
    assert_eq!(
        log.split('|').next_back(),
        Some("y=200 first=6 last=8 fit=2 content=800 height=100"),
        "{log}"
    );
    // Told only of changes: drawing again says nothing new.
    let lines = log.split('|').count();
    window.settle();
    assert_eq!(window.log().split('|').count(), lines, "{}", window.log());
}
