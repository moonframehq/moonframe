//! A paste of copied files reaches the field's mod as paths, and with no
//! field in focus the page's mod; a paste of text still types.

use {
    super::*,
    gpui_kit::{ClipboardEntry, ClipboardItem, ExternalPaths},
    std::path::PathBuf,
};

const COMPOSER: &str = r##"
return mod {
	paste = "page",
	init = function()
		return { log = "", focus = false }
	end,
	update = function(state, msg)
		if msg == "focus" then
			state.focus = true
		elseif msg.msg == "typed" then
			logged(state, `typed {msg.value}`)
		else
			logged(state, `{msg.msg} {table.concat(msg.files, ",")}`)
		end
	end,
	view = function(state)
		return shell(state, ui.column {
			ui.row { class = "focus", msg = "focus", ui.text { "Write" } },
			ui.input { id = "composer", paste = "attach", change = "typed", focus = state.focus },
		})
	end,
}
"##;

/// ⌘V on macOS, Ctrl-V elsewhere.
const PASTE: &str = if cfg!(target_os = "macos") {
    "cmd-v"
} else {
    "ctrl-v"
};

fn copy_files(window: &mut Window<'_>, paths: &[&str]) {
    let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    window.cx.write_to_clipboard(ClipboardItem {
        entries: vec![ClipboardEntry::ExternalPaths(ExternalPaths(paths.into()))],
    });
}

#[gpui_kit::test]
fn pasted_files_reach_the_field_or_the_page(cx: &mut TestAppContext) {
    let mut window = open(cx, COMPOSER);
    copy_files(&mut window, &["/tmp/notes.pdf", "/tmp/plan.key"]);
    // No field has focus: the page's mod hears it.
    window.keys(PASTE);
    assert_eq!(window.log(), "|page /tmp/notes.pdf,/tmp/plan.key");
    window.click("focus");
    window.keys(PASTE);
    assert_eq!(
        window.log(),
        "|page /tmp/notes.pdf,/tmp/plan.key|attach /tmp/notes.pdf,/tmp/plan.key"
    );
    // Text pastes as text.
    window
        .cx
        .write_to_clipboard(ClipboardItem::new_string("hello".to_owned()));
    window.keys(PASTE);
    assert!(window.log().ends_with("|typed hello"), "{}", window.log());
}
