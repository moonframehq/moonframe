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

/// The row is inside a scrolling card that waits for a row it does not
/// have yet: the outer column still finds it.
const NESTED: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	view = function(state)
		return shell(state, ui.column { class = "outer", height = 200, gap = "none", scroll = true,
			scroll_to = "wanted",
			ui.row { height = 600, ui.text { "above" } },
			ui.column { height = 150, gap = "none", shrink = false, scroll = true, scroll_to = "missing",
				ui.row { key = "first", height = 40, ui.text { "first" } },
				ui.row { key = "wanted", class = "wanted", height = 40, ui.text { "wanted" } },
			},
		})
	end,
}
"##;

#[gpui_kit::test]
fn an_inner_request_does_not_hide_the_outer_one(cx: &mut TestAppContext) {
    let mut window = open(cx, NESTED);
    let (outer, wanted) = (window.bounds("outer"), window.bounds("wanted"));
    assert!(
        wanted.top() >= outer.top() && wanted.bottom() <= outer.bottom() + px(1.),
        "{wanted:?} {outer:?}"
    );
}

/// A card growing in before the row as it is asked for: the column follows
/// the row until the card is in place.
const GROWING: &str = r##"
return mod {
	init = function()
		return { log = "", added = false }
	end,
	update = function(state, msg)
		if msg == "go" then
			state.added = true
			state.target = { key = "wanted", seq = 1 }
		end
	end,
	styles = { [".grow-in"] = { transition = "slide 200ms linear" } },
	view = function(state)
		local rows = {}
		for i = 1, 6 do
			table.insert(rows, ui.row { key = `after-{i}`, height = 40, shrink = false, ui.text { `after {i}` } })
		end
		return shell(state, ui.column {
			ui.row { class = "go", msg = "go", ui.text { "Go" } },
			ui.column { class = "growing", height = 200, gap = "none", scroll = true, scroll_to = state.target,
				ui.row { key = "first", height = 40, shrink = false, ui.text { "first" } },
				state.added and ui.row { key = "card", class = "grow-in", height = 300, shrink = false, ui.text { "card" } },
				ui.row { key = "wanted", class = "wanted", height = 40, shrink = false, ui.text { "wanted" } },
				table.unpack(rows),
			},
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_reveal_follows_its_row_while_a_card_grows_in_before_it(cx: &mut TestAppContext) {
    let mut window = open(cx, GROWING);
    window.click("go");
    for _ in 0..16 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        window.settle();
    }
    let (list, wanted) = (window.bounds("growing"), window.bounds("wanted"));
    assert!(
        wanted.top() >= list.top() && wanted.bottom() <= list.bottom() + px(1.),
        "{wanted:?} {list:?}"
    );
}

/// A spinner in the column animates every frame without moving any row:
/// the reveal still ends, and the wheel is free after it.
const SPINNING: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		if msg == "go" then
			state.target = { key = "wanted", seq = 1 }
		end
	end,
	view = function(state)
		local rows = {}
		for i = 1, 10 do
			table.insert(rows, ui.row { key = `row-{i}`, height = 40, shrink = false, ui.text { `row {i}` } })
		end
		table.insert(rows, ui.row { key = "wanted", class = "wanted", height = 40, shrink = false, ui.text { "wanted" } })
		return shell(state, ui.column {
			ui.row { class = "go", msg = "go", ui.text { "Go" } },
			ui.column { class = "spinning", height = 200, gap = "none", scroll = true, scroll_to = state.target,
				ui.row { height = 40, shrink = false, ui.icon { "loader", spin = true } },
				table.unpack(rows),
			},
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_spinner_does_not_hold_the_scroll_after_a_reveal(cx: &mut TestAppContext) {
    let mut window = open(cx, SPINNING);
    window.click("go");
    let list = window.bounds("spinning");
    assert!(window.bounds("wanted").bottom() <= list.bottom() + px(1.));
    window.wheel("spinning", -400.0);
    for _ in 0..4 {
        window.settle();
    }
    let wanted = window.bounds("wanted");
    assert!(wanted.top() > list.bottom(), "{wanted:?} {list:?}");
}
