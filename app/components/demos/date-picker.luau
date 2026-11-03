-- A task's due date, set two ways: the calendar drawn inline, and the date
-- pill that opens it in a popover. Pick a day or a preset, move between
-- months with ‹ ›, and Today brings this month back. A date before today
-- shows the pill in red. Luau's sandbox has no clock, so "today" is fixed
-- here; a real app gets it from a provider.
local date_picker = use "date-picker"

local TODAY = { year = 2026, month = 11, day = 3 }

local MONTHS = {
	"January",
	"February",
	"March",
	"April",
	"May",
	"June",
	"July",
	"August",
	"September",
	"October",
	"November",
	"December",
}
local WEEKDAYS = { "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun" }

-- Days since 1970-01-01 of a date, and back (Howard Hinnant's algorithms).
local function days_from(year: number, month: number, day: number): number
	local y = if month <= 2 then year - 1 else year
	local era = y // 400
	local yoe = y - era * 400
	local doy = (153 * (if month > 2 then month - 3 else month + 9) + 2) // 5 + day - 1
	local doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
	return era * 146097 + doe - 719468
end

local function date_of(days: number): (number, number, number)
	local z = days + 719468
	local era = z // 146097
	local doe = z - era * 146097
	local yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
	local doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
	local mp = (5 * doy + 2) // 153
	local day = doy - (153 * mp + 2) // 5 + 1
	local month = if mp < 10 then mp + 3 else mp - 9
	return yoe + era * 400 + (if month <= 2 then 1 else 0), month, day
end

-- 1 for Monday to 7 for Sunday (1970-01-01 was a Thursday).
local function weekday(days: number): number
	return (days + 3) % 7 + 1
end

local function id_of(days: number): string
	local year, month, day = date_of(days)
	return string.format("%04d-%02d-%02d", year, month, day)
end

-- "Nov 14", and the year too when it is not this one.
local function label(days: number): string
	local year, month, day = date_of(days)
	local text = `{string.sub(MONTHS[month], 1, 3)} {day}`
	return if year == TODAY.year then text else `{text}, {year}`
end

local today = days_from(TODAY.year, TODAY.month, TODAY.day)

-- The weeks of a month, Monday first, with the days around it to fill them.
local function weeks(year: number, month: number): { any }
	local first = days_from(year, month, 1)
	local next_year, next_month = if month == 12 then year + 1 else year, if month == 12 then 1 else month + 1
	local last = days_from(next_year, next_month, 1) - 1
	local start = first - (weekday(first) - 1)
	local finish = last + (7 - weekday(last))
	local list = {}
	for days = start, finish, 7 do
		local week = {}
		for at = days, days + 6 do
			local _, _, day = date_of(at)
			table.insert(week, {
				id = id_of(at),
				day = tostring(day),
				today = at == today,
				outside = at < first or at > last,
			})
		end
		table.insert(list, week)
	end
	return list
end

type State = { year: number, month: number, due: number?, open: boolean }

-- The presets, each a day (or none), worked out from today.
local PRESETS: { { id: string, label: string, days: number? } } = {
	{ id = "today", label = "Today", days = today },
	{ id = "tomorrow", label = "Tomorrow", days = today + 1 },
	{ id = "next-week", label = "Next week", days = today + 8 - weekday(today) },
	{ id = "none", label = "No date" },
}

local function show(state: State, days: number)
	local year, month = date_of(days)
	state.year, state.month = year, month
end

return mod {
	init = function(previous: State?): State
		return previous or { year = 2026, month = 11, due = days_from(2026, 10, 30), open = false }
	end,

	update = function(state: State, msg)
		if msg == "toggle" then
			state.open = not state.open
		elseif msg == "close" then
			state.open = false
		elseif msg == "previous" or msg == "next" then
			state.month += if msg == "previous" then -1 else 1
			if state.month < 1 then
				state.year, state.month = state.year - 1, 12
			elseif state.month > 12 then
				state.year, state.month = state.year + 1, 1
			end
		elseif msg == "today" then
			show(state, today)
		elseif type(msg) ~= "table" then
			return
		elseif msg.select then
			local year, month, day = string.match(msg.select, "(%d+)-(%d+)-(%d+)")
			state.due = days_from(tonumber(year) :: number, tonumber(month) :: number, tonumber(day) :: number)
			show(state, state.due :: number)
			state.open = false
		elseif msg.preset then
			for _, preset in PRESETS do
				if preset.id == msg.preset then
					state.due = preset.days
					if preset.days then
						show(state, preset.days)
					end
				end
			end
			state.open = false
		end
	end,

	view = function(state: State)
		local presets = {}
		for _, preset in PRESETS do
			table.insert(presets, {
				id = preset.id,
				label = preset.label,
				detail = if preset.days then WEEKDAYS[weekday(preset.days)] else nil,
			})
		end
		local calendar = {
			title = `{MONTHS[state.month]} {state.year}`,
			weeks = weeks(state.year, state.month),
			selected = if state.due then id_of(state.due) else nil,
			previous = "previous",
			next = "next",
			today = "today",
			presets = presets,
		}

		local pill = table.clone(calendar)
		pill.toggle = "toggle"
		pill.open = state.open
		pill.dismiss = "close"
		pill.value = if state.due then label(state.due) else nil
		pill.overdue = state.due ~= nil and state.due < today

		local inline = table.clone(calendar)
		inline.presets_side = "bottom"

		return ui.column {
			gap = "large",
			ui.row {
				align = "center",
				gap = "small",
				ui.row { width = 90, ui.text { "Due date", tone = "muted" } },
				date_picker(pill),
			},
			ui.column {
				width = 380,
				style = { background = "card", border = "all", border_color = "border", radius = 10 },
				date_picker(inline),
			},
		}
	end,
}
