-- Meetings: a header with Create, a toolbar (filters, whose meetings, the
-- layout, search, import, schedule), then the calendar (day, week or month)
-- beside a panel with the next meeting and the one picked. Clicking an
-- event shows it in the panel; Reschedule moves it a day on, Join copies
-- its link. The meetings come from `providers/meetings` (`app.meetings`),
-- which also says what today is: views have no clock.

local week_calendar = use "week-calendar"
local dropdown = use "dropdown"
local chips = use "chips"
local avatar = use "avatar"
local icon_button = use "icon-button"
local kbd = use "kbd"
local table_view = use "table"

type State = {
	view: string,
	layout: string,
	anchor: string?,
	selected: string?,
	query: string,
	scope: string,
	scope_open: boolean,
	filters: boolean,
	off: { string },
	focus: boolean,
	copied: string?,
	today: string?,
}

local MONTHS: { string } = {
	"January", "February", "March", "April", "May", "June",
	"July", "August", "September", "October", "November", "December",
}
local DAYS: { string } = { "Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat" }
local LONG_DAYS: { string } = { "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday" }

local KINDS: { { id: string, label: string } } = {
	{ id = "video", label = "Video" },
	{ id = "meet", label = "Meet" },
	{ id = "grid", label = "Teams" },
	{ id = "chat", label = "Chat" },
	{ id = "call", label = "Call" },
}

local SCOPES: { { id: string, label: string } } = {
	{ id = "all", label = "All Meetings" },
	{ id = "mine", label = "Hosted by me" },
	{ id = "invited", label = "Invited to" },
}

