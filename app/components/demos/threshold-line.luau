-- Latency against a 2 s limit: "Sample" adds a reading on the right (the
-- oldest drops off), "Spike" adds one past the limit, which turns red.
local threshold_line = use "threshold-line"

local HISTORY = { 1.4, 1.2, 0.9, 0.85, 0.8, 1.1, 1.05, 1.15, 1.3, 1.7 }

return mod {
	init = function(previous)
		return previous or { values = table.clone(HISTORY), seed = 11 }
	end,

	update = function(state, msg)
		local value
		if msg == "sample" then
			-- A small pseudo-random walk, so the demo needs no clock.
			state.seed = (state.seed * 1103515245 + 12345) % 2147483648
			local last = state.values[#state.values] or 1.2
			value = math.clamp(last + (state.seed % 41 - 20) / 50, 0.6, 1.9)
		elseif msg == "spike" then
			value = 2.6
		end
		if value then
			table.insert(state.values, value)
			table.remove(state.values, 1)
		end
	end,

	view = function(state)
		local last = state.values[#state.values]
		return ui.column {
			gap = "large",
			ui.row {
				gap = "large",
				align = "center",
				ui.text { string.format("%.1fs", last), size = "title", weight = "medium" },
				threshold_line { points = state.values, threshold = 2, label = "2S", breach = "above", width = 260, height = 60 },
			},
			ui.row {
				gap = "small",
				ui.button { "Sample", small = true, msg = "sample" },
				ui.button { "Spike", small = true, variant = "outline", msg = "spike" },
			},
		}
	end,
}
