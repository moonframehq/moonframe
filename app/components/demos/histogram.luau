-- Levels over a picture's histogram: drag the shadow and highlight
-- markers along it; what lies outside them is dimmed. Log scale shows the
-- small counts in the shadows.
local histogram = use "histogram"
local switch = use "switch"

local RED = { 2, 6, 14, 30, 52, 70, 64, 48, 40, 52, 66, 50, 30, 16, 8, 3, 2, 1, 1, 0 }
local GREEN = { 1, 4, 10, 22, 40, 60, 74, 66, 50, 44, 46, 38, 24, 12, 5, 2, 1, 1, 0, 0 }
local BLUE = { 4, 10, 22, 40, 58, 62, 50, 40, 36, 34, 30, 22, 14, 8, 4, 1, 1, 0, 0, 0 }

return mod {
	init = function(previous)
		return previous or { shadow = 0.06, highlight = 0.82, log = false }
	end,

	update = function(state, msg)
		if msg == "log" then
			state.log = not state.log
		elseif typeof(msg) == "table" and msg.marker then
			if msg.marker == "shadow" then
				state.shadow = math.min(msg.value, state.highlight - 0.02)
			else
				state.highlight = math.max(msg.value, state.shadow + 0.02)
			end
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			hug = true,
			histogram {
				width = 300,
				height = 110,
				log = state.log,
				channels = {
					{ name = "Red", color = "#ef4444", values = RED },
					{ name = "Green", color = "#22c55e", values = GREEN },
					{ name = "Blue", color = "#3b82f6", values = BLUE },
				},
				markers = {
					{ id = "shadow", value = state.shadow, tooltip = "Shadow" },
					{ id = "highlight", value = state.highlight, tooltip = "Highlight" },
				},
				drag = { level = true },
				shade = true,
			},
			ui.row {
				align = "center",
				gap = "large",
				ui.text { `Shadow {math.floor(state.shadow * 255 + 0.5)} · Highlight {math.floor(state.highlight * 255 + 0.5)}`, size = "small", tone = "muted" },
				ui.spacer(),
				switch { label = "Log", on = state.log, msg = "log", small = true },
			},
		}
	end,
}
