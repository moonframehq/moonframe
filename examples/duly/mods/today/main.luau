-- Today: a greeting with Export, five headline figures, the exceptions
-- forecast (pick a day to read its figures) with an AI insight, what is due
-- this week (searchable), open exceptions, and the deadlines table
-- (sortable, each row opens its page; a calendar of due dates). All from
-- `app.duly` (sample data).

local kpi_meter = use "kpi-meter"
local line_chart = use "line-chart"
local insight_note = use "insight-note"
local menu = use "menu"
local month_grid = use "month-grid"

type State = {
	day: number?,
	query: string,
	sort: string?,
	descending: boolean,
	selected: string?,
	export: boolean,
	calendar: boolean,
	refreshing: boolean,
	calendar_day: string?,
}

-- Where each headline figure's ↗ goes.
local KPI_PAGES = { "notices", "public", "contracts", "payments", "public" }
local TONE_ORDER = { red = 1, amber = 2, grey = 3 }

-- October 2026 for the calendar: Monday first, Thu 1 Oct, today Wed 28.
local function october(due: { [string]: number }): { any }
	local weeks = {}
	local day = -2 -- Mon 28 Sep
	for _ = 1, 5 do
		local week = {}
		for _ = 1, 7 do
			local id, label, outside
			if day < 1 then
				id, label, outside = `09-{30 + day}`, tostring(30 + day), true
			elseif day > 31 then
				id, label, outside = `11-{day - 31}`, tostring(day - 31), true
			else
				id, label, outside = `10-{day}`, tostring(day), false
			end
			table.insert(week, { id = id, day = label, today = id == "10-28", outside = outside, count = due[id] or 0 })
			day += 1
		end
		table.insert(weeks, week)
	end
	return weeks
end

-- A person's initials in a small grey circle (dark for you), as the
-- design draws owners.
local function initials(text: string, me: boolean?)
	return ui.row {
		class = if me then { "initials", "initials-me" } else "initials",
		width = 20,
		height = 20,
		align = "center",
		justify = "center",
		shrink = false,
		ui.text { text, class = if me then "initials-text-me" else "initials-text" },
	}
end

-- "Thu 29 Oct" or "Was 30 Sep" as a calendar id, "10-29".
local function due_id(text: string): string?
	local day, month = string.match(text, "(%d+) (%a+)$")
	if not day then
		return nil
	end
	local months = { Sep = "09", Oct = "10", Nov = "11" }
	return `{months[month] or "10"}-{day}`
end

