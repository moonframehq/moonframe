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

#[gpui_kit::test]
fn revision_timeline_reveals_unchanged_endpoints(cx: &mut TestAppContext) {
    let mut window = open(cx, TIMELINE);
    let bounds = window.bounds("revision-timeline");
    let top = point(bounds.right() - px(5.0), bounds.top() + px(22.0));
    let bottom = point(bounds.right() - px(5.0), bounds.bottom() - px(22.0));
    window.click_at(top);
    for (key, scroll, endpoint) in [("home", 5000.0, top), ("end", -5000.0, bottom)] {
        window.keys(key);
        let log = window.log();
        window.wheel("revision-timeline", scroll);
        window.keys(key);
        assert_eq!(
            window.log(),
            log,
            "revealing must not repeat the selection message"
        );
        window.click_at(endpoint);
        assert_eq!(window.log(), log, "{key} must reveal the selected endpoint");
    }
}

#[gpui_kit::test]
fn revision_timeline_multiline_labels_render_plain_and_in_cards(cx: &mut TestAppContext) {
    let mut window = open(
        cx,
        r#"
return mod {
    init = function() return { log = '', preview = false } end,
    update = function(state) state.preview = not state.preview end,
    view = function(state)
        return shell(state, ui.column {
            ui.row { class = 'toggle-preview', msg = 'toggle', ui.text 'Preview' },
            ui.revision_timeline {
                entries = { { id = 'a', day = 'Today\nSunday', label = 'First line\r\nSecond line',
                    summary = 'A multiline\npreview body' } },
                selected = 'a', width = 300, height = 220, preview = state.preview,
            },
        })
    end,
}
"#,
    );
    assert!(window.shows("revision-timeline"));
    window.click("toggle-preview");
    assert!(window.shows("revision-timeline"));
}

fn preview_card_clicks(side: &str, cx: &mut TestAppContext) {
    let main = r#"
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
                { id = 'b', label = 'Second turn', summary = 'Preview body' },
                { id = 'c', label = 'Third turn' },
            }, selected = state.selected, select = 'pick', side = 'SIDE', labels = 'hover',
            dates = false, preview = true, magnification = 4, radius = 1, spread = 0,
            tick_width = 6, width = 300, height = 220,
        })
    end,
}
"#
    .replace("SIDE", side);
    let dir = folder(&main);
    // Settle the lens deterministically, without relying on wall-clock animation time.
    std::fs::write(
        dir.path().join("style.luau"),
        "return style { animation_speed = 0 }",
    )
    .unwrap();
    let mut window = open_in(cx, dir);
    let bounds = window.bounds("revision-timeline");
    let at = |distance| {
        point(
            if side == "left" {
                bounds.left() + px(distance)
            } else {
                bounds.right() - px(distance)
            },
            bounds.center().y,
        )
    };
    window
        .cx
        .simulate_mouse_move(at(10.0), None, Modifiers::none());
    window.settle();
    // The magnified tick is 24px wide, so its card starts 8 + 24 + 12 = 44px in.
    window.click_at(at(50.0));
    assert_eq!(window.log(), "", "the card's inner edge must not select");
    window.click_at(at(10.0));
    assert_eq!(window.log(), "|b", "the rail must remain selectable");
}

#[gpui_kit::test]
fn revision_timeline_left_preview_card_excludes_selection(cx: &mut TestAppContext) {
    preview_card_clicks("left", cx);
}

#[gpui_kit::test]
fn revision_timeline_right_preview_card_excludes_selection(cx: &mut TestAppContext) {
    preview_card_clicks("right", cx);
}
