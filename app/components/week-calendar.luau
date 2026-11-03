-- A calendar card: a header (the month, Today and arrows, Day / Week /
-- Month), then a time grid for a day or a week, with events as cards that
-- span their hours, or a month of weeks with events as chips. The mod
-- passes the days (a provider knows the calendar) and the events, and gets
-- { select = id } for an event, { view = id } for the switch and
-- { day = id } for a day's heading. Overlapping events share their column.

type Day = { id: string, label: string?, day: string?, today: boolean?, outside: boolean? }

type Event = {
	id: string,
	day: string,
	start: number,
	finish: number,
	title: string,
	caption: string?,
	time: string?,
	mark: any,
	tooltip: string?,
}

type Props = {
	view: string?,
	views: any,
	title: string?,
	today: any,
	previous: any,
	next: any,
	days: { Day }?,
	weeks: { { Day } }?,
	weekdays: { string }?,
	events: { Event }?,
	selected: string?,
	hours: { from: number?, to: number? }?,
	hour_height: number?,
	column_width: number?,
	day_width: number?,
	gutter: number?,
	now: { day: string, hour: number }?,
	more: number?,
	event: string?,
	switch: string?,
	pick_day: string?,
	move: string?,
	over: string?,
	to: string?,
	grow: boolean?,
	height: number?,
	header: boolean?,
}

local segmented = use "segmented"

local VIEWS: { { id: string, label: string } } = {
	{ id = "day", label = "Day" },
	{ id = "week", label = "Week" },
	{ id = "month", label = "Month" },
}

local WEEKDAYS: { string } = { "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat" }

local function hour_label(hour: number): string
	local h = hour % 24
	local suffix = if h < 12 then "AM" else "PM"
	local twelve = h % 12
	if twelve == 0 then
		twelve = 12
	end
	return `{twelve} {suffix}`
end

-- A small camera drawn from boxes: a body and a lens.
local function camera(color: string): any
	return ui.row {
		gap = "none",
		align = "center",
		ui.row { width = 7, height = 6, style = { background = color, radius = 2 } },
		ui.row { width = 3, height = 4, style = { background = color, radius = { 0, 1, 1, 0 } } },
	}
end

-- The marks every calendar draws without a picture: generic call providers.
local function mark_of(mark: any, size: number): any
	if type(mark) ~= "string" then
		return mark
	end
	local function tile(class: string, ...): any
		return ui.row {
			class = { "week-calendar-mark", class },
			width = size,
			height = size,
			shrink = false,
			align = "center",
			justify = "center",
			...,
		}
	end
	if mark == "video" then
		return tile("week-calendar-mark-video", camera("#ffffff"))
	elseif mark == "meet" then
		return tile("week-calendar-mark-meet", camera("#ffffff"))
	elseif mark == "chat" then
		return tile("week-calendar-mark-chat", ui.icon { name = "message-square", size = "small" })
	elseif mark == "call" then
		return tile("week-calendar-mark-call", ui.icon { name = "mic", size = "small" })
	elseif mark == "grid" then
		local square = function(color: string)
			return ui.row { width = size / 2 - 1, height = size / 2 - 1, style = { background = color, radius = 1 } }
		end
		return ui.column {
			width = size,
			height = size,
			shrink = false,
			style = { gap = 2 },
			ui.row { style = { gap = 2 }, square("#f25022"), square("#7fba00") },
			ui.row { style = { gap = 2 }, square("#00a4ef"), square("#ffb900") },
		}
	end
	return tile("week-calendar-mark-plain", ui.icon { name = mark :: any, size = "small" })
end

