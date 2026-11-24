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

#[gpui_kit::test]
fn a_link_in_a_note_shows_the_mods_tooltip(cx: &mut TestAppContext) {
    let text = "> [!NOTE]\n> [the report](https://wrapped.example/?u=1) and more";
    let given = r#"{ ["https://wrapped.example/?u=1"] = "Leads to report.example" }"#;
    let mut window = open(cx, &document(text, given));
    // The note's title comes first; the link starts its body, under it.
    let doc = window.bounds("doc");
    let shown = (1..20).any(|step| {
        let at = point(doc.left() + px(24.), doc.top() + px(4. * step as f32));
        window.cx.simulate_mouse_move(at, None, Modifiers::none());
        window.settle();
        window.shows("markdown-link-tooltip")
    });
    assert!(shown, "no tooltip over the note's link");
}

#[gpui_kit::test]
fn a_title_gone_from_the_text_is_gone_from_its_tooltip(cx: &mut TestAppContext) {
    let titled = r#"[the report](https://wrapped.example/?u=1 "Old") and more"#;
    let untitled = "[the report](https://wrapped.example/?u=1) and more";
    let source = format!(
        r#"
return mod {{
	init = function() return {{ log = "", titled = true }} end,
	update = function(state) state.titled = false end,
	view = function(state)
		return shell(state, ui.column {{ class = "doc", width = 400,
			ui.markdown {{ text = if state.titled then {titled:?} else {untitled:?} }},
			ui.column {{ class = "drop", msg = "drop", height = 20, ui.text {{ "Drop the title" }} }},
		}})
	end,
}}"#
    );
    let mut window = open(cx, &source);
    assert!(hover_link(&mut window));
    window.click("drop");
    assert!(!hover_link(&mut window));
}
