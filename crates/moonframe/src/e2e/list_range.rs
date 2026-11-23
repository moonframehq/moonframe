//! A long list that says, with `range`, which of its rows show: what a mod
//! needs to ask a provider for only the rows around them.

use {super::*, std::time::Duration};

/// A thousand rows 20 high in a list 100 high.
const LIST: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		logged(state, `{msg.msg} {msg.first}-{msg.last}`)
	end,
	view = function(state)
		return shell(state, ui.column { class = "box",
			ui.list { id = "people", count = 1000, row_height = 20, height = 100, range = "rows",
				row = function(i) return ui.text { `row {i}` } end },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_list_says_which_rows_show_as_it_scrolls(cx: &mut TestAppContext) {
    let mut window = open(cx, LIST);
    assert_eq!(window.log(), "|rows 1-5");
    window.wheel("box", 200.0);
    window
        .cx
        .executor()
        .advance_clock(Duration::from_millis(200));
    window.settle();
    let log = window.log();
    assert_eq!(log.split('|').next_back(), Some("rows 11-15"), "{log}");
    // Told only of changes: drawing again says nothing new.
    window.settle();
    assert_eq!(window.log(), log);
}
