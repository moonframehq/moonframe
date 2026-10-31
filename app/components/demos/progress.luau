-- Indexing a workspace: − and + move the progress, Busy shows the striped
-- bar for unknown size, and the tone follows the segmented control.
local progress = use "progress"
local stepper = use "stepper"
local segmented = use "segmented"
local switch = use "switch"

local TOTAL = 2006

return mod {
	init = function(previous)
		return previous or { percent = 60, busy = false, tone = "accent" }
	end,

	update = function(state, msg)
		if msg == "toggle-busy" then
			state.busy = not state.busy
		elseif type(msg) == "table" and msg.step then
			state.percent = math.clamp(state.percent + msg.step * 10, 0, 100)
		elseif type(msg) == "table" and msg.select then
			state.tone = msg.select
		end
	end,

	view = function(state)
		local done = math.floor(TOTAL * state.percent / 100)
		return ui.column {
			gap = "large",
			ui.row {
				gap = "normal",
				align = "center",
				stepper { value = state.percent, unit = "%", min = 0, max = 100 },
				switch { on = state.busy, label = "Busy", small = true, msg = "toggle-busy" },
			},
			segmented {
				selected = state.tone,
				small = true,
				options = {
					{ id = "accent", label = "Accent" },
					{ id = "success", label = "Success" },
					{ id = "warning", label = "Warning" },
					{ id = "danger", label = "Danger" },
				},
			},
			progress {
				label = if state.percent >= 100 then "Workspace indexed" else "Indexing workspace",
				value = state.percent / 100,
				busy = state.busy,
				tone = state.tone,
				detail = if state.busy then "counting files…" else `{done} of {TOTAL} files`,
				width = 280,
			},
		}
	end,
}
