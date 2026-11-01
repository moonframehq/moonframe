-- Reports: for the audit committee and the compliance file. The period
-- (a menu of ranges) and Export pack, five headline figures, on time by
-- notice type, payment speed by month (pick a month), gaps in the public
-- record with an AI insight, and the export pack, whose files you tick to
-- include. From `app.duly` (sample data).

local kpi_meter = use "kpi-meter"
local insight_note = use "insight-note"
local dashboard_card = use "dashboard-card"
local menu = use "menu"
local checkbox = use "checkbox"

type State = { month: number?, range: string, ranges: boolean, skip: { [string]: boolean }, exported: boolean }

local RANGES = { "Apr to Oct 2026", "Jul to Sep 2026", "Year to date", "Last 12 months" }
local TONES = { green = "success", red = "danger" }

return mod {
	title = "Reports",

	styles = {
		[".d-page"] = { padding = { 22, 30, 30, 30 }, gap = 12 },
		[".crumb-on"] = { size = "large" },
		[".d-page-title"] = { size = "display", weight = "medium" },
		[".d-muted"] = { color = "muted-foreground" },
		[".muted-small"] = { color = "muted-foreground", size = "small" },
		[".mono-small"] = { font = "mono", size = "small" },
		[".mono-muted"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".outline-button"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 7, 12 }, gap = 6, shadow = "0 1px 1px #00000008" },
		[".outline-button:hover"] = { background = "secondary" },
		[".dark-button"] = { background = "foreground", color = "background", radius = 8, padding = { 7, 12 }, gap = 6 },
		[".strip"] = { background = "secondary", border = true, border_color = "border", radius = 12, gap = 0 },
		[".strip-rule"] = { background = "border" },
		[".type-track"] = { background = "track", radius = 3 },
		[".type-fill"] = { background = "blue", radius = 3 },
		[".month"] = { radius = 6, gap = 8, padding = { 4, 0 } },
		[".month:hover"] = { background = "list-hover" },
		[".month-bar"] = { background = "blue/25", radius = 4 },
		[".month-bar-on"] = { background = "blue", radius = 4 },
		[".kv"] = { padding = { 12, 0 }, border = "bottom", border_color = "hairline" },
		[".kv-last"] = { border = "none" },
		[".kv:hover"] = { background = "list-hover" },
	},

	init = function(previous: State?): State
		local state: State = previous or { range = RANGES[1], ranges = false, skip = {}, exported = false }
		state.ranges = false
		state.exported = false
		return state
	end,

	update = function(state: State, msg)
		if msg == "ranges" then
			state.ranges = not state.ranges
		elseif msg == "close-ranges" then
			state.ranges = false
		elseif msg == "export" then
			state.exported = true
		elseif type(msg) == "table" then
			if msg.month then
				state.month = msg.month
			elseif msg.pick then
				state.range = msg.pick
				state.ranges = false
			elseif msg.include then
				state.skip[msg.include] = not state.skip[msg.include]
				state.exported = false
			end
		end
	end,

	view = function(state: State, app)
		local data = app.duly
		if not data or data.available == false then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { if data then data.reason or "No data" else "Loading…", class = "d-muted" } }
		end
		local reports = data.reports

		local included = 0
		for _, item in reports.exports do
			if not state.skip[item.id] then
				included += 1
			end
		end

		local range_options = {}
		for _, range in RANGES do
			table.insert(range_options, { id = range, label = range })
		end
		local head = ui.column {
			gap = "none",
			ui.row {
				drag = true,
				align = "center",
				gap = "normal",
				height = 36,
				ui.icon { name = "chart-column", class = "d-muted" },
				ui.text { "Reports", class = "crumb-on" },
			},
			ui.row { height = 22 },
			ui.row {
				align = "end",
				ui.column {
					gap = "small",
					ui.text { "Reports", class = "d-page-title" },
					ui.text { reports.subtitle, class = "d-muted" },
				},
				ui.spacer(),
				ui.row {
					gap = "normal",
					ui.popover {
						open = state.ranges,
						dismiss = "close-ranges",
						side = "bottom",
						align = "end",
						width = 200,
						content = menu { items = range_options },
						ui.row { class = "outline-button", align = "center", msg = "ranges", ui.icon { name = "calendar", size = "small" }, ui.text { state.range, weight = "medium" } },
					},
					ui.row {
						class = "dark-button",
						align = "center",
						msg = "export",
						tooltip = `{included} of {#reports.exports} files`,
						ui.icon { name = if state.exported then "check" else "download", size = "small" },
						ui.text { if state.exported then "Pack ready" else "Export pack", weight = "medium" },
					},
				},
			},
		}

		local cells = {}
		for index, kpi in reports.kpis do
			if index > 1 then
				table.insert(cells, ui.row { class = "strip-rule", width = 1 })
			end
			table.insert(cells, kpi_meter {
				title = kpi.title,
				dot = kpi.dot,
				value = kpi.value,
				mono = kpi.mono,
				caption = kpi.caption,
				caption_color = if kpi.caption_tone == "green" then "success" else nil,
				segments = kpi.bar,
				width = 184,
				msg = { month = 6 },
			})
		end

		-- On time by notice type.
		local track = 340
		local types = {}
		for _, kind in reports.types do
			table.insert(types, ui.row {
				align = "center",
				gap = "normal",
				height = 22,
				ui.row { width = 150, shrink = false, ui.text { kind.label, size = "small" } },
				ui.row {
					class = "type-track",
					width = track,
					height = 6,
					shrink = false,
					kind.total > 0 and ui.row { class = "type-fill", width = math.floor(track * kind.done / kind.total), height = 6 },
				},
				ui.row {
					width = 120,
					shrink = false,
					justify = "end",
					ui.text { if kind.due then kind.due else `{kind.done} of {kind.total}`, class = "mono-small" },
				},
			})
		end
		local types_card = dashboard_card {
			title = "On time by notice type",
			icon = "bell",
			menu = "types-menu",
			ui.column { gap = "small", table.unpack(types) },
		}

		-- Payment speed by month.
		local month = state.month or #reports.months
		local months = {}
		for index, m in reports.months do
			local on = index == month
			table.insert(months, ui.column {
				class = "month",
				grow = true,
				align = "center",
				justify = "end",
				height = 300,
				msg = { month = index },
				tooltip = `{m.label}: {m.text} paid on time, {m.days} on average`,
				ui.text { m.text, class = "mono-small" },
				ui.row { class = if on then "month-bar-on" else "month-bar", width = 102, height = math.floor((m.value - 70) * 10.5) },
			})
		end
		local labels = {}
		for _, m in reports.months do
			table.insert(labels, ui.column {
				grow = true,
				align = "center",
				gap = "small",
				ui.text { m.label, size = "small", class = "d-muted" },
				ui.text { m.days, class = "muted-small" },
			})
		end
		local speed_card = dashboard_card {
			title = "Payment speed by month",
			icon = "file",
			menu = "speed-menu",
			grow = true,
			ui.column { gap = "small", ui.row { gap = "normal", align = "end", table.unpack(months) }, ui.row { gap = "normal", table.unpack(labels) } },
		}

		-- Gaps in the public record.
		local gaps = {}
		for index, gap in reports.gaps do
			table.insert(gaps, ui.row {
				class = if index == #reports.gaps then { "kv", "kv-last" } else "kv",
				ui.text { gap.label, size = "small", class = "d-muted" },
				ui.spacer(),
				ui.text { gap.value, size = "small", style = if gap.tone then { color = TONES[gap.tone] } else nil },
			})
		end
		local gaps_card = dashboard_card {
			title = "Gaps in the public record",
			icon = "globe",
			menu = "gaps-menu",
			ui.column { gap = "none", table.unpack(gaps) },
			insight_note { text = reports.gaps_insight },
		}

		-- The export pack: tick the files to include.
		local exports = {}
		for index, item in reports.exports do
			local on = not state.skip[item.id]
			table.insert(exports, ui.row {
				class = if index == #reports.exports then { "kv", "kv-last" } else "kv",
				align = "center",
				msg = { include = item.id },
				checkbox { checked = on, msg = { include = item.id } },
				ui.text { item.label, size = "small", class = if on then nil else "d-muted" },
				ui.spacer(),
				ui.text { item.value, size = "small", weight = "medium", class = if on then nil else "d-muted" },
			})
		end
		local export_card = dashboard_card {
			title = "Export pack",
			icon = "download",
			meta = `{included} of {#reports.exports}`,
			menu = "export-menu",
			ui.column { gap = "none", table.unpack(exports) },
		}

		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column {
				class = "d-page",
				shrink = false,
				head,
				ui.row { height = 4 },
				ui.row { class = "strip", align = "stretch", table.unpack(cells) },
				ui.row {
					gap = "normal",
					align = "start",
					ui.column { grow = true, gap = "normal", types_card, speed_card },
					ui.column { width = 372, shrink = false, gap = "normal", gaps_card, export_card },
				},
			},
		}
	end,
}