-- Lanes for events that overlap in one day: each gets { lane, lanes }.
local function lanes_of(events: { Event }): { [string]: { lane: number, lanes: number } }
	local sorted = table.clone(events)
	table.sort(sorted, function(a, b)
		return a.start < b.start or (a.start == b.start and a.finish > b.finish)
	end)
	local out = {}
	local group: { Event } = {}
	local ends: { number } = {}
	local group_end = -1
	local function close()
		for _, event in group do
			out[event.id].lanes = #ends
		end
		group = {}
		ends = {}
	end
	for _, event in sorted do
		if event.start >= group_end then
			close()
			group_end = -1
		end
		local lane = 0
		for index, finish in ends do
			if finish <= event.start then
				lane = index
				break
			end
		end
		if lane == 0 then
			table.insert(ends, event.finish)
			lane = #ends
		else
			ends[lane] = event.finish
		end
		out[event.id] = { lane = lane, lanes = 1 }
		table.insert(group, event)
		group_end = math.max(group_end, event.finish)
	end
	close()
	return out
end

local function grid_view(props: Props, events: { Event }): any
	local days: { Day } = props.days or {}
	local from = props.hours and props.hours.from or 8
	local to_hour = props.hours and props.hours.to or 18
	local hour = props.hour_height or 96
	local gutter = props.gutter or 64
	local width = if #days == 1 then (props.day_width or 560) else (props.column_width or 184)
	local total = gutter + width * #days
	local key = props.event or "select"
	local pick = props.pick_day or "day"

	-- The day headings, with the arrows over the hours' gutter.
	local heads = {
		ui.row {
			width = gutter,
			shrink = false,
			align = "center",
			class = "week-calendar-arrows",
			props.previous ~= nil and ui.row { class = "week-calendar-arrow", msg = props.previous, to = props.to, ui.icon { name = "chevron-left", size = "small" } },
			props.next ~= nil and ui.row { class = "week-calendar-arrow", msg = props.next, to = props.to, ui.icon { name = "chevron-right", size = "small" } },
		},
	}
	local column_of: { [string]: number } = {}
	for index, day in days do
		column_of[day.id] = index - 1
		table.insert(heads, ui.row {
			class = if day.today then { "week-calendar-head", "week-calendar-head-today" } else "week-calendar-head",
			width = width,
			shrink = false,
			justify = "center",
			align = "center",
			msg = { [pick] = day.id },
			to = props.to,
			ui.text { day.label or day.day or day.id, class = if day.today then { "week-calendar-head-text", "week-calendar-today" } else "week-calendar-head-text" },
		})
	end

	-- The hours: a line over each, its label in the gutter.
	local rows = {}
	for h = from, to_hour - 1 do
		table.insert(rows, ui.row {
			class = "week-calendar-hour",
			height = hour,
			width = total,
			shrink = false,
			ui.row { width = gutter, class = "week-calendar-hour-label", ui.text { hour_label(h), class = "week-calendar-hour-text" } },
		})
	end

	-- Events float over the hours, in their day's column and lane.
	local by_day: { [string]: { Event } } = {}
	for _, event in events do
		if column_of[event.day] ~= nil then
			by_day[event.day] = by_day[event.day] or {}
			table.insert(by_day[event.day], event)
		end
	end
	local cards = {}
	-- With `move`, each day's column takes events dropped on it.
	if props.move then
		for index, column in days do
			table.insert(cards, ui.column {
				class = "week-calendar-drop",
				state = if props.over == column.id then "over" else nil,
				float = { top = 0, left = gutter + (index - 1) * width },
				width = width,
				height = (to_hour - from) * hour,
				accept = { [props.move] = column.id },
				to = props.to,
			})
		end
	end
	for _, column in days do
		local day = column.id
		local list: { Event } = by_day[day] or {}
		local lanes = lanes_of(list)
		for _, event in list do
			local start = math.max(event.start, from)
			local finish = math.min(event.finish, to_hour)
			if finish > start then
				local place = lanes[event.id]
				local inner = width - 10
				local lane_width = inner / place.lanes
				local h = math.max((finish - start) * hour - 8, 26)
				-- Room for the caption over the time, and for a second line of title.
				local tall = h >= 88
				local narrow = lane_width < 110
				local selected = event.id == props.selected
				table.insert(cards, ui.column {
					class = "week-calendar-event",
					selected = selected,
					float = { top = (start - from) * hour + 4, left = gutter + column_of[day] * width + 5 + (place.lane - 1) * lane_width },
					width = lane_width - (if place.lanes > 1 then 4 else 0),
					height = h,
					msg = { [key] = event.id },
					carry = if props.move then { event = event.id } else nil,
					to = props.to,
					tooltip = event.tooltip,
					ui.row {
						align = "start",
						class = "week-calendar-event-head",
						event.mark ~= nil and not narrow and mark_of(event.mark, 16),
						ui.text { event.title, class = "week-calendar-event-title", lines = if h >= 60 then (if narrow and tall then 3 else 2) else nil, truncate = h < 60 },
					},
					tall and ui.spacer(),
					tall and not narrow and event.caption ~= nil and ui.text { event.caption, class = "week-calendar-event-caption", truncate = true },
					event.time ~= nil and h >= 44 and ui.text { event.time, class = "week-calendar-event-time", truncate = true },
				})
			end
		end
	end

	-- Now, as a line across today's column.
	local now = props.now
	if now and column_of[now.day] ~= nil and now.hour >= from and now.hour <= to_hour then
		local top = (now.hour - from) * hour
		table.insert(cards, ui.row {
			float = { top = top - 4, left = gutter + column_of[now.day] * width - 4 },
			width = width + 4,
			height = 8,
			align = "center",
			gap = "none",
			ui.row { class = "week-calendar-now-dot", width = 8, height = 8 },
			ui.row { class = "week-calendar-now", grow = true, height = 2 },
		})
	end

	for _, card in cards do
		table.insert(rows, card)
	end

	return ui.column {
		grow = true,
		scroll = "x",
		gap = "none",
		ui.column {
			width = total,
			grow = true,
			shrink = false,
			gap = "none",
			ui.row { class = "week-calendar-heads", width = total, shrink = false, height = 52, table.unpack(heads) },
			ui.column {
				grow = true,
				scroll = "y",
				gap = "none",
				ui.column {
					width = total,
					height = (to_hour - from) * hour + 1,
					shrink = false,
					gap = "none",
					class = "week-calendar-grid",
					table.unpack(rows),
				},
			},
		},
	}
