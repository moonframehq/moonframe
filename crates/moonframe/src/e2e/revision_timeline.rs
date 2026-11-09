//! Exercise the native ruler through GPUI events and the real Luau message path.
use super::*;

const TIMELINE: &str = r#"
local entries = {}
for i = 1, 80 do
    table.insert(entries, { id = `r{i}`, day = `Day {math.ceil(i / 4)}`, label = `Revision {i}` })
end
return mod {
    init = function() return { log = "", selected = "r1", disabled = false } end,
    update = function(state, msg)
        if msg == "disable" then state.disabled = true
        elseif type(msg) == "table" and msg.msg == "pick" then
            state.selected = msg.revision
            logged(state, `{msg.revision}:{msg.day}`)
        end
    end,
    view = function(state)
        return shell(state, ui.column {
            ui.row { class = "disable", msg = "disable", ui.text "Disable" },
            ui.revision_timeline { entries = entries, selected = state.selected,
                select = "pick", width = 260, height = 400, disabled = state.disabled },
        })
    end,
}
"#;

#[gpui_kit::test]
fn revision_timeline_click_keys_days_and_scrolling(cx: &mut TestAppContext) {
    let mut window = open(cx, TIMELINE);
    let bounds = window.bounds("revision-timeline");
    // Hover is visual only; it must not commit a selection.
    window
        .cx
        .simulate_mouse_move(bounds.center(), None, Modifiers::none());
    window.settle();
    assert_eq!(window.log(), "");
    window.click_at(point(bounds.right() - px(5.0), bounds.top() + px(22.0)));
    window.keys("down");
    assert!(window.log().ends_with("r2:Day 1"), "{}", window.log());
    window.keys("shift-down");
    assert!(window.log().ends_with("r5:Day 2"), "{}", window.log());
    window.keys("shift-up");
    assert!(window.log().ends_with("r4:Day 1"), "{}", window.log());
    window.keys("end");
    assert!(window.log().ends_with("r80:Day 20"), "{}", window.log());
    window.keys("home");
    assert!(window.log().ends_with("r1:Day 1"), "{}", window.log());
    // Scroll to the end, then click the painted bottom tick.
    window.wheel("revision-timeline", 5000.0);
    window.click_at(point(bounds.right() - px(5.0), bounds.bottom() - px(22.0)));
    assert!(window.log().ends_with("r80:Day 20"), "{}", window.log());
    let log = window.log();
    window.click("disable");
    window.click_at(bounds.center());
    window.keys("home");
    assert_eq!(window.log(), log, "disabled rulers do not select");
}

#[gpui_kit::test]
fn revision_timeline_left_chat_rail_previews_without_selecting(cx: &mut TestAppContext) {
    let mut window = open(
        cx,
        r#"
return mod {
    init = function() return { log = '', selected = 'a' } end,
    update = function(state, msg)
        state.selected = msg.revision
        logged(state, msg.revision)
    end,
    view = function(state)
        return shell(state, ui.revision_timeline {
            entries = {
                { id = 'a', label = 'First turn' },
                { id = 'b', label = 'Second turn', summary = 'A longer message wrapping into a small preview card.' },
                { id = 'c', label = 'Third turn' },
            }, selected = state.selected, select = 'pick', side = 'left', labels = 'hover',
            dates = false, preview = true, magnification = 3, spread = 0, tick_width = 6,
            width = 300, height = 220,
        })
    end,
}
"#,
    );
    let bounds = window.bounds("revision-timeline");
    window.cx.simulate_mouse_move(
        point(bounds.left() + px(10.0), bounds.center().y),
        None,
        Modifiers::none(),
    );
    window.settle();
    assert_eq!(window.log(), "", "hover only previews");
    window.click_at(point(bounds.left() + px(10.0), bounds.center().y));
    assert_eq!(window.log(), "|b");
    window.keys("shift-down");
    assert_eq!(
        window.log(),
        "|b|c",
        "undated entries navigate as individual turns"
    );
    window.click_at(point(bounds.right() - px(5.0), bounds.top() + px(25.0)));
    assert_eq!(window.log(), "|b|c", "the preview area is not a hit target");
}
