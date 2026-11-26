//! `scroll_to` by key: a row inside a card inside a scrolling column comes
//! into view, scrolled just enough, each time it is asked for.

use super::*;

const CARDS: &str = r##"
return mod {
	init = function()
		return { log = "", seq = 0 }
	end,
	update = function(state, msg)
		if msg == "go" then
			state.seq += 1
			state.target = { key = "row-7-3", seq = state.seq }
		end
	end,
	view = function(state)
		local cards = {}
		for i = 1, 10 do
			local rows = {}
			for j = 1, 5 do
				local class = if i == 7 and j == 3 then "target" else nil
				table.insert(rows, ui.row { key = `row-{i}-{j}`, class = class, height = 40, ui.text { `row {i}.{j}` } })
			end
			table.insert(cards, ui.column { key = `card-{i}`, gap = "none", shrink = false, table.unpack(rows) })
		end
		return shell(state, ui.column {
			ui.row { class = "go", msg = "go", ui.text { "Go" } },
			ui.column { class = "cards", height = 200, gap = "none", scroll = true,
				scroll_to = state.target, table.unpack(cards) },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_keyed_row_deep_in_a_scrolling_column_comes_into_view(cx: &mut TestAppContext) {
    let mut window = open(cx, CARDS);
    let list = window.bounds("cards");
    assert!(window.bounds("target").top() > list.bottom());
    window.click("go");
    let target = window.bounds("target");
    // Just enough: it comes up to the bottom edge.
    assert!(
        (f32::from(target.bottom()) - f32::from(list.bottom())).abs() < 1.,
        "{target:?} {list:?}"
    );
    // Scrolled away, then asked again with a new `seq`.
    window.wheel("cards", -600.0);
    assert!(window.bounds("target").top() > list.bottom());
    window.click("go");
    let target = window.bounds("target");
    assert!(
        target.top() >= list.top() && target.bottom() <= list.bottom() + px(1.),
        "{target:?} {list:?}"
    );
    // Redrawn with the same request, the wheel stays free.
    window.wheel("cards", 600.0);
    let target = window.bounds("target");
    assert!(target.bottom() < list.top(), "{target:?} {list:?}");
}
