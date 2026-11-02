-- Send Later: pick a day in November 2026, then a time with the arrows,
-- AM / PM or a preset; the line under it says when the message will go.
local month_grid = use "month-grid"
local time_picker = use "time-picker"

-- November 2026, Monday first: it starts on a Sunday, so the first week
-- begins with the last six days of October.
local function weeks(): { any }
	local list: { any } = {}
	local week: { any } = {}
	for day = 26, 31 do
		table.insert(week, { id = `2026-10-{day}`, day = tostring(day), outside = true })
	end
	for day = 1, 30 do
		table.insert(week, { id = string.format("2026-11-%02d", day), day = tostring(day), today = day == 1 })
		if #week == 7 then
			table.insert(list, week)
			week = {}
		end
	end
	local next = 1
	while #week > 0 and #week < 7 do
		table.insert(week, { id = string.format("2026-12-%02d", next), day = tostring(next), outside = true })
		next += 1
	end
	if #week > 0 then
		table.insert(list, week)
	end
	return list
end

local WEEKS = weeks()

local function when(hour: number, minute: number): string
	local shown = hour % 12
	return string.format("%d:%02d %s", if shown == 0 then 12 else shown, minute, if hour < 12 then "AM" else "PM")
end

return mod {
	init = function(previous)
		return previous or { day = "2026-11-02", hour = 9, minute = 0 }
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		if msg.select then
			state.day = msg.select
		elseif msg.time then
			state.hour = msg.time.hour
			state.minute = msg.time.minute
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			ui.row {
				gap = "large",
				align = "start",
				ui.column { width = 280, month_grid { weeks = WEEKS, selected = state.day } },
				time_picker {
					hour = state.hour,
					minute = state.minute,
					step = 15,
					presets = {
						{ hour = 9, minute = 0, label = "Morning" },
						{ hour = 13, minute = 0, label = "Afternoon" },
						{ hour = 18, minute = 0, label = "Evening" },
					},
				},
			},
			ui.text { `Sends on {state.day} at {when(state.hour, state.minute)}`, tone = "muted" },
		}
	end,
}
