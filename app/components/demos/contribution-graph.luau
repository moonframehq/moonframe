-- A year of contributions (demo data, made up from a seed): click a day to
-- pick it, and switch between the theme's colours and GitHub's greens.
local contribution_graph = use "contribution-graph"
local segmented = use "segmented"

local GITHUB = { "#161b22", "#0e4429", "#006d32", "#26a641", "#39d353" }

-- 365 days of demo counts: quiet weekends, busier stretches now and then.
local function demo_days(): { number }
	local days = {}
	local seed = 7
	for day = 1, 365 do
		seed = (seed * 1103515245 + 12345) % 2147483648
		local roll = seed // 65536 % 100
		local weekend = (day + 6) % 7 >= 5
		local busy = (day // 30) % 3 == 1
		local n = 0
		if roll < (if weekend then 25 else 70) then
			n = roll % (if busy then 12 else 5) + 1
		end
		days[day] = n
	end
	return days
end

return mod {
	init = function(previous)
		return previous or { day = nil, look = "theme" }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.day then
			state.day = if state.day == msg.day then nil else msg.day
		elseif type(msg) == "table" and msg.select then
			state.look = msg.select
		end
	end,

	view = function(state)
		local days = ui.memo("days", {}, demo_days)
		local total = 0
		for _, n in days do
			total += n
		end
		return ui.column {
			gap = "large",
			ui.row {
				align = "center",
				ui.text { `{total} contributions in the last year`, weight = "medium" },
				ui.spacer(),
				segmented {
					small = true,
					selected = state.look,
					options = { { id = "theme", label = "Theme" }, { id = "github", label = "GitHub" } },
				},
			},
			contribution_graph {
				start = "2025-11-02",
				days = days,
				palette = if state.look == "github" then GITHUB else nil,
				note = "Learn how we count contributions",
				event = "day",
				selected = state.day,
			},
			ui.text { if state.day then `Picked {state.day}` else "Click a day to pick it.", tone = "muted" },
		}
	end,
}
