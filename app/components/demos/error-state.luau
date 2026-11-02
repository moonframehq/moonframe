-- A failed load: Retry spins (click again to finish the attempt), Open
-- settings is the second button; the switch centres it as a block.
local error_state = use "error-state"
local switch = use "switch"

return mod {
	init = function(previous)
		return previous or { retrying = false, attempts = 0, block = false, note = "" }
	end,

	update = function(state, msg)
		if msg == "retry" then
			if state.retrying then
				state.retrying = false
				state.attempts += 1
			else
				state.retrying = true
			end
		elseif msg == "open-settings" then
			state.note = "Settings › Integrations opened."
		elseif msg == "toggle-block" then
			state.block = not state.block
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row { switch { on = state.block, label = "Block", small = true, msg = "toggle-block" } },
			ui.column {
				height = if state.block then 260 else nil,
				error_state {
					block = state.block,
					title = "Couldn't load pull requests",
					message = "GitHub refused the request. Check the token in Settings › Integrations.",
					details = `HTTP 401 Unauthorized: Bad credentials (GET /repos/moonframehq/moonframe/pulls) · attempt {state.attempts + 1}`,
					retry = "retry",
					retrying = state.retrying,
					secondary = { label = "Open settings", msg = "open-settings" },
				},
			},
			state.note ~= "" and ui.text { state.note, tone = "muted", size = "small" },
		}
	end,
}
