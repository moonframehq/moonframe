//! A list whose rows each have their own height: each starts where the one
//! before it ends, and scrolling builds the rows that come into view.

use super::*;

/// Two hundred rows, every tenth 60 high and the rest 20, in a list 100
/// high: each block of ten is 240.
const LIST: &str = r##"
local function tall(i) return if i % 10 == 0 then 60 else 20 end
return mod {
	init = function()
		return { log = "" }
	end,
	update = function() end,
	view = function(state)
		return shell(state, ui.column { class = "rows", height = 100,
			ui.list { count = 200, row_height = tall, row = function(i)
				return ui.row { class = `row-{i}`, height = tall(i), ui.text { `row {i}` } }
			end },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_row_of_its_own_height_starts_where_the_last_ended(cx: &mut TestAppContext) {
    let mut window = open(cx, LIST);
    let top = |window: &mut Window, row| f32::from(window.bounds(row).top());
    let tenth = top(&mut window, "row-10");
    assert_eq!(top(&mut window, "row-9"), tenth - 20.0);
    assert_eq!(top(&mut window, "row-11"), tenth + 60.0);
    assert_eq!(f32::from(window.bounds("row-10").size.height), 60.0);
    // Row 43 starts 1000 down: four blocks of 240, then 20 and 20.
    let list = f32::from(window.bounds("rows").top());
    window.wheel("rows", 1000.0);
    assert_eq!(top(&mut window, "row-43"), list);
    assert_eq!(top(&mut window, "row-50"), list + 140.0);
}