return mod {
	title = "Today",
	subscribe = { "provider/duly" },

	styles = {
		[".d-page"] = { padding = { 34, 30, 30, 30 }, gap = 12 },
		[".greeting"] = { size = "display", weight = "medium" },
		[".outline-button"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 7, 12 }, gap = 6, shadow = "0 1px 1px #00000008" },
		[".outline-button:hover"] = { background = "secondary" },
		[".strip"] = { background = "secondary", border = true, border_color = "border", radius = 12, gap = 0 },
		[".strip-rule"] = { background = "border" },
		[".card"] = { background = "secondary", border = true, border_color = "border", radius = 12, padding = { 14, 18, 18, 18 }, gap = 12 },
		[".card-title"] = { size = "large", weight = "medium" },
		[".hairline"] = { background = "border" },
		[".soft-rule"] = { background = "hairline" },
		[".ghost-small"] = { radius = 6, padding = { 4, 8 }, gap = 6 },
		[".ghost-small:hover"] = { background = "list-hover" },
		[".d-muted"] = { color = "muted-foreground" },
		[".muted-small"] = { color = "muted-foreground", size = "small" },
		[".mono"] = { font = "mono" },
		[".mono-small"] = { font = "mono", size = "small" },
		[".mono-muted"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".tip-mark"] = { radius = 1 },
		[".tip-red"] = { color = "danger", weight = "medium" },
		[".tip-badge"] = { background = "danger", color = "#ffffff", radius = 7 },
		[".search-box"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 2, 10 }, gap = 6 },
		[".search-input"] = { background = "#00000000", border = "none", padding = { 4, 0 } },
		[".list"] = { background = "background", border = true, border_color = "border", radius = 10, gap = 0 },
		[".list-row"] = { padding = { 13, 14 }, gap = 10, border = "bottom", border_color = "hairline" },
		[".list-row:hover"] = { background = "list-hover" },
		[".list-row-last"] = { border = "none" },
		[".d-notice-icon"] = { color = "blue" },
		[".initials"] = { background = "secondary", border = true, border_color = "border", radius = 10 },
		[".initials-me"] = { background = "foreground", border_color = "foreground" },
		[".initials-text"] = { size = "small", color = "muted-foreground", weight = "medium" },
		[".initials-text-me"] = { size = "small", color = "background", weight = "medium" },
		[".pair"] = { background = "secondary", border = true, border_color = "border", radius = 12, padding = { 0, 5, 5, 5 }, gap = 0 },
		[".pair-head"] = { padding = { 9, 10, 8, 12 }, gap = 8 },
		[".pair-title"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".pair-body"] = { background = "background", border = true, border_color = "border", radius = 9, gap = 0 },
		[".pair-left"] = { padding = { 16, 18 }, gap = 10, border = "right", border_color = "border" },
		[".pair-right"] = { padding = { 16, 18 }, gap = 6 },
		[".big-figure"] = { size = "display", font = "mono" },
		[".chip-good"] = { background = "success/14", color = "success", radius = 5, padding = { 1, 7 }, size = "small" },
		[".legend-square"] = { radius = 2 },
		[".bar-part"] = { radius = 4 },
		[".day-track"] = { background = "track", radius = 3 },
		[".day-fill"] = { background = "blue", radius = 3 },
		[".day-fill-today"] = { background = "ink", radius = 3 },
		[".day-fill-togo"] = { background = "danger/15", radius = 3 },
		[".day-togo"] = { color = "danger", font = "mono", size = "small" },
		[".table-head"] = { padding = { 6, 4 }, border = "bottom", border_color = "hairline" },
		[".d-table-heading"] = { size = "small", color = "muted-foreground" },
		[".d-table-heading:hover"] = { color = "foreground" },
		[".d-table-row"] = { padding = { 11, 4 }, border = "bottom", border_color = "hairline", radius = 4 },
		[".d-table-row:hover"] = { background = "list-hover" },
		[".d-table-row:selected"] = { background = "accent/8" },
		[".table-row-last"] = { border = "none" },
		[".d-status-dot"] = { radius = 3 },
		[".open-link"] = { weight = "medium", size = "small", radius = 5, padding = { 2, 6 } },
		[".open-link:hover"] = { background = "list-hover" },
		[".calendar-pop"] = { background = "background", border = true, border_color = "border", radius = 12, padding = 12, gap = 10, shadow = "0 12px 32px -8px #00000033" },
	},

	init = function(previous: State?): State
		local state: State = previous or { query = "", descending = false, export = false, calendar = false, refreshing = false }
		state.query = ""
		state.export = false
		state.calendar = false
		state.refreshing = false
		return state
	end,

	update = function(state: State, msg)
		if type(msg) == "string" then
			if msg == "refresh" then
				state.refreshing = true
				return state, { { to = "provider:duly", msg = "refresh" } }
			elseif msg == "export" then
				state.export = not state.export
			elseif msg == "close-export" then
				state.export = false
			elseif msg == "calendar" then
				state.calendar = not state.calendar
			elseif msg == "close-calendar" then
				state.calendar = false
			end
			return
		end
		if msg.topic == "provider/duly" then
			state.refreshing = false
		elseif msg.point then
			state.day = msg.point
		elseif msg.msg == "query" then
			state.query = msg.value
		elseif msg.msg == "clear" then
			state.query = ""
		elseif msg.sort then
			if state.sort == msg.sort then
				if state.descending then
					state.sort, state.descending = nil, false
				else
					state.descending = true
				end
			else
				state.sort, state.descending = msg.sort, false
			end
		elseif msg.row then
			state.selected = if state.selected == msg.row then nil else msg.row
		elseif msg.day then
			state.calendar_day = if state.calendar_day == msg.day then nil else msg.day
		elseif msg.pick then
			state.export = false
		elseif msg.go then
			return state, { { to = "sidebar", msg = { page = msg.go } } }
		end
	end,

	view = function(state: State, app)
		local data = app.duly
		if not data then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { "Loading…", class = "d-muted" } }
		end
		if data.available == false then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { data.reason or "No data", class = "d-muted" } }
		end
		local today = data.today
		local person = data.person

		-- The greeting and Export.
		local head = ui.row {
			align = "center",
			drag = true,
			ui.text { `Welcome back, {person.name}!`, class = "greeting" },
			ui.spacer(),
			ui.popover {
				open = state.export,
				dismiss = "close-export",
				side = "bottom",
				align = "end",
				width = 220,
				content = menu {
					items = ({
						{ id = "pdf", label = "Today as PDF", icon = "file-text" },
						{ id = "csv", label = "Deadlines as CSV", icon = "download" },
						"separator",
						{ id = "pack", label = "Audit pack…", icon = "layers" },
					} :: { any }),
				},
				ui.row { class = "outline-button", align = "center", msg = "export", ui.icon { name = "file", size = "small" }, ui.text { "Export", weight = "medium" } },
			},
		}

		-- Five headline figures.
		local cells = {}
		for index, kpi in today.kpis do
			if index > 1 then
				table.insert(cells, ui.row { class = "strip-rule", width = 1 })
			end
			table.insert(cells, kpi_meter {
				title = kpi.title,
				dot = kpi.dot,
				value = kpi.value,
				caption = kpi.caption,
				segments = kpi.bar,
				legend = kpi.legend,
				width = 186,
				msg = { go = KPI_PAGES[index] },
			})
		end
		local strip = ui.row { class = "strip", align = "stretch", table.unpack(cells) }

		-- The forecast.
		local forecast = today.forecast
		local day = state.day or forecast.selected
		local actual, predicted = forecast.actual[day], forecast.predicted[day]
		local deviation = math.floor((actual - predicted) / math.max(predicted, 1) * 100 + 0.5)
		local tips = {}
		for index, label in forecast.days do
			table.insert(tips, `{label}: {forecast.actual[index]} actual, {forecast.predicted[index]} predicted`)
		end
		local function tip_row(mark: string?, label: string, value: any)
			return ui.row {
				align = "center",
				gap = "small",
				mark and ui.row { class = "tip-mark", width = 2, height = 12, style = { background = mark } },
				ui.text { label, class = "muted-small" },
				ui.spacer(),
				value,
			}
		end
		local tip = ui.column {
			gap = "small",
			ui.text { forecast.days[day], size = "small", weight = "medium" },
			ui.row { class = "soft-rule", height = 1 },
			tip_row("ink", "Actual exceptions", ui.text { tostring(actual), size = "small", weight = "medium" }),
			tip_row("green", "Predicted", ui.text { tostring(predicted), size = "small", weight = "medium" }),
			tip_row(nil, "Deviation", ui.row {
				gap = "small",
				align = "center",
				ui.text { `{if deviation >= 0 then "+" else ""}{deviation}%`, size = "small", class = if deviation > 0 then "tip-red" else nil },
				deviation > 0 and ui.row { class = "tip-badge", width = 14, height = 14, align = "center", justify = "center", ui.icon { name = "arrow-up", size = "small" } },
			}),
			tip_row(nil, "AI confidence", ui.row {
				gap = "small",
				align = "center",
				ui.icon { name = "chart-column", size = "small", tone = "success" },
				ui.text { forecast.confidence[day], size = "small", weight = "medium" },
			}),
		}
		local chart_card = ui.column {
			class = "card",
			grow = true,
			ui.row {
				align = "center",
				gap = "normal",
				ui.icon { name = "sparkles" },
				ui.text { forecast.title, class = "card-title" },
				ui.spacer(),
				ui.row {
					class = "ghost-small",
					align = "center",
					msg = if state.refreshing then nil else "refresh",
					ui.icon { name = "refresh-cw", size = "small", spin = state.refreshing },
					ui.text { if state.refreshing then "Refreshing" else "Refresh", size = "small", weight = "medium" },
				},
			},
			ui.row { class = "hairline", height = 1 },
			line_chart {
				labels = forecast.labels,
				series = ({
					{ name = "Predicted", points = forecast.predicted, color = "green", dashed = true },
					{ name = "Actual", points = forecast.actual, color = "ink" },
				} :: { any }),
				bars = forecast.bars,
				max = forecast.max,
				ticks = forecast.ticks,
				width = 618,
				height = 172,
				selected = day,
				card = tip,
				card_width = 168,
				tips = tips,
			},
			insight_note {
				text = forecast.insight.text,
				link = forecast.insight.link,
				after = forecast.insight.after,
				msg = { go = "payments" },
			},
		}

		-- Due this week, searchable.
		local query = string.lower(state.query)
		local due_rows = {}
		for _, item in today.due do
			if query == "" or string.find(string.lower(item.title .. " " .. item.owner), query, 1, true) then
				table.insert(due_rows, item)
			end
		end
		local due_list = {}
		for index, item in due_rows do
			table.insert(due_list, ui.row {
				class = if index == #due_rows then { "list-row", "list-row-last" } else "list-row",
				align = "center",
				msg = { go = "payments" },
				ui.row {
					grow = true,
					gap = "normal",
					align = "center",
					ui.icon { name = "file-text", size = "small", class = "d-notice-icon" },
					ui.text { item.title, truncate = true },
				},
				ui.row {
					width = 150,
					shrink = false,
					gap = "normal",
					align = "center",
					initials(item.initials, item.me),
					ui.text { item.owner, truncate = true },
				},
			})
		end
		local due_card = ui.column {
			class = "card",
			width = 420,
			shrink = false,
			ui.row {
				align = "center",
				ui.text { "Due this week", class = "card-title" },
				ui.spacer(),
				ui.row {
					class = "search-box",
					width = 128,
					align = "center",
					ui.icon { name = "search", size = "small", class = "d-muted" },
					ui.input { id = "due-search", class = "search-input", value = state.query, placeholder = "Search", change = "query", keys = { escape = "clear" }, grow = true },
				},
			},
			ui.row {
				padding = "none",
				style = { padding = { 0, 14 } },
				ui.row { grow = true, ui.text { "Notice", class = "d-muted" } },
				ui.row { width = 150, ui.text { "Owner", class = "d-muted" } },
			},
			if #due_list == 0
				then ui.column { class = "list", style = { padding = 16 }, ui.text { `Nothing due matches “{state.query}”`, class = "d-muted" } }
				else ui.column { class = "list", scroll = true, height = 318, ui.column { gap = "none", shrink = false, table.unpack(due_list) } },
		}

		-- Open exceptions.
		local ex = today.exceptions
		local left_width = 362
		local fixed_width = math.floor((left_width - 4) * ex.fixed / (ex.fixed + ex.outstanding))
		local with = {}
		for _, who in ex.with do
			table.insert(with, ui.row {
				gap = "small",
				align = "center",
				if who.icon
					then ui.row { width = 18, height = 18, align = "center", justify = "center", ui.icon { name = who.icon :: any, size = "small", class = "d-muted" } }
					else initials(who.initials, false),
				ui.text { who.name, size = "small" },
				ui.text { tostring(who.count), class = "mono-small" },
			})
		end
		local days = {}
		local track = 206
		for _, d in ex.days do
			table.insert(days, ui.row {
				align = "center",
				gap = "normal",
				ui.row { width = 64, shrink = false, ui.text { d.label, size = "small", class = if d.today then nil else "d-muted", weight = if d.today then "medium" else nil } },
				ui.row {
					class = "day-track",
					width = track,
					height = 6,
					ui.row {
						class = if d.today then "day-fill-today" elseif d.togo then "day-fill-togo" else "day-fill",
						width = math.floor(track * d.value / d.max),
						height = 6,
					},
				},
				ui.spacer(),
				if d.togo then ui.text { `{d.value} to go`, class = "day-togo" } else ui.text { tostring(d.value), class = "mono-small" },
			})
		end
		local exceptions = ui.column {
			class = "pair-left",
			width = left_width + 36,
			shrink = false,
			ui.row {
				align = "center",
				ui.text { ex.label, class = "d-muted" },
				ui.spacer(),
				ui.text { ex.count, class = "mono-muted" },
			},
			ui.row {
				align = "center",
				gap = "normal",
				ui.text { ex.value, class = "big-figure" },
				ui.text { ex.chip, class = "chip-good" },
			},
			ui.row { height = 4 },
			ui.row {
				align = "center",
				gap = "small",
				ui.row { class = "legend-square", width = 7, height = 7, style = { background = "blue" } },
				ui.text { "Fixed", class = "d-muted" },
				ui.text { tostring(ex.fixed) },
				ui.spacer(),
				ui.row { class = "legend-square", width = 7, height = 7, style = { background = "red" } },
				ui.text { "Outstanding", class = "d-muted" },
				ui.text { tostring(ex.outstanding) },
			},
			ui.row {
				gap = "small",
				ui.row { class = "bar-part", width = fixed_width, height = 9, style = { background = "blue" } },
				ui.row { class = "bar-part", width = left_width - 4 - fixed_width, height = 9, style = { background = "red" } },
			},
			ui.row { class = "soft-rule", height = 1 },
			ui.row { align = "center", ui.text { "Still open with", class = "muted-small" }, ui.spacer(), ui.row { gap = "large", table.unpack(with) } },
			ui.row { class = "soft-rule", height = 1 },
			ui.row { align = "center", ui.text { "Fixed this week", size = "small", weight = "medium" }, ui.spacer(), ui.text { ex.week, class = "mono-small" } },
			ui.column { gap = "normal", table.unpack(days) },
			ui.row { class = "soft-rule", height = 1 },
			ui.row { align = "center", ui.text { ex.foot, class = "muted-small" }, ui.spacer(), ui.text { ex.chasers, class = "muted-small" } },
		}

		-- Deadlines, sortable, each opening its page.
		local deadlines = {}
		for index, row in today.deadlines do
			table.insert(deadlines, { index = index, row = row })
		end
		local sort = state.sort
		if sort then
			table.sort(deadlines, function(a, b)
				local x, y
				if sort == "owner" then
					x, y = a.row.owner, b.row.owner
				elseif sort == "status" then
					x, y = TONE_ORDER[a.row.tone] or 9, TONE_ORDER[b.row.tone] or 9
				elseif sort == "item" then
					x, y = a.row.item, b.row.item
				else
					x, y = a.index, b.index
				end
				if x == y then
					return a.index < b.index
				end
				if state.descending then
					return x > y
				end
				return x < y
			end)
		end
		local function heading(key: string, title: string, width: number?)
			local arrow = if sort == key then (if state.descending then " ↓" else " ↑") else ""
			return ui.row {
				width = width,
				grow = width == nil,
				shrink = false,
				msg = { sort = key },
				ui.text { title .. arrow, class = "d-table-heading" },
			}
		end
		local due_count: { [string]: number } = {}
		for _, row in today.deadlines do
			local id = due_id(row.due)
			if id then
				due_count[id] = (due_count[id] or 0) + 1
			end
		end
		local table_rows = {}
		for position, entry in deadlines do
			local row = entry.row
			local id = due_id(row.due)
			local lit = state.calendar_day ~= nil and id == state.calendar_day
			table.insert(table_rows, ui.row {
				class = if position == #deadlines then { "d-table-row", "table-row-last" } else "d-table-row",
				align = "center",
				selected = state.selected == row.id or lit,
				msg = { row = row.id },
				ui.row { grow = true, ui.text { row.item, size = "small", truncate = true } },
				ui.row { width = 130, shrink = false, ui.text { row.owner, size = "small", class = "d-muted", truncate = true } },
				ui.row { width = 100, shrink = false, ui.text { row.due, size = "small", class = "d-muted" } },
				ui.row {
					width = 132,
					shrink = false,
					gap = "small",
					align = "center",
					ui.row { class = "d-status-dot", width = 5, height = 5, style = { background = row.tone } },
					ui.text { row.status, size = "small", class = "d-muted" },
				},
				ui.row { width = 50, shrink = false, justify = "end", ui.row { class = "open-link", msg = { go = row.page }, ui.text { "Open" } } },
			})
		end
		local calendar = ui.popover {
			open = state.calendar,
			dismiss = "close-calendar",
			side = "bottom",
			align = "end",
			width = 300,
			content = ui.column {
				class = "calendar-pop",
				ui.row { align = "center", ui.text { "October 2026", weight = "medium" }, ui.spacer(), ui.text { "dots: notices due", class = "muted-small" } },
				month_grid { weeks = october(due_count), selected = state.calendar_day, event = "day" },
			},
			ui.row { class = "ghost-small", align = "center", msg = "calendar", ui.icon { name = "calendar", size = "small" }, ui.text { "Notice calendar", size = "small", weight = "medium" } },
		}
		local deadlines_view = ui.column {
			class = "pair-right",
			grow = true,
			ui.row {
				align = "center",
				gap = "normal",
				ui.text { "Due this week", weight = "medium" },
				ui.text { `{#today.deadlines} items, soonest first`, class = "muted-small" },
				ui.spacer(),
				calendar,
			},
			ui.row {
				class = "table-head",
				align = "center",
				heading("item", "Item"),
				heading("owner", "Owner", 130),
				heading("due", "Due", 100),
				heading("status", "Status", 132),
				ui.row { width = 50 },
			},
			ui.column { gap = "none", table.unpack(table_rows) },
		}

		local pair = ui.column {
			class = "pair",
			ui.row {
				gap = "none",
				ui.row {
					class = "pair-head",
					width = left_width + 36,
					align = "center",
					ui.icon { name = "circle-alert", size = "small", class = "d-muted" },
					ui.text { "OPEN EXCEPTIONS", class = "pair-title" },
					ui.spacer(),
					ui.icon { name = "ellipsis-vertical", size = "small", class = "d-muted" },
				},
				ui.row {
					class = "pair-head",
					grow = true,
					align = "center",
					ui.icon { name = "clock", size = "small", class = "d-muted" },
					ui.text { "DEADLINES", class = "pair-title" },
					ui.spacer(),
					ui.icon { name = "ellipsis-vertical", size = "small", class = "d-muted" },
				},
			},
			ui.row { class = "pair-body", align = "stretch", exceptions, deadlines_view },
		}

		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column {
				class = "d-page",
				shrink = false,
				head,
				ui.row { height = 6 },
				strip,
				ui.row { gap = "normal", align = "stretch", chart_card, due_card },
				pair,
			},
		}
	end,
}
