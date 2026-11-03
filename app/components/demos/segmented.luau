-- A theme picker: click a segment to choose it.
local segmented = use "segmented"

return mod {
	init = function(previous)
		return previous or { theme = "dark" }
	end,

	update = function(state, msg)
		if msg.select then
			state.theme = msg.select
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			segmented {
				selected = state.theme,
				options = {
					{ id = "light", label = "Light" },
					{ id = "dark", label = "Dark" },
					{ id = "system", label = "System" },
				},
			},
			ui.text { `Theme: {state.theme}`, tone = "muted", size = "small" },
		}
	end,
}
