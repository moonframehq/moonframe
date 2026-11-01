-- Three readings over a day: hover the plot for a sample's values, click
-- to pin the crosshair there.
local series_chart = use "series-chart"

local CPU = { 48, 51, 50, 53, 55, 54, 52, 56, 58, 57, 55, 57, 59, 60, 58, 61 }
local GPU = { 33, 34, 34, 36, 35, 39, 44, 48, 53, 57, 60, 62, 66, 71, 74, 72 }
local MEMORY = { 27, 28, 26, 27, 25, 24, 25, 23, 22, 23, 21, 22, 21, 20, 21, 20 }

return mod {
	init = function(previous)
		return previous or { pinned = 11 }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.pin then
			state.pinned = if state.pinned == msg.pin then nil else msg.pin
		end
	end,

	view = function(state)
		local titles = {}
		for index = 1, #CPU do
			titles[index] = string.format("TODAY, %02d:00", (index - 1) * 1.5 // 1)
		end
		return series_chart {
			width = 460,
			height = 150,
			pinned = state.pinned,
			titles = titles,
			series = {
				{ name = "CPU", color = "#3b82f6", points = CPU },
				{ name = "GPU", color = "#22c55e", points = GPU },
				{ name = "Memory", color = "#f97316", points = MEMORY },
			},
			labels = {
				{ index = 1, text = "00:00" },
				{ index = 6, text = "07:30" },
				{ index = 11, text = "15:00" },
				{ index = 16, text = "NOW" },
			},
		}
	end,
}