end

local function month_view(props: Props, events: { Event }): any
	local key = props.event or "select"
	local pick = props.pick_day or "day"
	local most = props.more or 3
	local by_day: { [string]: { Event } } = {}
	for _, event in events do
		by_day[event.day] = by_day[event.day] or {}
		table.insert(by_day[event.day], event)
	end
	for _, list in by_day do
		table.sort(list, function(a, b)
			return a.start < b.start
		end)
	end
	local names = {}
	for _, name in props.weekdays or WEEKDAYS do
		table.insert(names, ui.row { grow = true, class = "week-calendar-month-name", justify = "center", ui.text { name, class = "week-calendar-head-text" } })
	end
	local rows = { ui.row { class = "week-calendar-heads", height = 44, shrink = false, table.unpack(names) } }
	local weeks: { { Day } } = props.weeks or {}
	for _, week in weeks do
		local cells = {}
		for index, day in week do
			local list: { Event } = by_day[day.id] or {}
			local chips = {}
			for n, event in list do
				if n > most then
					table.insert(chips, ui.text { `+{#list - most} more`, class = "week-calendar-more" })
					break
				end
				table.insert(chips, ui.row {
					class = "week-calendar-chip",
					selected = event.id == props.selected,
					align = "center",
					msg = { [key] = event.id },
					carry = if props.move then { event = event.id } else nil,
					to = props.to,
					tooltip = event.time,
					event.mark ~= nil and mark_of(event.mark, 12),
					ui.text { event.title, class = "week-calendar-chip-text", truncate = true },
				})
			end
			local class = { "week-calendar-cell" }
			if index == 1 then
				table.insert(class, "week-calendar-cell-first")
			end
			if day.outside then
				table.insert(class, "week-calendar-outside")
			end
			table.insert(cells, ui.column {
				class = class,
				state = if props.over == day.id then "over" else nil,
				grow = true,
				width = 1,
				msg = { [pick] = day.id },
				accept = if props.move then { [props.move] = day.id } else nil,
				to = props.to,
				ui.row {
					ui.text { day.day or day.label or day.id, class = if day.today then { "week-calendar-cell-number", "week-calendar-cell-today" } else "week-calendar-cell-number" },
				},
				table.unpack(chips),
			})
		end
		table.insert(rows, ui.row { class = "week-calendar-week", grow = true, table.unpack(cells) })
	end
	return ui.column { grow = true, gap = "none", class = "week-calendar-month", table.unpack(rows) }
