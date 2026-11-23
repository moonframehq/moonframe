//! A password field: the mod hears what is typed, the clipboard never does.

use super::*;

const SIGN_IN: &str = r##"
return mod {
	init = function()
		return { log = "" }
	end,
	update = function(state, msg)
		logged(state, `typed {msg.value}`)
	end,
	view = function(state)
		return shell(state, ui.input { id = "password", secret = true, focus = true, change = "typed" })
	end,
}
"##;

#[gpui_kit::test]
fn a_secret_field_sends_its_text_but_never_copies_it(cx: &mut TestAppContext) {
    let mut window = open(cx, SIGN_IN);
    window.typed("hunter2");
    assert!(window.log().ends_with("|typed hunter2"), "{}", window.log());
    window.keys("cmd-a cmd-c");
    let copied = window
        .cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .unwrap_or_default();
    assert!(!copied.contains("hunter2"), "{copied}");
}
