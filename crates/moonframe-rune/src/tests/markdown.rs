//! `ui.markdown`'s structure: its headings, where a reader is, and a scroll
//! to a place, as its mod hears and asks for them.

use super::*;

/// A tall document in a short box, with its mod's view of it.
const LONG: &str = r###"
local lines = {}
table.insert(lines, "# First")
for i = 1, 40 do table.insert(lines, "Line " .. i .. " of the first part.") end
table.insert(lines, "## Second")
for i = 1, 40 do table.insert(lines, "Line " .. i .. " of the second part.") end
local text = table.concat(lines, "\n\n")
local second = string.find(text, "## Second", 1, true) - 1
return mod {
	init = function() return { headings = 0, heading = 0, go = false } end,
	update = function(state, msg)
		if msg.headings then state.headings = #msg.headings end
		if msg.heading then state.heading = msg.heading end
		if msg.go then state.go = true end
		return state
	end,
	view = function(state)
		return ui.column {
			grow = true,
			ui.text { `headings {state.headings}, at {state.heading}` },
			ui.markdown {
				text = text,
				height = 200,
				scroll = true,
				outline = { headings = true },
				section = { heading = true },
				reveal = if state.go then { at = second, seq = 1 } else nil,
			},
		}
	end,
}"###;

fn document() -> (TempDir, Runtime<Ui>) {
    let dir = folder(LONG);
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (600., 400.),
        Env::new(true, 2.),
    );
    run(&mut window);
    (dir, window)
}

/// Frames with the app's own housekeeping, which a test window lacks.
fn run(window: &mut Runtime<Ui>) {
    for _ in 0..8 {
        window.update(|ui, tx| {
            poll(ui, 0., tx);
        });
        settle(window);
    }
}

#[test]
fn a_document_tells_its_mod_its_headings() {
    let (_dir, window) = document();
    assert!(shows(&window, "headings 2, at 1"), "{:?}", said(&window));
}

#[test]
fn a_reveal_scrolls_there_and_the_section_follows() {
    let (_dir, mut window) = document();
    window.update(|ui, _| {
        ui.send(
            moonframe_core::SHELL,
            moonframe_core::SHELL,
            Data::map([("go", true.into())]),
        );
    });
    run(&mut window);
    assert!(shows(&window, "headings 2, at 2"), "{:?}", said(&window));
}

fn said(window: &Runtime<Ui>) -> Vec<String> {
    window
        .elements()
        .iter()
        .filter_map(|element| element.text.clone())
        .filter(|text| text.starts_with("headings"))
        .collect()
}

#[test]
fn a_link_to_a_heading_scrolls_there() {
    let (_dir, mut window) = document();
    window.update(|ui, _| {
        let path = ui.documents.get_mut().keys().next().cloned().unwrap();
        ui.documents.get_mut().get_mut(&path).unwrap().pending = Some(41);
    });
    run(&mut window);
    assert!(shows(&window, "headings 2, at 2"), "{:?}", said(&window));
}
