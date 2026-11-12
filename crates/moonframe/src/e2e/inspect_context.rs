//! A pick tells an agent more than where an element was built: what a
//! click on it sends, and the live state of the mod that drew it, kept
//! current while it stays selected.

use {super::*, crate::inspect, moonframe_core::Data};

/// A shell whose state holds a count and a label, with a button to bump it.
const SHELL: &str = r#"
return mod {
	init = function() return { log = "", count = 7, label = "hello" } end,
	update = function(state, msg)
		if type(msg) == "table" and msg.bump then state.count += msg.bump
		else logged(state, tostring(msg)) end
	end,
	view = function(state)
		return shell(state, ui.row { class = "target",
			ui.button { `Bump {state.count}`, msg = { bump = 1 } } })
	end,
}"#;

#[gpui_kit::test]
fn a_pick_carries_the_live_state_and_what_a_click_sends(cx: &mut TestAppContext) {
    let mut window = open(cx, SHELL);
    let app = window.app.clone();
    app.update(window.cx, |app, cx| app.toggle_inspect(cx));
    window.settle();
    window.click("target");
    let report = app.read_with(window.cx, |app, _| {
        let selected = app.inspector.selected.as_ref().expect("a pick");
        inspect::report(selected, &app.workspace)
    });
    assert!(report.contains("count = 7,"), "{report}");
    assert!(report.contains("label = \"hello\","), "{report}");
    assert!(report.contains(" px\n"), "{report}");

    // The saved report follows the state while the element stays selected.
    app.update(window.cx, |app, cx| {
        app.deliver(
            moonframe_core::SHELL,
            Data::map([("bump", Data::Integer(3))]),
            cx,
        );
        app.refresh_selection();
    });
    let saved = app.read_with(window.cx, |app, _| app.inspector.saved.clone());
    assert!(
        saved
            .as_deref()
            .is_some_and(|saved| saved.contains("count = 10,")),
        "{saved:?}"
    );
}

#[gpui_kit::test]
fn a_picked_button_says_what_its_click_sends(cx: &mut TestAppContext) {
    let mut window = open(cx, SHELL);
    let app = window.app.clone();
    app.update(window.cx, |app, cx| app.toggle_inspect(cx));
    window.settle();
    // The button sits at the row's left end; the row spans the window.
    let row = window.bounds("target");
    window.click_at(row.origin + point(px(12.), row.size.height / 2.));
    let report = app.read_with(window.cx, |app, _| {
        app.inspector
            .leaf
            .as_ref()
            .map(|leaf| inspect::report(leaf, &app.workspace))
    });
    let report = report.expect("a pick");
    assert!(
        report.contains("a click sends { bump = 1, } to its mod's update"),
        "{report}"
    );
}
