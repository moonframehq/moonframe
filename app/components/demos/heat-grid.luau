-- Lessons per week over the last twelve weeks: "Log a lesson" adds one to
-- this week's column, and the switch shows the grid by counts (filled from
-- the bottom) or by levels (each square shaded on its own).
local heat_grid = use "heat-grid"
local segmented = use "segmented"

return mod {
	init = function(previous)
		return previous or { weeks = { 1, 0, 1, 0, 0, 2, 1, 0, 3, 1, 1, 1 }, mode = "counts" }
	end,

	update = function(state, msg)
		if msg == "log" then
			local last = #state.weeks
			state.weeks[last] = (state.weeks[last] + 1) % 4
		elseif type(msg) == "table" and msg.select then
			state.mode = msg.select
		end
	end,

	view = function(state)
		local columns: { any } = state.weeks
		local levels = 4
		if state.mode == "levels" then
			-- Five weekdays a week, each shaded by how much was done that day.
			columns = {}
			for week, count in state.weeks do
				local days = {}
				for day = 1, 5 do
					days[day] = if (week * 3 + day * 5) % 7 < count * 2 then (week + day) % levels + 1 else 0
				end
				columns[week] = days
			end
		end
		return ui.column {
			gap = "large",
			width = 340,
			ui.row {
				align = "center",
				ui.text { "Lessons per week", weight = "medium" },
				ui.spacer(),
				segmented {
					small = true,
					selected = state.mode,
					options = { { id = "counts", label = "Counts" }, { id = "levels", label = "Levels" } },
				},
			},
			heat_grid {
				rows = if state.mode == "counts" then 3 else 5,
				levels = levels,
				size = 16,
				spread = true,
				columns = columns,
				labels = { "Aug 10", "This week" },
			},
			ui.row { ui.button { "Log a lesson", icon = "plus", variant = "outline", small = true, msg = "log" } },
		}
	end,
}
