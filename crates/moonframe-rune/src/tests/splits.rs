//! Showing and hiding a split's panes.

use super::*;

#[test]
fn hiding_a_pane_moves_the_others_rather_than_fading_them() {
    let main = r#"
return mod {
	init = function(previous) return previous or { left = true } end,
	update = function(state, msg)
		if msg == "toggle" then state.left = not state.left end
	end,
	view = function(state)
		return ui.column {
			grow = true,
			ui.button { "Toggle", msg = "toggle" },
			ui.hsplit {
				id = "body",
				ui.pane { size = 200, visible = state.left, ui.text { "left" } },
				ui.pane { ui.text { "right" } },
			},
		}
	end,
}
"#;
    let dir = folder(main);
    let mut window = open(dir.path());
    let right = |window: &Runtime<Ui>| {
        window
            .elements()
            .iter()
            .filter(|element| element.text.as_deref() == Some("right"))
            .map(|element| (element.rect, element.exiting))
            .collect::<Vec<_>>()
    };
    let before = right(&window);
    assert_eq!(before.len(), 1);
    let toggle = rect_of(&window, |element| element.text.as_deref() == Some("Toggle"));
    let (x, y) = (toggle[0] + toggle[2] / 2., toggle[1] + toggle[3] / 2.);
    window.pointer_down(x, y);
    window.pointer_up(x, y);
    // The frame after the click: no copy of the right pane fading out.
    window.frame(FRAME);
    window.frame(FRAME);
    let after = right(&window);
    assert_eq!(
        after.len(),
        1,
        "one right pane, not a fading copy: {after:?}"
    );
    assert!(!after[0].1, "not on its way out");
    // And it is already where it goes, not springing there.
    settle(&mut window);
    assert_eq!(right(&window)[0].0, after[0].0, "landed at once");
    assert!(
        after[0].0[0] < before[0].0[0],
        "it took the hidden pane's room"
    );
}

#[test]
fn a_child_coming_before_others_leaves_them_as_they_were() {
    // As the tab strip's Show sidebar button: `cond and ...` first.
    let main = r#"
return mod {
	init = function(previous) return previous or { on = false } end,
	update = function(state, msg) if msg == "toggle" then state.on = not state.on end end,
	view = function(state)
		return ui.column {
			ui.button { "Toggle", msg = "toggle" },
			ui.row {
				state.on and ui.button { "Show", msg = "show" },
				ui.spacer(),
				ui.button { "Files", msg = "files" },
				ui.row { key = "tools", ui.text { "Split" } },
			},
		}
	end,
}
"#;
    let dir = folder(main);
    let mut window = open(dir.path());
    let toggle = rect_of(&window, |element| element.text.as_deref() == Some("Toggle"));
    let (x, y) = (toggle[0] + toggle[2] / 2., toggle[1] + toggle[3] / 2.);
    window.pointer_down(x, y);
    window.pointer_up(x, y);
    window.frame(FRAME);
    window.frame(FRAME);
    let fading = |text: &str| {
        window
            .elements()
            .iter()
            .filter(|element| element.text.as_deref() == Some(text))
            .map(|element| (element.opacity, element.exiting))
            .collect::<Vec<_>>()
    };
    assert!(shows(&window, "Show"));
    for text in ["Files", "Split"] {
        let seen = fading(text);
        assert_eq!(seen.len(), 1, "{text}: not a copy fading out: {seen:?}");
        assert!(
            seen[0].0 > 0.99 && !seen[0].1,
            "{text}: not faded: {seen:?}"
        );
    }
}