-- Dates as day numbers since 1970-01-01 (Howard Hinnant's civil algorithms).
local function day_number(y: number, m: number, d: number): number
	y -= if m <= 2 then 1 else 0
	local era = (if y >= 0 then y else y - 399) // 400
	local yoe = y - era * 400
	local mp = (m + 9) % 12
	local doy = (153 * mp + 2) // 5 + d - 1
	local doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
	return era * 146097 + doe - 719468
end

local function civil(n: number): (number, number, number)
	n += 719468
	local era = (if n >= 0 then n else n - 146096) // 146097
	local doe = n - era * 146097
	local yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
	local doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
	local mp = (5 * doy + 2) // 153
	local d = doy - (153 * mp + 2) // 5 + 1
	local m = if mp < 10 then mp + 3 else mp - 9
	return yoe + era * 400 + (if m <= 2 then 1 else 0), m, d
end

local function parse(id: string): number
	local y, m, d = string.match(id, "^(%d+)-(%d+)-(%d+)$")
	return day_number(tonumber(y) or 1970, tonumber(m) or 1, tonumber(d) or 1)
end

local function id_of(n: number): string
	local y, m, d = civil(n)
	return string.format("%04d-%02d-%02d", y, m, d)
end

local function weekday(n: number): number
	return (n + 4) % 7 -- 0 is Sunday
end

local function clock(hour: number): string
	local h = math.floor(hour)
	local minutes = math.floor((hour - h) * 60 + 0.5)
	local twelve = h % 12
	if twelve == 0 then
		twelve = 12
	end
	return string.format("%d:%02d %s", twelve, minutes, if h % 24 < 12 then "AM" else "PM")
end

local function range(meeting: any): string
	return `{clock(meeting.start)} - {clock(meeting.finish)}`
end

local function when(meeting: any, today: number): string
	local n = parse(meeting.date)
	local _, m, d = civil(n)
	local day = if n == today then "Today" elseif n == today + 1 then "Tomorrow" else `{DAYS[weekday(n) + 1]}, {string.sub(MONTHS[m], 1, 3)} {d}`
	return `{day}, {clock(meeting.start)}`
end

-- A meeting's mark, drawn larger for the panel: a tinted tile with the
-- provider's sign, as the calendar's cards draw it smaller.
local function big_mark(mark: string): any
	local tint = if mark == "meet" then "#1fa463" elseif mark == "chat" then "#5865f2" elseif mark == "call" then "#f97316" elseif mark == "grid" then "#00a4ef" else "#2d8cff"
	local sign
	if mark == "grid" then
		local function square(color: string): any
			return ui.row { width = 8, height = 8, style = { background = color, radius = 2 } }
		end
		sign = ui.column {
			style = { gap = 2 },
			ui.row { style = { gap = 2 }, square("#f25022"), square("#7fba00") },
			ui.row { style = { gap = 2 }, square("#00a4ef"), square("#ffb900") },
		}
	elseif mark == "chat" or mark == "call" then
		sign = ui.icon { name = if mark == "chat" then "message-square" else "mic" }
	else
		sign = ui.row {
			gap = "none",
			align = "center",
			ui.row { width = 13, height = 11, style = { background = tint, radius = 3 } },
			ui.row { width = 5, height = 7, style = { background = tint, radius = { 0, 2, 2, 0 } } },
		}
	end
	return ui.row {
		class = "mt-big-mark",
		width = 38,
		height = 38,
		shrink = false,
		align = "center",
		justify = "center",
		style = { background = tint .. "1f", color = tint },
		sign,
	}
end

return mod {
	region = "meetings",
	title = "Meetings",
	subscribe = { "provider/meetings" },

	styles = {
		[".mt"] = { padding = { 16, 22, 18, 22 }, gap = 14 },
		[".mt-head"] = { padding = { 0, 0, 14, 0 }, gap = 10, border = "bottom", border_color = "border" },
		[".mt-title"] = { size = "large", weight = "medium" },
		[".mt-sub"] = { color = "muted-foreground" },
		[".mt-new"] = { padding = { 7, 14 }, gap = 6 },
		[".mt-tool"] = { gap = 8 },
		[".mt-button"] = { border = true, border_color = "border", radius = 9, padding = { 0, 12 }, gap = 8, background = "background" },
		[".mt-button:hover"] = { background = "list-hover" },
		[".mt-button:selected"] = { background = "secondary", border_color = "foreground/30" },
		[".mt-layouts"] = { border = true, border_color = "border", radius = 9, padding = 3, gap = 2, background = "background" },
		[".mt-layout"] = { radius = 6, color = "muted-foreground" },
		[".mt-layout:hover"] = { color = "foreground" },
		[".mt-layout:selected"] = { background = "background", color = "foreground", border = true, border_color = "border", shadow = "0 1px 2px #0000001a" },
		[".mt-search"] = { border = true, border_color = "border", radius = 9, padding = { 0, 6, 0, 12 }, gap = 8, background = "background" },
		[".mt-search-icon"] = { color = "muted-foreground" },
		[".mt-field"] = { background = "#00000000", border = "none", shadow = "none", padding = { 5, 0 } },
		[".mt-panel"] = { background = "frame", radius = 16, padding = 6, gap = 6 },
		[".mt-card"] = { background = "background", radius = 12, border = true, border_color = "border", padding = { 14, 16 }, gap = 14 },
		[".mt-card:hover"] = { border_color = "foreground/20" },
		[".mt-card-title"] = { size = "large", weight = "medium" },
		[".mt-grip"] = { color = "muted-foreground" },
		[".mt-label"] = { weight = "medium", size = "small" },
		[".mt-faint"] = { size = "small", color = "muted-foreground" },
		[".mt-text"] = { color = "foreground/75" },
		[".mt-field-group"] = { gap = 6 },
		[".mt-big-mark"] = { radius = 10 },
		[".mt-zone"] = { border = true, border_color = "border", radius = 5, padding = { 0, 5 }, size = "small" },
		[".mt-link"] = { radius = 6, gap = 6 },
		[".mt-link:hover"] = { background = "list-hover" },
		[".mt-actions"] = { padding = { 2, 2, 2, 2 }, gap = 6 },
		[".mt-outline"] = { border = true, border_color = "border", radius = 9, background = "background", padding = { 9, 0 } },
		[".mt-outline:hover"] = { background = "list-hover" },
		[".mt-join"] = { padding = { 9, 0 }, gap = 6, radius = 9 },
		[".mt-empty"] = { color = "muted-foreground", padding = 16 },
	},

	init = function(previous: State?): State
		local state: State = previous
			or {
				view = "week",
				layout = "calendar",
				anchor = nil,
				selected = nil,
				query = "",
				scope = "all",
				scope_open = false,
				filters = false,
				off = {},
				focus = false,
				copied = nil,
				today = nil,
			}
		state.scope_open = false
		state.focus = false
		state.copied = nil
		return state
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.topic == "provider/meetings" then
			state.today = msg.value and msg.value.today
			return
		end
		local today = if state.today then parse(state.today) else nil
		if msg == "focus" then
			state.focus = true
			return
		elseif msg == "toggle-scope" then
			state.scope_open = not state.scope_open
			return
		elseif msg == "close-scope" then
			state.scope_open = false
			return
		elseif msg == "filters" then
			state.filters = not state.filters
			return
		elseif msg == "today" then
			state.anchor = nil
			return
		elseif msg == "previous" or msg == "next" then
			local base = if state.anchor then parse(state.anchor) else today
			if not base then
				return
			end
			local sign = if msg == "next" then 1 else -1
			if state.view == "month" then
				local y, m = civil(base)
				m += sign
				if m < 1 then
					y, m = y - 1, 12
				elseif m > 12 then
					y, m = y + 1, 1
				end
				state.anchor = id_of(day_number(y, m, 1))
			else
				state.anchor = id_of(base + sign * (if state.view == "day" then 1 else 7))
			end
			return
		elseif msg == "reschedule" or msg == "join" or msg == "schedule" then
			local id = state.selected
			if msg == "schedule" then
				-- An hour-long meeting on the day in view, at 9 AM.
				local day = state.anchor or state.today
				return state, day and { { to = "provider:meetings", msg = { schedule = { date = day, start = 9 } } } } or nil
			elseif msg == "reschedule" and id then
				return state, { { to = "provider:meetings", msg = { reschedule = id } } }
			elseif msg == "join" then
				state.copied = id
			end
			return
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.selected = if state.selected == msg.select then nil else msg.select
		elseif msg.view then
			state.view = msg.view
		elseif msg.day then
			state.anchor = msg.day
			state.view = "day"
		elseif msg.layout then
			state.layout = msg.layout
		elseif msg.pick then
			state.scope = msg.pick
			state.scope_open = false
		elseif msg.toggle then
			local at = table.find(state.off, msg.toggle)
			if at then
				table.remove(state.off, at)
			else
				table.insert(state.off, msg.toggle)
			end
		elseif msg.msg == "query" then
			state.query = msg.value
			state.focus = false
		elseif msg.msg == "clear" then
			state.query = ""
		elseif msg.copied then
			state.copied = msg.copied
		end
	end,

	view = function(state: State, app)
		local data = app.meetings
		if not data then
			return ui.column { class = "mt", grow = true, ui.text { "Opening the calendar…", tone = "muted" } }
		end
		local today = parse(data.today)
		local now = data.now
		local me = data.me

		-- What the filters let through.
		local query = string.lower(state.query)
		local function shown(meeting: any): boolean
			if table.find(state.off, meeting.mark) then
				return false
			elseif state.scope == "mine" and meeting.host ~= me then
				return false
			elseif state.scope == "invited" and meeting.host == me then
				return false
			end
			return query == ""
				or string.find(string.lower(meeting.title), query, 1, true) ~= nil
				or string.find(string.lower(meeting.related), query, 1, true) ~= nil
		end
		local by_id = {}
		local next_meeting = nil
		for _, meeting in data.meetings do
			by_id[meeting.id] = meeting
			if not next_meeting and (meeting.date > now.date or (meeting.date == now.date and meeting.finish > now.hour)) then
				next_meeting = meeting
			end
		end
		local picked = (state.selected and by_id[state.selected]) or next_meeting

		-- The dates in view.
		local anchor = if state.anchor then parse(state.anchor) else today
		local ay, am, ad = civil(anchor)
		local days = {}
		local weeks = {}
		local first, last
		local title
		if state.view == "month" then
			local start = day_number(ay, am, 1)
			first = start - weekday(start)
			local week = {}
			local n = first
			while true do
				local _, m, d = civil(n)
				table.insert(week, { id = id_of(n), day = tostring(d), today = n == today, outside = m ~= am })
				if #week == 7 then
					table.insert(weeks, week)
					week = {}
					local _, next_month = civil(n + 1)
					if next_month ~= am then
						break
					end
				end
				n += 1
			end
			last = n
			title = `{MONTHS[am]} {ay}`
		else
			first = if state.view == "day" then anchor else anchor - weekday(anchor)
			last = if state.view == "day" then anchor else first + 6
			for n = first, last do
				local _, _, d = civil(n)
				local label = if state.view == "day" then `{LONG_DAYS[weekday(n) + 1]} {d}` else `{DAYS[weekday(n) + 1]} {d}`
				table.insert(days, { id = id_of(n), label = label, today = n == today })
			end
			local fy, fm = civil(first)
			local ly, lm = civil(last)
			if state.view == "day" then
				title = `{MONTHS[am]} {ad}, {ay}`
			elseif fm == lm then
				title = `{MONTHS[fm]} {fy}`
			elseif fy == ly then
				title = `{string.sub(MONTHS[fm], 1, 3)} – {string.sub(MONTHS[lm], 1, 3)} {ly}`
			else
				title = `{string.sub(MONTHS[fm], 1, 3)} {fy} – {string.sub(MONTHS[lm], 1, 3)} {ly}`
			end
		end
		local first_id, last_id = id_of(first), id_of(last)

		local events = {}
		local rows = {}
		local from, to = 8, 18
		for _, meeting in data.meetings do
			if shown(meeting) then
				if meeting.date >= first_id and meeting.date <= last_id then
					table.insert(events, {
						id = meeting.id,
						day = meeting.date,
						start = meeting.start,
						finish = meeting.finish,
						title = meeting.title,
						caption = "Meeting Time",
						time = range(meeting),
						mark = meeting.mark,
						tooltip = `{meeting.related} · {range(meeting)}`,
					})
					from = math.min(from, math.floor(meeting.start))
					to = math.max(to, math.ceil(meeting.finish))
				end
				local n = parse(meeting.date)
				local _, m, d = civil(n)
				table.insert(rows, {
					id = meeting.id,
					title = meeting.title,
					related = meeting.related,
					date = `{DAYS[weekday(n) + 1]}, {string.sub(MONTHS[m], 1, 3)} {d}`,
					time = range(meeting),
					host = if meeting.host == me then `{meeting.host} (You)` else meeting.host,
				})
			end
		end

		local body
		if state.layout == "list" then
			body = ui.column {
				grow = true,
				class = "week-calendar",
				table_view {
					grow = true,
					columns = {
						{ key = "title", title = "Meeting" },
						{ key = "related", title = "Related to", width = 150, muted = true },
						{ key = "date", title = "Date", width = 110 },
						{ key = "time", title = "Time", width = 160, muted = true },
						{ key = "host", title = "Host", width = 140 },
					},
					rows = rows,
					selected = picked and picked.id,
					sortable = false,
					empty = "No meetings match",
				},
			}
		else
			body = week_calendar {
				view = state.view,
				title = title,
				today = "today",
				previous = "previous",
				next = "next",
				days = days,
				weeks = weeks,
				events = events,
				selected = picked and picked.id,
				hours = { from = from, to = to },
				hour_height = 96,
				column_width = 152,
				day_width = 152 * 7,
				gutter = 62,
				now = { day = now.date, hour = now.hour },
			}
		end

		-- The panel: the next meeting, then the one picked.
		local function field(label: string, ...)
			return ui.column { class = "mt-field-group", ui.text { label, class = "mt-label" }, ... }
		end
		local panel
		if picked then
			local people = {}
			for _, name in picked.participants do
				table.insert(people, ui.row {
					align = "center",
					gap = "normal",
					avatar { name = name, shape = "circle", size = "small" },
					ui.text { name, class = "mt-text" },
				})
			end
			local upcoming = next_meeting
			panel = ui.column {
				class = "mt-panel",
				width = 330,
				shrink = false,
				ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					ui.column {
						shrink = false,
						style = { gap = 6 },
						upcoming and ui.column {
							class = "mt-card",
							msg = { select = upcoming.id },
							ui.row {
								align = "center",
								gap = "small",
								ui.icon { name = "grip-vertical", size = "small", class = "mt-grip" },
								ui.text { "Next Upcoming Meeting", class = "mt-card-title" },
								ui.spacer(),
								icon_button { icon = "pencil", tooltip = "Edit (not in this example)", size = "small", msg = { select = upcoming.id } },
							},
							ui.row {
								align = "center",
								gap = "normal",
								big_mark(upcoming.mark),
								ui.column {
									gap = "none",
									grow = true,
									ui.text { upcoming.title, weight = "medium", truncate = true },
									ui.text { when(upcoming, today), class = "mt-sub" },
								},
							},
							field("Meeting Agenda", ui.text { upcoming.agenda, class = "mt-text", wrap = true }),
							field(
								"Meeting Link",
								ui.row {
									class = "mt-link",
									align = "center",
									copy = upcoming.link,
									msg = { copied = upcoming.id },
									tooltip = "Copy the link",
									ui.icon { name = "link", size = "small", tone = "muted" },
									ui.text { upcoming.link, class = "mt-text", truncate = true },
								}
							),
							field("Meeting Duration", ui.text { range(upcoming), class = "mt-text" }),
						},
						ui.column {
							class = "mt-card",
							ui.row {
								align = "center",
								gap = "small",
								ui.icon { name = "grip-vertical", size = "small", class = "mt-grip" },
								ui.text { "Meeting information", class = "mt-card-title" },
							},
							picked ~= upcoming and ui.row {
								align = "center",
								gap = "normal",
								big_mark(picked.mark),
								ui.column {
									gap = "none",
									grow = true,
									ui.text { picked.title, weight = "medium", truncate = true },
									ui.text { when(picked, today), class = "mt-sub" },
								},
							},
							field(
								"Meeting Host",
								ui.row {
									align = "center",
									gap = "normal",
									avatar { name = picked.host, shape = "circle", size = "small" },
									ui.text { picked.host, class = "mt-text" },
									picked.host == me and ui.text { "(You)", class = "mt-faint" },
								}
							),
							field(
								"Related To",
								ui.row {
									align = "center",
									gap = "normal",
									ui.icon { name = "building-2", size = "small", tone = "muted" },
									ui.text { picked.related, class = "mt-text" },
								}
							),
							field(
								"Created Date",
								ui.row {
									align = "center",
									gap = "small",
									ui.text { picked.created.time, class = "mt-text" },
									ui.text { picked.created.zone, class = "mt-zone" },
								},
								ui.text { picked.created.date, class = "mt-text" }
							),
							#people > 0 and field("Participants", ui.column { gap = "normal", table.unpack(people) }),
							picked.notes ~= "" and field("Notes", ui.text { picked.notes, class = "mt-text", wrap = true }),
						},
					},
				},
				ui.row {
					class = "mt-actions",
					shrink = false,
					ui.row {
						class = "mt-outline",
						grow = true,
						justify = "center",
						msg = "reschedule",
						tooltip = "Move it to the next day, same time",
						ui.text { "Reschedule", weight = "medium" },
					},
					ui.row {
						class = { "dark-button", "mt-join" },
						grow = true,
						justify = "center",
						align = "center",
						msg = "join",
						copy = picked.link,
						tooltip = "Copy the meeting link",
						ui.icon { name = if state.copied == picked.id then "check" else "external-link", size = "small" },
						ui.text { if state.copied == picked.id then "Link copied" else "Join Meeting", weight = "medium" },
					},
				},
			}
		else
			panel = ui.column {
				class = "mt-panel",
				width = 330,
				shrink = false,
				ui.column { class = { "mt-card" }, ui.text { "No meetings coming up.", class = "mt-empty" } },
			}
		end

		local kind_chips = {}
		for _, kind in KINDS do
			local count = 0
			for _, meeting in data.meetings do
				if meeting.mark == kind.id then
					count += 1
				end
			end
			table.insert(kind_chips, { id = kind.id, label = kind.label, count = count, on = table.find(state.off, kind.id) == nil })
		end

		local scope_label = "All Meetings"
		for _, scope in SCOPES do
			if scope.id == state.scope then
				scope_label = scope.label
			end
		end

		return ui.column {
			class = "mt",
			grow = true,
			ui.row {
				class = "mt-head",
				align = "center",
				shrink = false,
				ui.icon { name = "calendar" },
				ui.text { "Meetings", class = "mt-title" },
				ui.text { `{#events} in view · {#data.meetings} in all`, class = "mt-sub" },
				ui.spacer(),
				ui.row {
					class = { "dark-button", "mt-new" },
					align = "center",
					msg = "schedule",
					tooltip = "A new meeting on the day in view, at 9 AM",
					ui.icon { name = "plus", size = "small" },
					ui.text { "Create", weight = "medium" },
				},
			},
			ui.row {
				class = "mt-tool",
				align = "center",
				height = 38,
				shrink = false,
				ui.row {
					class = "mt-button",
					height = 38,
					align = "center",
					selected = state.filters,
					msg = "filters",
					ui.icon { name = "filter", size = "small" },
					ui.text { if state.filters then "Hide Filters" else "Show Filters" },
				},
				dropdown {
					icon = "workflow",
					value = scope_label,
					selected = state.scope,
					options = SCOPES,
					open = state.scope_open,
					toggle = "toggle-scope",
					dismiss = "close-scope",
					width = 200,
				},
				ui.row {
					class = "mt-layouts",
					height = 38,
					align = "center",
					ui.row {
						class = "mt-layout",
						width = 30,
						height = 30,
						align = "center",
						justify = "center",
						selected = state.layout == "list",
						msg = { layout = "list" },
						tooltip = "List",
						ui.icon { name = "rows-2", size = "small" },
					},
					ui.row {
						class = "mt-layout",
						width = 30,
						height = 30,
						align = "center",
						justify = "center",
						selected = state.layout == "calendar",
						msg = { layout = "calendar" },
						tooltip = "Calendar",
						ui.icon { name = "calendar", size = "small" },
					},
				},
				ui.row {
					class = "mt-search",
					grow = true,
					height = 38,
					align = "center",
					ui.icon { name = "search", size = "small", class = "mt-search-icon" },
					ui.input {
						id = "search",
						value = state.query,
						placeholder = "Search",
						change = "query",
						keys = { escape = "clear" },
						focus = state.focus,
						grow = true,
						class = "mt-field",
					},
					kbd { keys = { "⌘K" }, small = true },
				},
				ui.row {
					class = "mt-button",
					height = 38,
					align = "center",
					tooltip = "Import a calendar (not in this example)",
					ui.icon { name = "upload", size = "small" },
					ui.text "Import",
				},
				ui.row {
					class = "mt-button",
					height = 38,
					align = "center",
					msg = "schedule",
					tooltip = "A new meeting on the day in view, at 9 AM",
					ui.icon { name = "plus", size = "small" },
					ui.text "Schedule",
				},
			},
			state.filters and chips { chips = kind_chips },
			ui.row {
				grow = true,
				align = "stretch",
				style = { gap = 14 },
				body,
				panel,
			},
		}
	end,
}
