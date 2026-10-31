-- Tokens per day: the chips hide or show a series, and the switches turn
-- the scale and the legend on and off.
local bar_chart = use "bar-chart"
local chips = use "chips"
local switch = use "switch"

local SERIES = {
	{ id = "input", name = "Input", tone = "c1" },
	{ id = "output", name = "Output", tone = "c4" },
	{ id = "cache", name = "Cache", tone = "muted" },
}

local DAYS: { { any } } = {
	{ "Mon", { 42, 18, 12 } },
	{ "Tue", { 55, 22, 20 } },
	{ "Wed", { 31, 12, 9 } },
	{ "Thu", { 68, 30, 26 } },
	{ "Fri", { 74, 35, 31 } },
	{ "Sat", { 12, 4, 3 } },
	{ "Sun", { 8, 2, 2 } },
}

return mod {
	init = function(previous)
		return previous or { shown = { input = true, output = true, cache = true }, axis = true, legend = true }
	end,

	update = function(state, msg)
		if msg == "toggle-axis" then
			state.axis = not state.axis
		elseif msg == "toggle-legend" then
			state.legend = not state.legend
		elseif type(msg) == "table" and msg.toggle then
			state.shown[msg.toggle] = not state.shown[msg.toggle]
		end
	end,

	view = function(state)
		local series, columns, selected = {}, {}, {}
		for index, entry in SERIES do
			if state.shown[entry.id] then
				table.insert(series, { name = entry.name, tone = entry.tone })
				table.insert(columns, index)
				table.insert(selected, entry.id)
			end
		end
		local buckets = {}
		for index, day in DAYS do
			local values = {}
			local total = 0
			for _, column in columns do
				table.insert(values, day[2][column])
				total += day[2][column]
			end
			buckets[index] = { label = day[1], values = values, tooltip = `{day[1]}: {total}k tokens` }
		end
		return ui.column {
			gap = "normal",
			chips {
				selected = selected,
				chips = {
					{ id = "input", label = "Input" },
					{ id = "output", label = "Output" },
					{ id = "cache", label = "Cache" },
				},
			},
			ui.row {
				gap = "normal",
				switch { on = state.axis, label = "Scale", small = true, msg = "toggle-axis" },
				switch { on = state.legend, label = "Legend", small = true, msg = "toggle-legend" },
			},
			bar_chart {
				height = 120,
				axis = state.axis,
				legend = state.legend,
				unit = "k",
				series = series,
				buckets = buckets,
			},
		}
	end,
}
