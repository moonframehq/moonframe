//! A source field and its preview that follow each other's scroll with
//! `scrolled` and `scroll_at`, as a markdown editor's two panes do.

use super::*;

/// A source field beside its preview, each following the other's scroll.
const EDITOR: &str = r##"
local lines = {}
for i = 1, 120 do
	table.insert(lines, `## Section {i}\n\nA paragraph about the ledger number {i}.\n`)
end
local TEXT = table.concat(lines, "\n")
return mod {
	init = function()
		return { log = "", seq = 0, source = nil :: any, preview = nil :: any }
	end,
	update = function(state, msg)
		logged(state, `{msg.msg}={msg.fraction}`)
		state.seq += 1
		local at = { fraction = msg.fraction, seq = state.seq }
		if msg.msg == "source" then state.preview = at else state.source = at end
	end,
	view = function(state)
		return shell(state, ui.row { grow = true,
			ui.column { class = "source", grow = true,
				ui.input { id = "source", value = TEXT, multiline = true, grow = true,
					scrolled = "source", scroll_at = state.source },
			},
			ui.markdown { text = TEXT, grow = true, scroll = true,
				scrolled = "preview", scroll_at = state.preview },
		})
	end,
}
"##;

/// Each fraction the log holds for `pane`, in order.
fn told(log: &str, pane: &str) -> Vec<f64> {
    let mark = format!("{pane}=");
    log.split('|')
        .filter_map(|line| line.strip_prefix(&mark))
        .map(|fraction| fraction.parse().unwrap())
        .collect()
}

#[gpui_kit::test]
fn a_preview_scrolled_by_the_reader_takes_its_source_along(cx: &mut TestAppContext) {
    let mut window = open(cx, EDITOR);
    assert_eq!(window.log(), "", "nothing moved yet");
    window.wheel("markdown", 1500.0);
    let log = window.log();
    let preview = told(&log, "preview");
    assert!(
        preview
            .first()
            .is_some_and(|fraction| *fraction > 0.0 && *fraction < 1.0),
        "{log}"
    );
    // The source followed, and did not echo the move back.
    assert!(told(&log, "source").is_empty(), "{log}");
    window.wheel("markdown", 1500.0);
    let further = told(&window.log(), "preview");
    assert!(further.last() > preview.last(), "{}", window.log());
    assert!(told(&window.log(), "source").is_empty(), "{}", window.log());
}

#[gpui_kit::test]
fn a_source_scrolled_by_the_writer_takes_its_preview_along(cx: &mut TestAppContext) {
    let mut window = open(cx, EDITOR);
    window.wheel("source", 600.0);
    let log = window.log();
    let source = told(&log, "source");
    assert!(
        source
            .first()
            .is_some_and(|fraction| *fraction > 0.0 && *fraction < 1.0),
        "{log}"
    );
    assert!(told(&log, "preview").is_empty(), "{log}");
    // Then the reader moves the preview: the source hears of it, and the
    // preview keeps where the reader put it.
    window.wheel("markdown", 300.0);
    let log = window.log();
    assert_eq!(told(&log, "preview").len(), 1, "{log}");
    assert_eq!(told(&log, "source").len(), 1, "{log}");
}

#[gpui_kit::test]
fn the_end_of_one_pane_is_the_end_of_the_other(cx: &mut TestAppContext) {
    let mut window = open(cx, EDITOR);
    window.wheel("markdown", 1_000_000.0);
    assert_eq!(told(&window.log(), "preview").last(), Some(&1.0));
    // The source, now at its end, moves back up from there.
    window.wheel("source", -200.0);
    let back = told(&window.log(), "source");
    assert!(
        back.last()
            .is_some_and(|fraction| *fraction < 1.0 && *fraction > 0.5),
        "{}",
        window.log()
    );
}

#[gpui_kit::test]
fn typing_moves_no_pane_for_the_other(cx: &mut TestAppContext) {
    let mut window = open(
        cx,
        r##"
local lines = {}
for i = 1, 120 do
	table.insert(lines, `## Section {i}\n\nA paragraph about the ledger number {i}.\n`)
end
return mod {
	init = function() return { log = "", text = table.concat(lines, "\n") } end,
	update = function(state, msg)
		if msg.msg == "typed" then state.text = msg.value
		else logged(state, `{msg.msg}={msg.fraction}`) end
	end,
	view = function(state)
		return shell(state, ui.row { grow = true,
			ui.column { class = "source", grow = true,
				ui.input { id = "source", value = state.text, multiline = true, grow = true,
					change = "typed", scrolled = "source" },
			},
			ui.markdown { text = state.text, grow = true, scroll = true, scrolled = "preview" },
		})
	end,
}
"##,
    );
    window.wheel("markdown", 1500.0);
    let before = told(&window.log(), "preview").len();
    assert_eq!(before, 1, "{}", window.log());
    window.click("source");
    // New blocks, which the preview lays out afresh.
    window.typed("\n\n## Inserted\n\nMore text.\n\n");
    let log = window.log();
    assert_eq!(told(&log, "preview").len(), before, "{log}");
}
