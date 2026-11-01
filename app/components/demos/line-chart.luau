-- Exceptions per day against the forecast: click a day to read its
-- figures in the card beside it.
local line_chart = use "line-chart"

local ACTUAL = { 3, 6, 11, 9, 8, 9, 9, 3 }
local PREDICTED = { 6, 4, 6, 7, 5, 3, 5, 4 }
local DAYS = { "21 Oct", "22 Oct", "23 Oct", "24 Oct", "25 Oct", "26 Oct", "27 Oct", "28 Oct" }

return mod {
	init = function(previous)
		return previous or { day = 4 }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.point then
			state.day = msg.point
		end
	end,

	view = function(state)
		local day = state.day
		return line_chart {
			labels = DAYS,
			series = {
				{ name = "Predicted", points = PREDICTED, color = "#3f9a5c", dashed = true },
				{ name = "Actual", points = ACTUAL, color = "#3f73b5", dashed = false },
			},
			bars = { 6, 5, 6, 7, 6, 6, 6, 5 },
			max = 15,
			ticks = { "15", "10", "5", "0" },
			width = 520,
			height = 160,
			selected = day,
			card_width = 150,
			card = ui.column {
				gap = "small",
				ui.text { DAYS[day], weight = "medium", size = "small" },
				ui.row { ui.text { "Actual", size = "small", tone = "muted" }, ui.spacer(), ui.text { tostring(ACTUAL[day]), size = "small" } },
				ui.row { ui.text { "Predicted", size = "small", tone = "muted" }, ui.spacer(), ui.text { tostring(PREDICTED[day]), size = "small" } },
			},
		}
	end,
}
