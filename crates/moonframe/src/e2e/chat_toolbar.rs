//! The shared chat composer fits both an inspector and a full-width pane.

use super::*;

#[gpui_kit::test]
fn composer_actions_fit_at_sidebar_and_full_widths(cx: &mut TestAppContext) {
    for width in [300, 380, 480, 900] {
        let main = r#"
local chat = use "agent-chat"
return mod {
    init = function() return { log = "" } end,
    update = function(state, msg)
        if type(msg) == "table" and msg.prompt then logged(state, msg.prompt) end
    end,
    view = function(state)
        return shell(state, ui.column { width = WIDTH, height = 400,
            chat { id = "test", chat = "c", to = "parent", view = { id = "c", status = "live", view = {
                status = "working", accepts = true, rows = {},
                mode = { current_name = "Bypass Permissions" },
                model = { current_name = "Opus 5.5 (recommended)" },
                effort = { current_name = "High" },
                usage = { text = "47.6k / 1M", cost = "$1.25", percent = 5 },
            } } },
        })
    end,
}"#
        .replace("WIDTH", &width.to_string());
        let mut window = open(cx, &main);
        let composer = window.bounds("agent-chat-composer");
        let actions = window.bounds("agent-chat-actions");
        let picker = window.bounds("dropdown-trigger");
        assert!(composer.size.width <= px(width as f32));
        assert!(actions.size.width > px(0.));
        assert!(composer.contains(&actions.origin));
        assert!(actions.right() <= composer.right() && actions.bottom() <= composer.bottom());
        window.typed("Hello");
        window.click_at(point(actions.right() - px(10.), actions.center().y));
        assert_eq!(window.log(), "|Hello", "Send at width {width}");
        if width == 900 {
            assert!(actions.origin.y < picker.bottom());
        } else if width <= 380 {
            assert!(actions.origin.y >= picker.bottom());
        }
    }
}
