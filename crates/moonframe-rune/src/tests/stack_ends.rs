//! `at_end` and `scroll_end` on a scrolling column, as on GPUI: the mod
//! hears where the end is, and a new `scroll_end` goes there.

use super::*;

const LOG: &str = r###"
return mod {
	init = function() return { ending = "unknown", seq = nil } end,
	update = function(state, msg)
		if msg.go then state.seq = msg.go end
		if msg.msg == "ends" then state.ending = tostring(msg.at_end) end
		return state
	end,
	view = function(state)
		local rows = {}
		for i = 1, 60 do
			table.insert(rows, ui.row { height = 30, shrink = false, ui.text { `line {i}` } })
		end
		return ui.column {
			grow = true,
			ui.text { `end {state.ending}` },
			ui.column { height = 200, scroll = true, at_end = "ends", scroll_end = state.seq,
				table.unpack(rows) },
		}
	end,
}"###;

#[test]
fn a_column_says_where_its_end_is_and_goes_there_when_asked() {
    let dir = folder(LOG);
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (600., 400.),
        Env::new(true, 2.),
    );
    run(&mut window);
    assert!(shows(&window, "end false"), "{:?}", said(&window));
    window.update(|ui, _| {
        ui.send(
            moonframe_core::SHELL,
            moonframe_core::SHELL,
            Data::map([("go", Data::Integer(1))]),
        );
    });
    run(&mut window);
    assert!(shows(&window, "end true"), "{:?}", said(&window));
}

fn run(window: &mut Runtime<Ui>) {
    for _ in 0..12 {
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
        .filter(|text| text.starts_with("end "))
        .collect()
}

/// A chat that can be closed and opened again.
const REOPENED: &str = r###"
return mod {
	init = function() return { ending = "unknown", open = true, told = 0 } end,
	update = function(state, msg)
		if msg.open ~= nil then state.open = msg.open end
		if msg.msg == "ends" then
			state.ending = tostring(msg.at_end)
			state.told += 1
		end
		return state
	end,
	view = function(state)
		local rows = {}
		for i = 1, 60 do
			table.insert(rows, ui.row { height = 30, shrink = false, ui.text { `line {i}` } })
		end
		return ui.column {
			grow = true,
			ui.text { `end {state.ending} told {state.told}` },
			state.open and ui.column { height = 200, scroll = true, at_end = "ends",
				table.unpack(rows) },
		}
	end,
}"###;

#[test]
fn a_column_drawn_again_tells_its_mod_again() {
    let dir = folder(REOPENED);
    let mut window = Runtime::new(
        Ui::open(dir.path().to_owned(), None),
        view,
        (600., 400.),
        Env::new(true, 2.),
    );
    run(&mut window);
    assert!(shows(&window, "end false told 1"), "{:?}", said(&window));
    for open in [false, true] {
        window.update(|ui, _| {
            ui.send(
                moonframe_core::SHELL,
                moonframe_core::SHELL,
                Data::map([("open", Data::Bool(open))]),
            );
        });
        run(&mut window);
    }
    assert!(shows(&window, "end false told 2"), "{:?}", said(&window));
}
