//! A chat's "latest message" button: the column says when it leaves or
//! reaches its end, and goes back to its end each time it is asked.

use super::*;

const CHAT: &str = r##"
return mod {
	init = function()
		return { log = "", seq = 0 }
	end,
	update = function(state, msg)
		if msg == "latest" then
			state.seq += 1
			state.jump = { seq = state.seq }
		else
			logged(state, `{msg.msg} {msg.at_end}`)
		end
	end,
	view = function(state)
		local lines = {}
		for i = 1, 30 do
			table.insert(lines, ui.row { height = 40, shrink = false, ui.text { `message {i}` } })
		end
		return shell(state, ui.column {
			ui.row { class = "latest", msg = "latest", ui.text { "Latest" } },
			ui.column { class = "chat", height = 200, gap = "none", scroll = true,
				at_end = "end", scroll_end = state.jump, table.unpack(lines) },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_column_says_when_it_leaves_its_end_and_goes_back_when_asked(cx: &mut TestAppContext) {
    let mut window = open(cx, CHAT);
    assert_eq!(window.log(), "|end false");
    window.wheel("chat", 2000.0);
    assert_eq!(window.log(), "|end false|end true");
    window.wheel("chat", -300.0);
    assert_eq!(window.log(), "|end false|end true|end false");
    window.click("latest");
    assert_eq!(window.log(), "|end false|end true|end false|end true");
    // Asked again, with a new value, after scrolling up once more.
    window.wheel("chat", -300.0);
    window.click("latest");
    assert_eq!(
        window.log(),
        "|end false|end true|end false|end true|end false|end true"
    );
}
