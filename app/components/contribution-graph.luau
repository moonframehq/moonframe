-- A contribution calendar as GitHub draws one: a square per day, a column
-- per week, shaded by how much happened that day, with the months across
-- the top, Mon, Wed and Fri down the side, a tooltip on every day ("No
-- contributions on May 14th.") and a Less … More legend under it.
--
-- The mod passes one count per day, oldest first, and the date of the
-- first (views have no clock: the mod or a provider knows the dates). The
-- component works out the weekdays, the weeks and the months itself.

type Props = {
	days: { number },
	start: string,
	thresholds: { number }?,
	palette: { string }?,
	color: string?,
	empty: string?,
	size: number?,
	gap: number?,
	week_start: string?,
	weekdays: boolean?,
	months: boolean?,
	legend: boolean?,
	noun: string?,
	nouns: string?,
	note: string?,
	note_msg: any?,
	selected: string?,
	event: string?,
	to: string?,
}

local MONTHS = { "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec" }
local MONTH_NAMES = {
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
local WEEKDAYS = { "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat" }

-- Days since 1970-01-01 for a date, and back (Howard Hinnant's algorithms).
local function days_from_civil(y: number, m: number, d: number): number
	if m <= 2 then
		y -= 1
	end
	local era = (if y >= 0 then y else y - 399) // 400
	local yoe = y - era * 400
	local mp = (m + 9) % 12
	local doy = (153 * mp + 2) // 5 + d - 1
	local doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
	return era * 146097 + doe - 719468
end

local function civil_from_days(z: number): (number, number, number)
	z += 719468
	local era = (if z >= 0 then z else z - 146096) // 146097
	local doe = z - era * 146097
	local yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
	local doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
	local mp = (5 * doy + 2) // 153
	local d = doy - (153 * mp + 2) // 5 + 1
	local m = if mp < 10 then mp + 3 else mp - 9
	local y = yoe + era * 400 + (if m <= 2 then 1 else 0)
	return y, m, d
end

local function parse(date: string): number
	local y, m, d = string.match(date, "^(%d+)-(%d+)-(%d+)$")
	assert(y and m and d, `contribution-graph: start must be "YYYY-MM-DD", got {date}`)
	return days_from_civil(tonumber(y) :: number, tonumber(m) :: number, tonumber(d) :: number)
end

local function iso(z: number): string
	local y, m, d = civil_from_days(z)
	return string.format("%04d-%02d-%02d", y, m, d)
end

local function ordinal(n: number): string
	local tens = n % 100
	if tens >= 11 and tens <= 13 then
		return `{n}th`
	end
	local last = n % 10
	return `{n}{if last == 1 then "st" elseif last == 2 then "nd" elseif last == 3 then "rd" else "th"}`
end

-- The level of a count, 0 for none: past the thresholds given, else split
-- into quarters of the busiest day.
local function level_of(count: number, thresholds: { number }?, top: number): number
	if count <= 0 then
		return 0
	end
	if thresholds then
		local level = 0
		for index, minimum in thresholds do
			if count >= minimum then
				level = index
			end
		end
		return math.max(level, 1)
	end
	return math.clamp(math.ceil(count / math.max(top, 1) * 4), 1, 4)
end

return component {
	title = "Contribution Graph",
	category = "Data",
	icon = "calendar",
	description = "A year of days as squares in week columns, shaded by count, with months on top, weekdays on the side, a tooltip per day and a Less … More legend.",
	when = "Daily activity over months or a year, GitHub style: commits, agent runs, posts, workouts, a streak. Pass one count per day and the first day's date. For weekly or arbitrary buckets without dates use `heat-grid`; for counts with an axis use `bar-chart`; for picking a day use `month-grid`.",
	props = {
		["days"] = "one count per day, oldest first",
		["start"] = "the first day's date, \"YYYY-MM-DD\"",
		["thresholds"] = "the smallest count of each level, e.g. { 1, 3, 6, 10 }; default quarters of the busiest day",
		["palette"] = "colours from empty to busiest, e.g. GitHub's { \"#161b22\", \"#0e4429\", \"#006d32\", \"#26a641\", \"#39d353\" }; one more than the thresholds",
		["color"] = "the busiest colour, lighter levels tinted from it (default success)",
		["empty"] = "a day with nothing (default muted-foreground at 12%)",
		["size"] = "a square's side in pixels (default 11)",
		["gap"] = "pixels between squares (default 3)",
		["week_start"] = "\"sunday\" (default) or \"monday\": the top row",
		["weekdays"] = "false hides the Mon / Wed / Fri labels",
		["months"] = "false hides the month labels",
		["legend"] = "false hides the Less … More legend",
		["noun"] = "what is counted (default \"contribution\")",
		["nouns"] = "its plural (default the noun with an s)",
		["note"] = "muted text on the legend's left, e.g. \"Learn how we count contributions\"",
		["note_msg"] = "message a click on the note sends",
		["selected"] = "the chosen day, \"YYYY-MM-DD\", outlined",
		["event"] = "key of the message a day sends: { [event] = \"YYYY-MM-DD\" }; days are not clickable without it",
		["to"] = "mod to send it to, if not the drawing one",
	},
	styles = {
		[".contribution-graph"] = { gap = 6 },
		[".contribution-graph-cell"] = { radius = 2, border = "all", border_color = "#0000000d" },
		[".contribution-graph-cell:hover"] = { border_color = "foreground/40" },
		[".contribution-graph-cell:selected"] = { border_color = "foreground" },
		[".contribution-graph-month"] = { size = "small", color = "foreground" },
		[".contribution-graph-weekday"] = { size = "small", color = "foreground" },
		[".contribution-graph-legend"] = { size = "small", color = "muted-foreground" },
		[".contribution-graph-note"] = { size = "small", color = "muted-foreground" },
		[".contribution-graph-note:hover"] = { color = "accent" },
	},
	example = {
		start = "2026-08-02",
		days = {
			0, 1, 3, 0, 2, 5, 0, 0, 4, 6, 2, 1, 0, 0, 3, 8, 9, 2, 0, 1, 0,
			0, 2, 4, 6, 1, 0, 0, 5, 7, 3, 2, 4, 0, 1, 0, 0, 2, 9, 4, 3, 0,
			1, 3, 0, 6, 2, 0, 0, 4, 1, 3, 7, 5, 0, 0, 2, 2, 8, 4, 1, 0, 0,
			3, 5, 1, 0, 6, 2, 0, 1, 4, 9, 3, 2, 0, 0, 2, 1, 5, 7, 4, 0, 0,
			1, 2, 3,
		},
		note = "Learn how we count contributions",
	},
	usage = [[
local contribution_graph = use "contribution-graph"

contribution_graph {
	start = "2025-11-02",       -- the first count's date
	days = state.counts,        -- one count per day, oldest first
	note = "Learn how we count contributions", note_msg = "help",
	event = "day", selected = state.day,   -- a click sends { day = "2026-05-14" }
	-- palette = { "#161b22", "#0e4429", "#006d32", "#26a641", "#39d353" },
}
]],

	build = function(props: Props)
		local size = props.size or 11
		local gap = props.gap or 3
		local first = parse(props.start)
		local count = #props.days
		local monday = props.week_start == "monday"
		local noun = props.noun or "contribution"
		local nouns = props.nouns or `{noun}s`

		local top = 0
		for _, n in props.days do
			top = math.max(top, n)
		end
		local levels = if props.thresholds then #props.thresholds else 4
		local palette: { string } = props.palette or {}
		if not props.palette then
			local color = props.color or "success"
			palette[1] = props.empty or "muted-foreground/12"
			for level = 1, levels do
				palette[level + 1] = if level == levels
					then color
					else `{color}/{math.floor(25 + 75 * (level - 1) / math.max(levels - 1, 1))}`
			end
		end

		-- Row 1 to 7 of a day, from the week's first day.
		local function row_of(z: number): number
			local weekday = (z + 4) % 7 -- 0 is Sunday
			return (if monday then (weekday + 6) % 7 else weekday) + 1
		end
		local lead = row_of(first) - 1
		local weeks = (lead + count + 6) // 7

		local columns = {}
		local labels = {}
		local last_month = nil
		local label_start = 1
		local label_text = nil
		local function close_label(upto: number)
			local span = upto - label_start
			if span > 0 then
				-- A month too narrow for its name at the start is left blank.
				local text = if label_text and (span >= 3 or label_start > 1) then label_text else nil
				table.insert(labels, ui.row {
					width = span * (size + gap),
					shrink = false,
					text and ui.text { text, class = "contribution-graph-month", truncate = true },
				})
			end
		end

		for week = 1, weeks do
			local cells = {}
			for row = 1, 7 do
				local index = (week - 1) * 7 + row - lead
				if index < 1 then
					table.insert(cells, ui.row { width = size, height = size, shrink = false })
				elseif index <= count then
					local z = first + index - 1
					local n = props.days[index]
					local date = iso(z)
					local _, m, d = civil_from_days(z)
					local what = if n == 0 then `No {nouns}` elseif n == 1 then `1 {noun}` else `{n} {nouns}`
					table.insert(cells, ui.row {
						class = "contribution-graph-cell",
						width = size,
						height = size,
						shrink = false,
						selected = props.selected == date,
						style = { background = palette[level_of(n, props.thresholds, top) + 1] or palette[#palette] },
						tooltip = `{what} on {MONTH_NAMES[m]} {ordinal(d)}.`,
						msg = if props.event then { [props.event] = date } else nil,
						to = props.to,
					})
				end
			end
			table.insert(columns, ui.column { style = { gap = gap }, shrink = false, table.unpack(cells) })

			-- The month of the week's first day in range names its column.
			local z = first + math.max((week - 1) * 7 - lead, 0)
			local _, m = civil_from_days(z)
			if m ~= last_month then
				if last_month then
					close_label(week)
				end
				last_month = m
				label_start = week
				label_text = MONTHS[m]
			end
		end
		close_label(weeks + 1)

		local day_labels = {}
		for row = 1, 7 do
			local weekday = if monday then row % 7 + 1 else row
			local name = WEEKDAYS[weekday]
			local shown = name == "Mon" or name == "Wed" or name == "Fri"
			table.insert(day_labels, ui.row {
				height = size,
				align = "center",
				shown and ui.text { name, class = "contribution-graph-weekday" },
			})
		end

		local legend_row: { any } =
			{ ui.text { "Less", class = "contribution-graph-legend", style = { margin = { right = 4 } } } }
		for _, fill in palette do
			table.insert(legend_row, ui.row {
				class = "contribution-graph-cell",
				width = size,
				height = size,
				shrink = false,
				style = { background = fill },
			})
		end
		table.insert(
			legend_row,
			ui.text { "More", class = "contribution-graph-legend", style = { margin = { left = 4 } } }
		)

		local grid = ui.column {
			gap = "none",
			hug = true,
			props.months ~= false and ui.row { gap = "none", style = { margin = { bottom = 4 } }, table.unpack(labels) },
			ui.row { style = { gap = gap }, table.unpack(columns) },
		}

		-- Wider than its parent (a year in a narrow pane), the grid scrolls
		-- sideways while the legend stays put.
		return ui.column {
			class = "contribution-graph",
			max_width = 2 * 8 + 32 + weeks * (size + gap),
			ui.row {
				gap = "none",
				align = "end",
				scroll = "x",
				props.weekdays ~= false and ui.column {
					style = { gap = gap, padding = { right = 8 } },
					shrink = false,
					table.unpack(day_labels),
				},
				grid,
			},
			props.legend ~= false and ui.row {
				align = "center",
				style = { gap = gap },
				props.note and ui.row {
					class = "contribution-graph-note",
					msg = props.note_msg,
					to = if props.note_msg then props.to else nil,
					ui.text { props.note },
				},
				ui.spacer(),
				ui.row { align = "center", style = { gap = gap }, table.unpack(legend_row) },
			},
		}
	end,
}
