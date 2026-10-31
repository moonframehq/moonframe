-- Setting rows: flip the switch, pick a preset from the dropdown; turning
-- off Show agents disables the row that depends on it.
local settings_row = use "settings-row"
local switch = use "switch"
local dropdown = use "dropdown"

local PRESETS = {
	{ id = "comfortable", label = "Comfortable Rounded" },
	{ id = "compact", label = "Compact" },
	{ id = "cozy", label = "Cozy Square" },
}

return mod {
	init = function(previous)
		return previous or { preset = "comfortable", open = false, agents = true, icons = false }
	end,

	update = function(state, msg)
		if msg == "toggle-preset" then
			state.open = not state.open
		elseif msg == "close-preset" then
			state.open = false
		elseif msg == "toggle-agents" then
			state.agents = not state.agents
		elseif msg == "toggle-icons" then
			state.icons = not state.icons
		elseif type(msg) == "table" and msg.pick then
			state.preset = msg.pick
			state.open = false
		end
	end,

	view = function(state)
		return ui.column {
			gap = "none",
			settings_row {
				title = "Layout preset",
				tag = "Local",
				description = "Row shape and density; spacing below starts from the preset.",
				dropdown {
					open = state.open,
					selected = state.preset,
					options = PRESETS,
					toggle = "toggle-preset",
					dismiss = "close-preset",
					align = "end",
				},
			},
			settings_row {
				title = "Show agents",
				description = "List running agents under their workspace.",
				switch { on = state.agents, msg = "toggle-agents" },
			},
			settings_row {
				title = "Show device icons",
				tag = "Shared",
				tag_tone = "shared",
				description = "A mark before each device's name.",
				disabled = not state.agents,
				reason = if state.agents then nil else "Turn on Show agents first.",
				border = false,
				switch { on = state.icons, disabled = not state.agents, msg = "toggle-icons" },
			},
		}
	end,
}
