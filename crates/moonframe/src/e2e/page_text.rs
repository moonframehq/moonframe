//! `app:text`: a mod reads what another draws, and the text the user last
//! selected, still known after a click elsewhere clears it on screen.

use {super::*, gpui_kit::MouseButton};

const PAGE: &str = r##"
return mod {
	init = function() return { log = "" } end,
	update = function(state, msg)
		if msg == "ask" then
			return state, { { to = "app:text", msg = { mod = "shell", reply = "page" } } }
		elseif type(msg) == "table" and msg.msg == "page" then
			local selection = msg.selection or {}
			local page = if string.find(msg.text, "Alpha beta", 1, true) then "page" else "none"
			logged(state, `{page} sel={selection.text or ""} in={selection.mod or ""}`)
		end
	end,
	view = function(state)
		return shell(state, ui.column {
			ui.column { class = "doc", width = 400, ui.markdown { text = "Alpha beta gamma delta" } },
			ui.row { class = "ask", msg = "ask", ui.text "Ask" },
		})
	end,
}
"##;

#[gpui_kit::test]
fn a_mod_reads_the_page_and_the_last_selection(cx: &mut TestAppContext) {
    let mut window = open(cx, PAGE);
    // Select the start of the document, then let go.
    let doc = window.bounds("markdown");
    let y = doc.center().y;
    let (from, to) = (
        point(doc.left() + px(10.), y),
        point(doc.left() + px(100.), y),
    );
    let none = Modifiers::none();
    window.cx.simulate_mouse_move(from, None, none);
    window.cx.simulate_mouse_down(from, MouseButton::Left, none);
    window.settle();
    window
        .cx
        .simulate_mouse_move(to, Some(MouseButton::Left), none);
    window.settle();
    window.cx.simulate_mouse_up(to, MouseButton::Left, none);
    window.settle();
    window.click("ask");
    let log = window.log();
    assert_eq!(log, "|page sel=lpha beta g in=shell");
    // Kept after a click elsewhere, which clears it on screen.
    window.click("ask");
    assert_eq!(window.log(), format!("{log}{log}"));
}
