//! `scrolled` and `scroll_at` on a scrolling document, as a preview that
//! follows its source.

use super::*;

/// A document that is short until its mod is sent `go`, which gives it its
/// whole text and asks for its end in the same update.
const GROWS: &str = r###"
local lines = {}
table.insert(lines, "# First")
for i = 1, 40 do table.insert(lines, "Line " .. i .. " of the first part.") end
table.insert(lines, "## Second")
for i = 1, 40 do table.insert(lines, "Line " .. i .. " of the second part.") end
local long = table.concat(lines, "\n\n")
return mod {
	init = function() return { heading = 0, told = 0, go = false } end,
	update = function(state, msg)
		if msg.heading then state.heading = msg.heading end
		if msg.scrolled then state.told += 1 end
		if msg.go then state.go = true end
		return state
	end,
	view = function(state)
		return ui.column {
			grow = true,
			ui.text { `at {state.heading}, told {state.told}` },
			ui.markdown {
				text = if state.go then long else "# First\n\nShort.",
				height = 200,
				scroll = true,
				section = { heading = true },
				scrolled = { scrolled = true },
				scroll_at = if state.go then { fraction = 1, seq = 1 } else nil,
			},
		}
	end,
}"###;

#[test]
fn a_scroll_at_that_comes_with_new_text_waits_for_its_layout() {
    let dir = folder(GROWS);
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (600., 400.),
        Env::new(true, 2.),
    );
    run(&mut window);
    window.update(|ui, _| {
        ui.send(
            moonframe_core::SHELL,
            moonframe_core::SHELL,
            Data::map([("go", true.into())]),
        );
    });
    run(&mut window);
    // At the end of the new text, not the top the old one had room for,
    // and the move the mod asked for is not told back.
    assert!(shows(&window, "at 2, told 0"), "{:?}", said(&window));
}

fn run(window: &mut Runtime<Ui>) {
    for _ in 0..8 {
        window.update(|ui, tx| {
            poll(ui, 0., tx);
        });
        settle(window);
    }
}

fn said(window: &Runtime<Ui>) -> Vec<String> {
    window
        .elements()
        .iter()
        .filter_map(|element| element.text.clone())
        .filter(|text| text.starts_with("at "))
        .collect()
}
