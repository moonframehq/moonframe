-- One toast: pick its level, × hides it and Show again brings it back;
-- Reconnect is its action.
local toast = use "toast"
local segmented = use "segmented"

local TEXT = {
	info = { "Checkpoint saved", "checkpoint-5 · Balance per currency" },
	success = { "Worktree created", "feature/inspect at ~/code/app-inspect" },
	warning = { "Local Git and PR details unavailable", "The daemon socket directory must be owned by your user and not world-writable." },
	error = { "Agent exited with code 1", "cargo test failed in crates/moonframe-core" },
}

return mod {
	init = function(previous)
		return previous or { shown = true, level = "warning", reconnects = 0 }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.dismiss then
			state.shown = false
		elseif type(msg) == "table" and msg.select then
			state.level = msg.select
			state.shown = true
		elseif msg == "show" then
			state.shown = true
		elseif msg == "reconnect" then
			state.reconnects += 1
		end
	end,

	view = function(state)
		local text = TEXT[state.level]
		return ui.column {
			gap = "normal",
			segmented {
				selected = state.level,
				small = true,
				options = {
					{ id = "info", label = "Info" },
					{ id = "success", label = "Success" },
					{ id = "warning", label = "Warning" },
					{ id = "error", label = "Error" },
				},
			},
			if state.shown
				then toast {
					id = "demo",
					level = state.level,
					source = "Local",
					title = text[1],
					body = text[2],
					full = true,
					action = { label = "Reconnect", msg = "reconnect" },
				}
				else ui.row { ui.button { "Show again", small = true, variant = "outline", msg = "show" } },
			state.reconnects > 0 and ui.text { `Reconnect clicked {state.reconnects}×`, tone = "muted", size = "small" },
		}
	end,
}
