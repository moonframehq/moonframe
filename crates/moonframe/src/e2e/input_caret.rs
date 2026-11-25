//! A field's `caret`: where the caret goes when the mod sets a new value,
//! as a prompt history recalls a line and leaves the caret after it.

use super::*;

/// A text area whose `up` recalls a line; `caret` comes from the test.
fn recalling(caret: &str) -> String {
    field(caret, false)
}

/// The same field; with `echo`, the mod keeps what is typed as its value.
fn field(caret: &str, echo: bool) -> String {
    format!(
        r#"
return mod {{
	init = function() return {{ log = "", text = "", echo = {echo} }} end,
	update = function(state, msg)
		if msg.msg == "recall" then
			state.text = "recalled line"
		elseif msg.msg == "type" then
			if state.echo then state.text = msg.value end
			logged(state, msg.value)
		end
	end,
	view = function(state)
		return shell(state, ui.input {{ id = "ask", multiline = true, focus = true,
			value = state.text, caret = {caret}, change = "type", keys = {{ up = "recall" }} }})
	end,
}}"#
    )
}

#[gpui_kit::test]
fn a_recalled_line_leaves_the_caret_after_it(cx: &mut TestAppContext) {
    let mut window = open(cx, &recalling(r#""end""#));
    window.keys("up");
    window.typed("!");
    assert_eq!(window.log(), "|recalled line!");
}

#[gpui_kit::test]
fn a_caret_can_go_some_bytes_in(cx: &mut TestAppContext) {
    let mut window = open(cx, &recalling("8"));
    window.keys("up");
    window.typed("!");
    assert_eq!(window.log(), "|recalled! line");
}

#[gpui_kit::test]
fn a_mod_echoing_the_text_leaves_the_caret_where_the_user_put_it(cx: &mut TestAppContext) {
    let mut window = open(cx, &field(r#""end""#, true));
    window.typed("ab");
    window.keys("left");
    window.typed("X");
    window.typed("Y");
    let log = window.log();
    assert!(log.ends_with("|aXb|aXYb"), "{log}");
}
