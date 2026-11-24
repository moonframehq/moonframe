//! A markdown link's tooltip: its title, or what the mod's `tooltips`
//! gives for its address; none for a link with neither.

use super::*;

fn document(text: &str, tooltips: &str) -> String {
    format!(
        r#"
return mod {{
	init = function() return {{ log = "" }} end,
	update = function() end,
	view = function(state)
		return shell(state, ui.column {{ class = "doc", width = 400,
			ui.markdown {{ text = {text:?}, tooltips = {tooltips} }},
		}})
	end,
}}"#
    )
}

/// Rests the pointer on the document's first words, the link.
fn hover_link(window: &mut Window) -> bool {
    let doc = window.bounds("doc");
    let at = point(doc.left() + px(8.), doc.top() + px(8.));
    window.cx.simulate_mouse_move(at, None, Modifiers::none());
    window.settle();
    window.shows("markdown-link-tooltip")
}

#[gpui_kit::test]
fn a_links_title_shows_as_its_tooltip(cx: &mut TestAppContext) {
    let text = r#"[the report](https://wrapped.example/?u=1 "Leads to report.example") and more"#;
    let mut window = open(cx, &document(text, "nil"));
    assert!(hover_link(&mut window));
}

#[gpui_kit::test]
fn the_mod_can_give_a_links_tooltip(cx: &mut TestAppContext) {
    let text = "[the report](https://wrapped.example/?u=1) and more";
    let given = r#"{ ["https://wrapped.example/?u=1"] = "Leads to report.example" }"#;
    let mut window = open(cx, &document(text, given));
    assert!(hover_link(&mut window));
}

#[gpui_kit::test]
fn a_link_with_neither_shows_none(cx: &mut TestAppContext) {
    let text = "[the report](https://wrapped.example/?u=1) and more";
    let mut window = open(cx, &document(text, "nil"));
    assert!(!hover_link(&mut window));
}
