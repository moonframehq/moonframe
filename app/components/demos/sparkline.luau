-- CPU over the last samples: "Sample" adds a reading on the right,
-- "Spike" adds a hot one (past 80% it shows in the warning colour).
local sparkline = use "sparkline"

local HISTORY = { 0.1, 0.3, 0.2, 0.5, 0.4, 0.9, 0.6, 0.3, 0.2, 0.4, 0.7, 0.5 }

return mod {
	init = function(previous)
		return previous or { values = table.clone(HISTORY), seed = 7 }
	end,

	update = function(state, msg)
		local value
		if msg == "sample" then
			-- A small pseudo-random walk, so the demo needs no clock.
			state.seed = (state.seed * 1103515245 + 12345) % 2147483648
			local last = state.values[#state.values] or 0.3
			value = math.clamp(last + (state.seed % 41 - 20) / 100, 0.05, 0.75)
		elseif msg == "spike" then
			value = 0.95
		elseif msg == "clear" then
			state.values = {}
		end
		if value then
			table.insert(state.values, value)
			while #state.values > 24 do
				table.remove(state.values, 1)
			end
		end
	end,

	view = function(state)
		local last = state.values[#state.values] or 0
		return ui.column {
			gap = "large",
			ui.row {
				gap = "normal",
				align = "center",
				ui.icon "cpu",
				sparkline { values = state.values, samples = 24, height = 18, hot = 0.8 },
				ui.text { `{math.round(last * 100)}%`, tone = if last > 0.8 then "warning" else "muted" },
			},
			ui.row {
				gap = "small",
				ui.button { "Sample", small = true, msg = "sample" },
				ui.button { "Spike", small = true, variant = "outline", msg = "spike" },
				ui.button { "Clear", small = true, variant = "ghost", msg = "clear" },
			},
		}
	end,
}
