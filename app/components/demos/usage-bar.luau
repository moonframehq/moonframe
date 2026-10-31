-- A weekly quota: Use 10% spends some and the bar turns warning, then
-- danger; Reset fills it again.
local usage_bar = use "usage-bar"

return mod {
	init = function(previous)
		return previous or { left = 0.97 }
	end,

	update = function(state, msg)
		if msg == "use" then
			state.left = math.max(0, state.left - 0.1)
		elseif msg == "reset" then
			state.left = 1
		end
	end,

	view = function(state)
		local pace = 0.61
		local reserve = math.floor((state.left - (1 - pace)) * 100 + 0.5)
		return ui.column {
			gap = "large",
			ui.row {
				gap = "small",
				ui.button { "Use 10%", small = true, variant = "outline", msg = "use" },
				ui.button { "Reset", small = true, variant = "ghost", msg = "reset" },
			},
			usage_bar {
				title = "Weekly",
				left = state.left,
				pace = pace,
				resets = "Resets in 4d 6h",
				note = if reserve >= 0 then `{reserve}% in reserve · Lasts until reset` else `{-reserve}% over pace · Runs out early`,
				width = 320,
			},
			usage_bar { title = "5-hour", left = 0.42, pace = 0.3, resets = "Resets in 2h 10m", width = 320 },
		}
	end,
}
