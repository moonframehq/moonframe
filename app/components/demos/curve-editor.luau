-- A gradation curve per channel: press to add a point and drag it,
-- double-click a point to remove it. The tabs switch channels, each with
-- its own points; the faint shape behind is that channel's histogram.
local curve_editor = use "curve-editor"
local segmented = use "segmented"

local COLORS = { rgb = nil, red = "#ef4444", green = "#22c55e", blue = "#3b82f6" }
local HISTOGRAM = { 1, 3, 8, 16, 30, 46, 58, 62, 55, 44, 40, 46, 52, 44, 30, 18, 9, 4, 2, 1 }

return mod {
	init = function(previous)
		return previous
			or {
				channel = "rgb",
				curves = {
					rgb = { { 0, 0 }, { 0.25, 0.2 }, { 0.75, 0.82 }, { 1, 1 } },
					red = { { 0, 0 }, { 1, 1 } },
					green = { { 0, 0 }, { 1, 1 } },
					blue = { { 0, 0.04 }, { 0.5, 0.46 }, { 1, 1 } },
				},
			}
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		if msg.select then
			state.channel = msg.select
		elseif msg.points then
			state.curves[state.channel] = msg.points
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			hug = true,
			segmented {
				small = true,
				selected = state.channel,
				options = {
					{ id = "rgb", label = "RGB" },
					{ id = "red", label = "Red" },
					{ id = "green", label = "Green" },
					{ id = "blue", label = "Blue" },
				},
			},
			curve_editor {
				points = state.curves[state.channel],
				change = { curve = state.channel },
				color = COLORS[state.channel],
				values = HISTOGRAM,
				size = 220,
			},
			ui.text { `{#state.curves[state.channel]} points`, tone = "muted", size = "small" },
		}
	end,
}
