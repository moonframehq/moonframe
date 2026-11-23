//! A scrolling document keeps the text being read where it is: through a
//! resize that rewraps it, an edit above it, and steady wheel steps, and a
//! jump far down lands on its heading at once.

use super::*;

/// Sections of wrapping prose, code and lists, so blocks differ in height
/// and a narrower window changes them.
const SECTIONS: &str = r###"
local parts = {}
for i = 1, 40 do
	local prose = string.rep(`Ledger entry {i} is reconciled against the bank statement. `, 6)
	table.insert(parts, `## Section {i}\n\n{prose}\n`)
	if i % 3 == 0 then
		table.insert(parts, "```lua\nlocal total = 0\nfor _, entry in entries do\n\ttotal += entry\nend\n```\n")
	end
	if i % 4 == 0 then
		table.insert(parts, "- debit\n- credit\n- balance\n")
	end
end
local SECTIONS = table.concat(parts, "\n")
local FAR = string.find(SECTIONS, "## Section 20\n", 1, true) - 1
local LATE = string.find(SECTIONS, "## Section 32\n", 1, true) - 1
local PREFACE = string.rep("A paragraph written above the one being read.\n\n", 12)
"###;

/// A document opened at `at`, whose `prepend` button writes a preface above
/// everything, and whose section and scroll moves are logged.
fn document(at: &str) -> String {
    format!(
        r###"{SECTIONS}
return mod {{
	init = function() return {{ log = "", text = SECTIONS }} end,
	update = function(state, msg)
		if msg == "prepend" then
			state.text = PREFACE .. state.text
		elseif msg.msg == "reading" then
			logged(state, "section=" .. msg.heading)
		elseif msg.msg == "moved" then
			logged(state, "fraction=" .. msg.fraction)
		end
	end,
	view = function(state)
		return shell(state, ui.column {{ grow = true,
			ui.row {{ class = "prepend", ui.button {{ "Prepend", msg = "prepend" }} }},
			ui.markdown {{ text = state.text, grow = true, scroll = true,
				section = "reading", scrolled = "moved", reveal = {{ at = {at} }} }},
		}})
	end,
}}"###
    )
}

/// The last section the log reached.
fn section(log: &str) -> usize {
    log.rsplit("section=")
        .next()
        .and_then(|rest| rest.split('|').next())
        .and_then(|number| number.parse().ok())
        .unwrap_or(0)
}

/// Each fraction the log holds, in order.
fn fractions(log: &str) -> Vec<f64> {
    log.split('|')
        .filter_map(|line| line.strip_prefix("fraction="))
        .map(|fraction| fraction.parse().unwrap())
        .collect()
}

/// Nudges the document so it tells which section it shows now.
fn reading(window: &mut Window<'_>) -> usize {
    window.wheel("markdown", 1.0);
    section(&window.log())
}

#[gpui_kit::test]
fn a_resize_keeps_the_section_being_read(cx: &mut TestAppContext) {
    let mut window = open(cx, &document("FAR"));
    assert_eq!(section(&window.log()), 20, "{}", window.log());
    for width in [700.0, 420.0, 1100.0] {
        let before = window.bounds("markdown").size.width;
        window
            .cx
            .simulate_resize(gpui_kit::size(px(width), px(700.)));
        window.settle();
        assert_ne!(window.bounds("markdown").size.width, before, "it rewrapped");
        assert_eq!(reading(&mut window), 20, "at {width}: {}", window.log());
    }
}

#[gpui_kit::test]
fn an_edit_above_keeps_the_section_being_read(cx: &mut TestAppContext) {
    let mut window = open(cx, &document("FAR"));
    assert_eq!(section(&window.log()), 20, "{}", window.log());
    // The button sits at the start of its row.
    let row = window.bounds("prepend");
    window.click_at(row.origin + point(px(12.), row.size.height / 2.));
    assert!(window.view().contains("written above"), "the preface is in");
    assert_eq!(reading(&mut window), 20, "{}", window.log());
    // Nor did the mod hear of a section passed on the way back.
    let log = window.log();
    assert!(
        log.split('|')
            .filter(|line| line.starts_with("section="))
            .all(|line| line == "section=20"),
        "{log}"
    );
    // The mod's own move is not a reader's: it is not told back.
    assert_eq!(fractions(&log).len(), 1, "only the nudge: {log}");
}

#[gpui_kit::test]
fn equal_wheel_steps_move_the_document_equally(cx: &mut TestAppContext) {
    let mut window = open(cx, &document("0"));
    for _ in 0..4 {
        window.wheel("markdown", 300.0);
    }
    let moved = fractions(&window.log());
    assert_eq!(moved.len(), 4, "{}", window.log());
    // A total height that grew as blocks came into view would make later
    // steps cover less of the document than earlier ones.
    let steps: Vec<f64> = moved
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .chain([moved[0]])
        .collect();
    let (least, most) = steps.iter().fold((f64::MAX, 0.0_f64), |(lo, hi), step| {
        (lo.min(*step), hi.max(*step))
    });
    assert!(least > 0.0 && most - least < 0.002, "steps {steps:?}");
}

#[gpui_kit::test]
fn a_jump_far_down_lands_on_its_heading_at_once(cx: &mut TestAppContext) {
    let mut window = open(cx, &document("LATE"));
    assert_eq!(window.log(), "|section=32");
}
