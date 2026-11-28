//! The rows of a scrolling column keep their height and scroll, rather
//! than squeeze into its box and draw over each other.

use super::*;

const LIST: &str = r##"
return mod {
	init = function() return { log = "" } end,
	view = function(state)
		local rows = {}
		for i = 1, 30 do
			table.insert(rows, ui.row { class = `line-{i}`, padding = "small", ui.text { `line {i}` } })
		end
		return shell(state, ui.column { height = 120, scroll = true, table.unpack(rows) })
	end,
}
"##;

#[gpui_kit::test]
fn the_rows_of_a_scrolling_column_keep_their_height(cx: &mut TestAppContext) {
    let mut window = open(cx, LIST);
    let (first, second) = (window.bounds("line-1"), window.bounds("line-2"));
    assert!(first.size.height > px(16.), "squeezed: {first:?}");
    assert!(
        second.origin.y >= first.origin.y + first.size.height,
        "{first:?} {second:?}"
    );
}
