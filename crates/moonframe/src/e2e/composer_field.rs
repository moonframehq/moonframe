//! A chat's composer: Enter sends and Shift-Enter makes a new line
//! (`enter = "submit"`), and Up and Down step through `history`.

use super::*;

const COMPOSER: &str = r#"
return mod {
	init = function() return { log = "", text = nil } end,
	update = function(state, msg)
		local value = string.gsub(msg.value or "", "\n", "/")
		if msg.msg == "send" then
			logged(state, "send:" .. value)
			state.text = ""
		elseif msg.msg == "type" then
			logged(state, value)
		end
	end,
	view = function(state)
		return shell(state, ui.input { id = "ask", multiline = true, focus = true, enter = "submit",
			value = state.text, change = "type", submit = "send", history = { "first", "second" } })
	end,
}"#;

#[gpui_kit::test]
fn enter_sends_and_shift_enter_makes_a_new_line(cx: &mut TestAppContext) {
    let mut window = open(cx, COMPOSER);
    window.typed("a");
    window.keys("shift-enter");
    window.typed("b");
    window.keys("enter");
    let log = window.log();
    assert!(log.ends_with("|a|a/|a/b|send:a/b"), "{log}");
}

#[gpui_kit::test]
fn up_and_down_recall_what_was_sent_then_the_draft(cx: &mut TestAppContext) {
    let mut window = open(cx, COMPOSER);
    window.typed("d");
    window.keys("up");
    window.keys("up");
    window.keys("down");
    window.keys("down");
    let log = window.log();
    assert!(log.ends_with("|d|second|first|second|d"), "{log}");
    // A recalled entry sends as typed text does.
    window.keys("up");
    window.keys("enter");
    assert!(window.log().ends_with("|second|send:second"));
}
