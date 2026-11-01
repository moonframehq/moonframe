-- Revenue by month: click a month to move the spotlight to it, and the
-- figure above follows.
local spotlight_bars = use "spotlight-bars"

local MONTHS: { { any } } = {
	{ "JAN", 90.2 },
	{ "FEB", 145.4 },
	{ "MAR", 116.8 },
	{ "APR", 136.2 },
	{ "MAY", 193.2 },
	{ "JUN", 158.2 },
	{ "JUL", 112.2 },
	{ "AUG", 170.2 },
}

return mod {
	init = function(previous)
		return previous or { month = 5 }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.select then
			state.month = msg.select
		end
	end,

	view = function(state)
		local bars = {}
		for index, month in MONTHS do
			bars[index] = { label = month[1], value = month[2], tooltip = `${month[2]}k` }
		end
		local chosen = MONTHS[state.month]
		return ui.column {
			gap = "large",
			padding = "large",
			ui.text { `{chosen[1]} · ${chosen[2]}k`, weight = "medium", style = { font = "mono" } },
			spotlight_bars { bars = bars, selected = state.month, height = 160 },
		}
	end,
}
