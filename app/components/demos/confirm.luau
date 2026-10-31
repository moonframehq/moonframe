-- Close a window with running agents: the button asks, Keep open (or Esc)
-- cancels, Close window (or ⌘↩) confirms. The switch flips `danger`.
local confirm = use "confirm"
local switch = use "switch"

return mod {
	init = function(previous)
		return previous or { open = true, danger = true, result = "" }
	end,

	update = function(state, msg)
		if msg == "ask" then
			state.open = true
		elseif msg == "confirm" then
			state.open = false
			state.result = "Window closed; 2 agents interrupted."
		elseif msg == "cancel" then
			state.open = false
			state.result = "Kept open."
		elseif msg == "toggle-danger" then
			state.danger = not state.danger
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				ui.button { "Close window", icon = "close", variant = "outline", msg = "ask" },
				switch { on = state.danger, label = "Destructive", small = true, msg = "toggle-danger" },
			},
			state.result ~= "" and ui.text { state.result, tone = "muted", size = "small" },
			confirm {
				open = state.open,
				title = "Close this window?",
				message = "Closing it interrupts the agents still running here.",
				items_title = "Agents that will be interrupted",
				items = {
					{ label = "Claude", detail = "feature/inspect · writing tests", icon = "agent-claude" },
					{ label = "Codex", detail = "fix/menus · running cargo test", icon = "agent-codex" },
				},
				confirm_label = "Close window",
				cancel_label = "Keep open",
				danger = state.danger,
			},
		}
	end,
}
