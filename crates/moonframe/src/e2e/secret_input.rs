//! A password field: the mod hears what is typed, the clipboard never does.

use super::*;

/// A field, plain or secret, that logs what is typed.
fn sign_in(secret: bool) -> String {
    format!(
        r##"
return mod {{
	init = function()
		return {{ log = "" }}
	end,
	update = function(state, msg)
		logged(state, `typed {{msg.value}}`)
	end,
	view = function(state)
		return shell(state, ui.input {{ id = "password", secret = {secret}, focus = true, change = "typed" }})
	end,
}}
"##
    )
}

/// Select all, then copy: ⌘ on macOS, Ctrl elsewhere.
const COPY_ALL: &str = if cfg!(target_os = "macos") {
    "cmd-a cmd-c"
} else {
    "ctrl-a ctrl-c"
};

/// Types into the field, copies all of it, and says what the clipboard
/// then holds.
fn copied(cx: &mut TestAppContext, secret: bool) -> String {
    let mut window = open(cx, &sign_in(secret));
    window.typed("hunter2");
    assert!(window.log().ends_with("|typed hunter2"), "{}", window.log());
    window
        .cx
        .write_to_clipboard(gpui_kit::ClipboardItem::new_string("before".to_owned()));
    window.keys(COPY_ALL);
    window
        .cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .unwrap_or_default()
}

#[gpui_kit::test]
fn a_plain_field_copies_what_is_typed(cx: &mut TestAppContext) {
    assert_eq!(copied(cx, false), "hunter2");
}

#[gpui_kit::test]
fn a_secret_field_sends_its_text_but_never_copies_it(cx: &mut TestAppContext) {
    let copied = copied(cx, true);
    assert!(!copied.contains("hunter2"), "{copied}");
}
