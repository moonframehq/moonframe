-- The overview: a header (where you are, Export, New capital call), the
-- filters and when data last synced, then four cards: cash for the next 13
-- weeks against the floor, what is waiting on you, the current raise, and
-- the funds with each one's curve. All from `app.fund` (sample data).

local breadcrumbs = use "breadcrumbs"
local dropdown = use "dropdown"
local dashboard_card = use "dashboard-card"
local metric = use "metric"
local forecast_chart = use "forecast-chart"
local progress_stack = use "progress-stack"
local date_tile = use "date-tile"
local avatar = use "avatar"
local avatar_stack = use "avatar-stack"
local table_view = use "table"

type State = { filter: string? }

return mod {
	region = "main",
	title = "Overview",

	styles = {
		[".overview-head"] = { padding = { 0, 18, 0, 20 }, gap = 10, border = "bottom", border_color = "border" },
		[".overview-filters"] = { padding = { 12, 18 }, gap = 8 },
		[".overview-grid"] = { padding = { 4, 18, 18, 18 }, gap = 14 },
		[".overview-row"] = { gap = 14 },
		[".muted"] = { color = "muted-foreground" },
		[".muted-small"] = { color = "muted-foreground", size = "small" },
		[".mono"] = { font = "mono" },
		[".divider-v"] = { background = "border" },
		[".accounts"] = { background = "secondary", radius = 10, padding = { 6, 10 }, gap = 8, border = true, border_color = "border" },
		[".account"] = { radius = 6, color = "#ffffff" },
		[".legend-dot"] = { radius = 3 },
		[".waiting"] = { radius = 12, border = true, border_color = "border", padding = 12, gap = 12, background = "background" },
		[".waiting-soon"] = { color = "danger" },
		[".waiting-title"] = { weight = "medium" },
		[".primary-dark"] = { background = "foreground", color = "background", radius = 8, padding = { 6, 12 }, gap = 8 },
		[".ghost-text"] = { padding = { 6, 10 } },
		[".raise-short"] = { color = "danger", weight = "medium" },
		[".stage"] = { background = "secondary", radius = 12, padding = 12, gap = 8, border = true, border_color = "border" },
		[".stage-needed"] = { background = "danger/5", border_color = "danger/30" },
		[".stage-value"] = { font = "mono", weight = "medium", size = "large" },
		[".fund-icon"] = { background = "secondary", radius = 8, border = true, border_color = "border" },
		[".fund-curve"] = { color = "accent", background = "muted-foreground/10", border_color = "muted-foreground/40" },
		[".fund-note"] = { color = "muted-foreground", size = "small", padding = { 10, 14 } },
		[".button-outline"] = { radius = 8, border = true, border_color = "border", padding = { 6, 12 }, gap = 8 },
		[".button-blue"] = { background = "accent", color = "#ffffff", radius = 8, padding = { 6, 14 }, gap = 8 },
	},

	init = function(previous: State?): State
		return previous or { filter = nil }
	end,

	update = function(state: State, msg)
		if msg == "close" then
			state.filter = nil
		elseif type(msg) == "table" and msg.open then
			state.filter = if state.filter == msg.open then nil else msg.open
		elseif type(msg) == "table" and msg.pick then
			state.filter = nil
		end
	end,

	view = function(state: State, app)
		local fund = app.fund
		if not fund then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { "Loading…", class = "muted" } }
		end
		if fund.available == false then
			return ui.column { grow = true, align = "center", justify = "center", ui.text { fund.reason or "No data", class = "muted" } }
		end

		local function filter(id: string, icon: string, label: string, options: { any })
			return dropdown {
				icon = icon,
				options = options,
				selected = options[1].id,
				open = state.filter == id,
				toggle = { open = id },
				dismiss = "close",
			}
		end

		local head = ui.row {
			class = "overview-head",
			drag = true,
			height = 60,
			align = "center",
			ui.button { icon = "panel-left", variant = "ghost", small = true, msg = "toggle-left", to = "shell", tooltip = "Show or hide the sidebar" },
			breadcrumbs { crumbs = { { label = "Capital" }, { label = "Overview" } } },
			ui.spacer(),
			ui.row { class = "button-outline", align = "center", msg = "export", ui.icon { name = "file-text", size = "small" }, ui.text { "Export", weight = "medium" } },
			ui.row { class = "button-blue", align = "center", msg = "new-call", ui.icon { name = "plus", size = "small" }, ui.text { "New capital call", weight = "medium" } },
		}

		local filters = ui.row {
			class = "overview-filters",
			align = "center",
			filter("weeks", "calendar", "Next 13 weeks", { { id = "13", label = "Next 13 weeks" }, { id = "26", label = "Next 26 weeks" } }),
			filter("funds", "layers", "All funds", { { id = "all", label = "All funds" }, { id = "credit", label = "Credit only" } }),
			filter("currency", "hash", "USD", { { id = "usd", label = "USD" }, { id = "eur", label = "EUR" } }),
			ui.spacer(),
			ui.text { fund.synced, class = "muted-small" },
		}

		-- Cash for the next 13 weeks.
		local cash = fund.cash
		local accounts = {}
		for _, account in cash.accounts do
			table.insert(accounts, ui.row {
				class = "account",
				width = 22,
				height = 22,
				align = "center",
				justify = "center",
				style = { background = account.color },
				tooltip = account.name,
				ui.text { account.initials, size = "small", weight = "bold" },
			})
		end
		local cash_card = dashboard_card {
			title = "Cash · next 13 weeks",
			icon = "chart-column",
			grow = true,
			menu = "cash-menu",
			meta = ui.row {
				gap = "large",
				align = "center",
				ui.row { gap = "small", align = "center", ui.row { class = "legend-dot", width = 8, height = 8, style = { background = "accent" } }, ui.text { "Free to use", class = "muted-small" } },
				ui.row { gap = "small", align = "center", ui.hatch { width = 9, height = 9, gap = 3, style = { color = "muted-foreground/60" } }, ui.text { `Held at the ${cash.floor}M floor`, class = "muted-small" } },
			},
			ui.row {
				gap = "large",
				align = "start",
				metric { label = "Cash on hand today", value = cash.today, size = "hero" },
				ui.row { class = "divider-v", width = 1, height = 56 },
				metric { label = "Lowest week", value = cash.lowest.value, note = cash.lowest.note },
				metric { label = "Free at the low", value = cash.free.value, note = cash.free.note, tone = "success" },
				metric { label = "In 13 weeks", value = cash["end"].value, note = cash["end"].note },
				ui.spacer(),
				ui.row {
					class = "accounts",
					align = "center",
					shrink = false,
					ui.row { gap = "none", shrink = false, table.unpack(accounts) },
					ui.text { `{#accounts} accounts`, class = "muted-small", wrap = false },
				},
			},
			forecast_chart { bars = cash.weeks, max = cash.max, floor = cash.floor, ticks = cash.ticks, height = 250 },
		}

		-- What is waiting on you.
		local waiting = {}
		for _, item in fund.waiting do
			table.insert(waiting, ui.row {
				class = "waiting",
				align = "start",
				date_tile { month = item.month, day = item.day, tone = item.tone },
				ui.column {
					grow = true,
					gap = "small",
					ui.text { item.when, class = if item.tone == "danger" then "waiting-soon" else "muted-small" },
					ui.text { item.title, class = "waiting-title" },
					ui.row {
						align = "center",
						gap = "small",
						avatar { name = item.who, initial = item.initials, size = "small" },
						ui.text { item.who, class = "muted-small", truncate = true },
						ui.spacer(),
						ui.text { item.amount, class = "mono", weight = "medium" },
					},
					item.actions and ui.row {
						gap = "small",
						ui.row { class = "primary-dark", align = "center", msg = "send", ui.icon { name = "check", size = "small" }, ui.text { "Send deck", weight = "medium" } },
						ui.row { class = "ghost-text", msg = "snooze", ui.text { "Snooze" } },
					},
				},
			})
		end
		local waiting_card = dashboard_card {
			title = "Waiting on you",
			icon = "inbox",
			meta = ui.badge { tostring(#fund.waiting) },
			menu = "waiting-menu",
			footer = ui.row {
				grow = true,
				align = "center",
				ui.icon { name = "check", size = "small", tone = "success" },
				ui.text { `{fund.handled} handled this week`, class = "muted-small" },
				ui.spacer(),
				ui.text { "View all ›", class = "muted-small" },
			},
			table.unpack(waiting),
		}

		-- The current raise.
		local raise = fund.raise
		local stages = {}
		for _, stage in raise.stages do
			local people = {}
			for _, name in stage.people do
				table.insert(people, { name = name })
			end
			table.insert(stages, ui.column {
				class = if stage.note then { "stage", "stage-needed" } else "stage",
				grow = true,
				ui.row {
					gap = "small",
					align = "center",
					if stage.hatch
						then ui.hatch { width = 10, height = 10, gap = 3, style = { color = stage.color } }
						else ui.row { class = "legend-dot", width = 10, height = 10, style = { background = stage.color } },
					ui.text { stage.label, class = if stage.note then "raise-short" else "muted-small" },
				},
				ui.text { stage.value, class = "stage-value" },
				if stage.note then ui.text { stage.note, class = "muted-small" } else avatar_stack { people = people, count = stage.count },
			})
		end
		local raise_card = dashboard_card {
			title = raise.title,
			icon = "chart-column",
			meta = raise.closes,
			menu = "raise-menu",
			ui.row {
				align = "end",
				ui.column {
					gap = "none",
					ui.text { "Closed so far", class = "muted" },
					ui.row {
						gap = "small",
						align = "end",
						ui.text { raise.closed, size = "title", weight = "bold" },
						ui.text { `/ {raise.target}`, size = "title", class = "muted" },
					},
				},
				ui.spacer(),
				ui.column { gap = "none", align = "end", ui.text { "At this pace", class = "muted" }, ui.text { raise.pace, class = "raise-short" } },
			},
			progress_stack { segments = raise.segments, total = raise.total, width = 460, marker = raise.marker, labels = raise.labels },
			ui.row { gap = "small", table.unpack(stages) },
		}

		-- The funds.
		local rows = {}
		for index, row in fund.funds.rows do
			table.insert(rows, {
				id = tostring(index),
				fund = ui.row {
					align = "center",
					gap = "normal",
					ui.row { class = "fund-icon", width = 32, height = 32, align = "center", justify = "center", ui.icon { name = row.icon :: any, size = "small" } },
					ui.column { gap = "none", shrink = true, ui.text { row.name, weight = "medium", wrap = false, truncate = true }, ui.text { row.detail, class = "muted-small", wrap = false, truncate = true } },
				},
				nav = row.nav,
				called = row.called,
				dpi = row.dpi,
				irr = row.irr,
				curve = ui.line { points = row.curve, baseline = 0, area = true, dots = "last", width = 104, height = 34, min = -1.4, max = 1.0, class = "fund-curve" },
			})
		end
		local funds_card = dashboard_card {
			title = "Funds",
			icon = "layers",
			meta = fund.funds.summary,
			menu = "funds-menu",
			grow = true,
			padding = false,
			table_view {
				columns = {
					{ key = "fund", title = "FUND" },
					{ key = "nav", title = "NAV", width = 90, align = "right", mono = true },
					{ key = "called", title = "CALLED", width = 80, align = "right", mono = true },
					{ key = "dpi", title = "DPI", width = 70, align = "right", mono = true },
					{ key = "irr", title = "NET IRR", width = 80, align = "right", mono = true },
					{ key = "curve", title = "NET CASH", width = 120 },
				},
				rows = rows,
				sortable = false,
			},
			ui.text { fund.funds.note, class = "fund-note" },
		}

		return ui.column {
			grow = true,
			gap = "none",
			head,
			filters,
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column {
					class = "overview-grid",
					shrink = false,
					ui.row { class = "overview-row", align = "stretch", cash_card, ui.column { width = 470, shrink = false, waiting_card } },
					ui.row { class = "overview-row", align = "stretch", ui.column { width = 540, shrink = false, raise_card }, funds_card },
				},
			},
		}
	end,
}
