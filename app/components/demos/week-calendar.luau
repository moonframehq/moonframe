-- A week of meetings (sample data): click an event to pick it, switch Day,
-- Week and Month, click a day's heading to open that day. A real calendar
-- takes its dates from a provider, which knows what today is.
local week_calendar = use "week-calendar"

-- November 2026, Sunday first; the 3rd is "today".
local EVENTS = {
	{ id = "e1", day = "2026-11-02", start = 8, finish = 9, title = "Product demo - Kestrel", mark = "video" },
	{ id = "e2", day = "2026-11-03", start = 9, finish = 11, title = "Roadmap review", mark = "grid" },
	{ id = "e3", day = "2026-11-03", start = 10, finish = 11.5, title = "Pricing sync", mark = "call" },
	{ id = "e4", day = "2026-11-04", start = 12, finish = 13, title = "Discovery call", mark = "meet" },
	{ id = "e5", day = "2026-11-05", start = 9.5, finish = 10.5, title = "Weekly sync", mark = "chat" },
	{ id = "e6", day = "2026-11-12", start = 14, finish = 15, title = "Onboarding kickoff", mark = "meet" },
	{ id = "e7", day = "2026-11-18", start = 10, finish = 11, title = "Renewal check-in", mark = "video" },
}

local function clock(hour: number): string
	local h = math.floor(hour)
	local twelve = if h % 12 == 0 then 12 else h % 12
	return string.format("%d:%02d %s", twelve, math.floor((hour - h) * 60), if h < 12 then "AM" else "PM")
end

return mod {
	init = function(previous)
		return previous or { view = "week", selected = "e2", day = "2026-11-03" }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.selected = msg.select
		elseif msg.view then
			state.view = msg.view
		elseif msg.day then
			state.day = msg.day
			state.view = "day"
		end
	end,

	view = function(state)
		local events = {}
		for _, event in EVENTS do
			table.insert(events, {
				id = event.id,
				day = event.day,
				start = event.start,
				finish = event.finish,
				title = event.title,
				mark = event.mark,
				caption = "Meeting Time",
				time = `{clock(event.start)} - {clock(event.finish)}`,
			})
		end
		local names = { "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat" }
		local days = {}
		for n = 2, 6 do
			local id = string.format("2026-11-%02d", n)
			if state.view == "week" or id == state.day then
				table.insert(days, { id = id, label = `{names[n]} {n}`, today = n == 3 })
			end
		end
		local weeks = {}
		local n = 1
		for _ = 1, 5 do
			local week = {}
			for _ = 1, 7 do
				local day = (n - 1) % 30 + 1
				table.insert(week, { id = string.format("2026-%02d-%02d", if n > 30 then 12 else 11, day), day = tostring(day), today = n == 3, outside = n > 30 })
				n += 1
			end
			table.insert(weeks, week)
		end
		return week_calendar {
			title = if state.view == "day" then "November 3, 2026" else "November 2026",
			view = state.view,
			today = "today",
			previous = "previous",
			next = "next",
			days = days,
			weeks = weeks,
			events = events,
			selected = state.selected,
			hours = { from = 8, to = 14 },
			hour_height = 76,
			column_width = 200,
			day_width = 900,
			now = { day = "2026-11-03", hour = 12.25 },
			height = 560,
		}
	end,
}
