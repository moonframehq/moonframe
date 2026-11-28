//! `agent-chat` from the keyboard: Enter completes a `/` command (the
//! commands are only in `build`'s view, never in `update`), then sends.

use super::*;

const CHAT: &str = r#"
local agent_chat = use "agent-chat"
return mod {
	init = function() return { log = "" } end,
	update = function(state, msg)
		if type(msg) == "table" and msg.prompt then
			logged(state, "prompt:" .. msg.prompt)
		elseif type(msg) == "table" and msg.setting then
			logged(state, "setting:" .. msg.setting .. "=" .. tostring(msg.value))
		end
	end,
	view = function(state)
		return shell(state, agent_chat {
			id = "test", chat = "c", to = "parent",
			view = { id = "c", status = "live", view = {
				status = "idle", accepts = true, rows = {}, permissions = {}, queue = {},
				settings = {}, activities = {},
				commands = { { name = "review", description = "Review" }, { name = "reset", description = "Reset" } },
			} },
		})
	end,
}"#;

#[gpui_kit::test]
fn enter_completes_a_slash_command_then_sends_it(cx: &mut TestAppContext) {
    let mut window = open(cx, CHAT);
    window.typed("/re");
    window.keys("down");
    window.keys("enter");
    assert_eq!(window.log(), "", "completing sends nothing");
    window.typed("now");
    window.keys("enter");
    assert_eq!(window.log(), "|prompt:/reset now");
}