end

return component {
	title = "Week Calendar",
	category = "Data",
	icon = "calendar",
	description = "A calendar card: the month, Today and arrows, Day / Week / Month, then hours with events spanning them, or a month with event chips.",
	when = "Meetings, bookings or shifts placed in time: a day or week of hours where events span their length and overlapping ones share the column, or a month of them. For picking a day without events use `month-grid` or `day-strip`; for events in order without hours use `timeline`; for a day's list use `task-row`.",
	props = {
		["view"] = "\"day\", \"week\" (default) or \"month\"",
		["views"] = "the switch's options: ids from day, week, month (default all three); false hides it",
		["title"] = "header title (\"January 2026\"); the header shows with a title or arrows",
		["header"] = "false hides the header",
		["today"] = "message the Today button sends; no button without it",
		["previous"] = "message the back arrows send",
		["next"] = "message the forward arrows send",
		["days"] = "day and week: the columns, { id, label (\"Sun 11\"), today? }",
		["weeks"] = "month: weeks of 7 { id, day (\"11\"), today?, outside? }",
		["weekdays"] = "month: the heading's names (default Sun … Sat)",
		["events"] = "list of { id, day (a day id), start, finish (hours, 9.5 = 9:30), title, caption?, time? (text), mark? (\"video\", \"meet\", \"chat\", \"call\", \"grid\", an icon name, or an element), tooltip? }",
		["selected"] = "id of the chosen event",
		["hours"] = "{ from = 8, to = 18 }: the hours shown",
		["hour_height"] = "pixels per hour (default 96)",
		["column_width"] = "pixels per day in the week (default 184); the grid scrolls sideways past its width",
		["day_width"] = "pixels for the one column of the day view (default 560)",
		["gutter"] = "pixels for the hours' labels (default 64)",
		["now"] = "{ day, hour }: a line across that day at that hour",
		["more"] = "month: chips per day before \"+N more\" (default 3)",
		["event"] = "key of an event's message, default \"select\": { select = id }",
		["switch"] = "key of the switch's message, default \"view\": { view = id }",
		["pick_day"] = "key of a day's message, default \"day\": { day = id }",
		["move"] = "events drag to another day (and hour): a drop sends { [move] = day id, phase = \"drop\", carry = { event = id }, y }, `y` the fraction down the hours shown in day and week; `over` and `leave` come the same way",
		["over"] = "id of the day a dragged event is over: tint it",
		["to"] = "mod to send them to, if not the drawing one",
		["grow"] = "fill the parent (default true)",
		["height"] = "pixels instead of growing",
	},
	styles = {
		[".week-calendar"] = { background = "background", border = true, border_color = "border", radius = 14, gap = 0 },
		[".week-calendar-top"] = { padding = { 14, 18, 12, 14 }, gap = 10, border = "bottom", border_color = "border" },
		[".week-calendar-grip"] = { color = "muted-foreground" },
		[".week-calendar-title"] = { size = "large", weight = "medium" },
		[".week-calendar-nav"] = { gap = 4, color = "muted-foreground" },
		[".week-calendar-today-button"] = { padding = { 4, 8 }, radius = 6, color = "muted-foreground" },
		[".week-calendar-today-button:hover"] = { background = "list-hover", color = "foreground" },
		[".week-calendar-arrow"] = { padding = 4, radius = 6, color = "muted-foreground" },
		[".week-calendar-arrow:hover"] = { background = "list-hover", color = "foreground" },
		[".week-calendar-arrows"] = { gap = 2, padding = { 0, 0, 0, 2 } },
		[".week-calendar-heads"] = { border = "bottom", border_color = "border", gap = 0 },
		[".week-calendar-head"] = { radius = 8 },
		[".week-calendar-head:hover"] = { background = "list-hover" },
		[".week-calendar-head-text"] = { color = "foreground/75" },
		[".week-calendar-today"] = { color = "accent", weight = "medium" },
		[".week-calendar-grid"] = { gap = 0 },
		[".week-calendar-hour"] = { border = "top", border_color = "border/70" },
		[".week-calendar-hour-label"] = { padding = { 8, 0, 0, 6 } },
		[".week-calendar-hour-text"] = { size = "small", color = "muted-foreground" },
		[".week-calendar-event"] = {
			background = "secondary",
			border = true,
			border_color = "border",
			radius = 10,
			padding = { 8, 10 },
			gap = 2,
			shadow = "0 1px 2px #0000000a",
		},
		[".week-calendar-event:hover"] = { border_color = "foreground/25" },
		[".week-calendar-event:selected"] = { background = "background", border_color = "foreground/70", shadow = "0 4px 14px -4px #00000026" },
		[".week-calendar-event-head"] = { gap = 6 },
		[".week-calendar-event-title"] = { weight = "medium", size = "small" },
		[".week-calendar-event-caption"] = { size = "small", color = "muted-foreground" },
		[".week-calendar-event-time"] = { size = "small", color = "foreground/80" },
		[".week-calendar-mark"] = { radius = 5, color = "#ffffff", size = "small" },
		[".week-calendar-mark-video"] = { background = "#2d8cff", radius = 8 },
		[".week-calendar-mark-meet"] = { background = "#1fa463" },
		[".week-calendar-mark-chat"] = { background = "#5865f2" },
		[".week-calendar-mark-call"] = { background = "#f97316", radius = 8 },
		[".week-calendar-mark-plain"] = { background = "accent" },
		[".week-calendar-now"] = { background = "danger" },
		[".week-calendar-now-dot"] = { background = "danger", radius = 4 },
		[".week-calendar-month"] = { padding = { 0, 0, 0, 0 } },
		[".week-calendar-month-name"] = { padding = { 0, 6 } },
		[".week-calendar-week"] = { border = "bottom", border_color = "border/70", gap = 0 },
		[".week-calendar-cell"] = { padding = { 6, 6 }, gap = 3, border = "left", border_color = "border/70" },
		[".week-calendar-cell-first"] = { border = "none" },
		[".week-calendar-cell:hover"] = { background = "list-hover" },
		[".week-calendar-cell:over"] = { background = "accent/10" },
		[".week-calendar-drop:over"] = { background = "accent/6" },
		[".week-calendar-outside"] = { opacity = 0.4 },
		[".week-calendar-cell-number"] = { size = "small", weight = "medium", padding = { 0, 2 } },
		[".week-calendar-cell-today"] = { color = "accent" },
		[".week-calendar-chip"] = { background = "secondary", border = true, border_color = "border", radius = 6, padding = { 2, 5 }, gap = 5 },
		[".week-calendar-chip:hover"] = { border_color = "foreground/25" },
		[".week-calendar-chip:selected"] = { background = "background", border_color = "foreground/70" },
		[".week-calendar-chip-text"] = { size = "small" },
		[".week-calendar-more"] = { size = "small", color = "muted-foreground", padding = { 0, 4 } },
	},
	example = {
		title = "November 2026",
		view = "week",
		today = "today",
		previous = "previous",
		next = "next",
		height = 520,
		hours = { from = 8, to = 14 },
		hour_height = 76,
		column_width = 150,
		selected = "e2",
		days = {
			{ id = "mon", label = "Mon 2" },
			{ id = "tue", label = "Tue 3", today = true },
			{ id = "wed", label = "Wed 4" },
			{ id = "thu", label = "Thu 5" },
		},
		events = {
			{ id = "e1", day = "mon", start = 8, finish = 9, title = "Product demo", caption = "Meeting Time", time = "8:00 AM - 9:00 AM", mark = "video" },
			{ id = "e2", day = "tue", start = 9, finish = 11, title = "Roadmap review", caption = "Meeting Time", time = "9:00 AM - 11:00 AM", mark = "grid" },
			{ id = "e3", day = "tue", start = 10, finish = 11.5, title = "Pricing sync", caption = "Meeting Time", time = "10:00 AM - 11:30 AM", mark = "chat" },
			{ id = "e4", day = "thu", start = 11, finish = 12, title = "Follow-up call", caption = "Meeting Time", time = "11:00 AM - 12:00 PM", mark = "meet" },
		},
		now = { day = "tue", hour = 12.25 },
	},
	usage = [[local week_calendar = use "week-calendar"

week_calendar {
	title = "November 2026", view = state.view, today = "today", previous = "previous", next = "next",
	days = { { id = "2026-11-02", label = "Mon 2" }, { id = "2026-11-03", label = "Tue 3", today = true } },
	events = {
		{ id = "m1", day = "2026-11-03", start = 9, finish = 10.5, title = "Roadmap review",
			caption = "Meeting Time", time = "9:00 AM - 10:30 AM", mark = "video" },
	},
	selected = state.selected,
}
-- update: msg.select is an event's id, msg.view "day" | "week" | "month",
-- msg.day a day heading's id; "today", "previous" and "next" move the dates.]],

	build = function(props: Props)
		local view = props.view or "week"
		local events: { Event } = props.events or {}
		local options = {}
		if props.views ~= false then
			local wanted = if type(props.views) == "table" then props.views else nil
			for _, option in VIEWS do
				if not wanted or table.find(wanted, option.id) then
					table.insert(options, option)
				end
			end
		end
		local show_header = props.header ~= false and (props.title ~= nil or props.previous ~= nil or #options > 0)
		local header = if not show_header
			then nil
			else ui.row {
				class = "week-calendar-top",
				align = "center",
				shrink = false,
				ui.icon { name = "grip-vertical", size = "small", class = "week-calendar-grip" },
				props.title ~= nil and ui.text { props.title, class = "week-calendar-title" },
				ui.row {
					class = "week-calendar-nav",
					align = "center",
					props.previous ~= nil and ui.row { class = "week-calendar-arrow", msg = props.previous, to = props.to, ui.icon { name = "chevron-left", size = "small" } },
					props.today ~= nil and ui.row { class = "week-calendar-today-button", msg = props.today, to = props.to, ui.text "Today" },
					props.next ~= nil and ui.row { class = "week-calendar-arrow", msg = props.next, to = props.to, ui.icon { name = "chevron-right", size = "small" } },
				},
				ui.spacer(),
				#options > 0 and segmented { options = options, selected = view, event = props.switch or "view", to = props.to },
			}
		local body
		if view == "month" then
			body = month_view(props, events)
		else
			body = grid_view(props, events)
		end
		return ui.column {
			class = "week-calendar",
			grow = if props.height then nil else props.grow ~= false,
			height = props.height,
			header,
			body,
		}
	end,
}
