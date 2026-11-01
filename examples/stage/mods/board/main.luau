-- The board: Moonframe's components set out as a bento of tiles, each with
-- sample data and a caption naming what it is, to screenshot as stills.
-- Every number and name here is made up.

local pill = use "pill"
local metric = use "metric"
local kpi_cell = use "kpi-cell"
local forecast_chart = use "forecast-chart"
local dot_chart = use "dot-chart"
local share_bar = use "share-bar"
local ratio_row = use "ratio-row"
local month_grid = use "month-grid"
local day_strip = use "day-strip"
local task_row = use "task-row"
local progress_stack = use "progress-stack"
local avatar_stack = use "avatar-stack"
local date_tile = use "date-tile"
local segmented = use "segmented"
local tabs = use "tabs"
local switch = use "switch"
local chips = use "chips"
local kbd = use "kbd"
local meter = use "meter"
local status_dot = use "status-dot"
local diff_counts = use "diff-counts"
local code = use "code"
local file_tree = use "file-tree"
local country_code = use "country-code"
local kanban_column = use "kanban-column"
local board_card = use "board-card"
local chat_message = use "chat-message"
local thinking = use "thinking"
local composer = use "composer"
local mini_bars = use "mini-bars"
local numbered_list = use "numbered-list"

type State = { captions: boolean, backdrop: string, tab: string, mode: string, sync: boolean }

local DAYS: { any } = {
	{ id = "d1", day = "31", weekday = "Sat", today = true, dot = true },
	{ id = "d2", day = "1", weekday = "Sun", today = false, dot = false },
	{ id = "d3", day = "2", weekday = "Mon", today = false, dot = true },
	{ id = "d4", day = "3", weekday = "Tue", today = false, dot = false },
	{ id = "d5", day = "4", weekday = "Wed", today = false, dot = true },
	{ id = "d6", day = "5", weekday = "Thu", today = false, dot = false },
	{ id = "d7", day = "6", weekday = "Fri", today = false, dot = false },
}

local function month(): { any }
	local weeks = {}
	local day = 28
	for week = 1, 5 do
		local row = {}
		for _ = 1, 7 do
			local outside = (week == 1 and day > 7) or (week == 5 and day < 7)
			table.insert(row, {
				id = `{week}-{day}`,
				day = tostring(day),
				today = day == 31 and not outside,
				outside = outside,
				count = if day % 5 == 0 then 2 elseif day % 3 == 0 then 1 else 0,
			})
			day = if day == 30 and week == 1 then 1 elseif day == 31 then 1 else day + 1
		end
		table.insert(weeks, row)
	end
	return weeks
end

local BARS: { any } = {}
for index, value in { 46.8, 44, 53.2, 36.4, 34.7, 46.5, 44, 51.5, 40.4, 46, 26.4, 39, 28.9 } do
	local previous = if index == 1 then value else ({ 46.8, 44, 53.2, 36.4, 34.7, 46.5, 44, 51.5, 40.4, 46, 26.4, 39 })[index - 1]
	local delta = value - previous
	table.insert(BARS, {
		label = if index == 1 then "Now" else `W{index - 1}`,
		value = value,
		delta = if index == 1 then `${value}M` else string.format("%+.1f", delta),
		current = index == 1,
		low = index == 11,
	})
end

local COLUMNS: { any } = {}
for index = 1, 26 do
	table.insert(COLUMNS, { current = 40 + (index * 37) % 70, previous = 50 + (index * 23) % 55 })
end

local CODE: { string } = {
	"-- A mod: state, messages, and a view.",
	"return mod {",
	"\tregion = \"left\",",
	"\tinit = function(previous)",
	"\t\treturn previous or { count = 0 }",
	"\tend,",
	"\tupdate = function(state, msg)",
	"\t\tif msg == \"add\" then state.count += 1 end",
	"\tend,",
	"\tview = function(state)",
	"\t\treturn ui.button { `Clicked {state.count}`, msg = \"add\" }",
	"\tend,",
	"}",
}

return mod {
	region = "main",
	title = "Board",

	styles = {
		[".board"] = { padding = { 54, 40, 40, 40 }, gap = 20 },
		[".board-row"] = { gap = 20 },
		[".backdrop-mist"] = { background = "backdrop-mist" },
		[".backdrop-dusk"] = { background = "backdrop-dusk" },
		[".backdrop-paper"] = { background = "backdrop-paper" },
		[".tile"] = {
			background = "background",
			radius = 22,
			border = true,
			border_color = "border",
			padding = 22,
			gap = 14,
			shadow = "0 1px 2px #0000000a, 0 12px 32px -12px #0000002a",
		},
		[".tile-caption"] = { size = "small", weight = "medium", color = "muted-foreground" },
		[".hero-title"] = { size = "title", weight = "bold" },
		[".hero-line"] = { color = "muted-foreground", size = "large" },
		[".muted"] = { color = "muted-foreground" },
		[".chat-me"] = { background = "secondary", radius = 14, padding = { 8, 12 } },
		[".mono"] = { font = "mono" },
	},

	init = function(previous: State?): State
		return previous or { captions = true, backdrop = "mist", tab = "board", mode = "dark", sync = true }
	end,

	update = function(state: State, msg)
		if msg == "captions" then
			state.captions = not state.captions
		elseif type(msg) == "table" then
			if msg.backdrop then
				state.backdrop = msg.backdrop
			elseif msg.select then
				-- The tabs and the segmented control both say `select`.
				if msg.select == "board" or msg.select == "list" or msg.select == "timeline" then
					state.tab = msg.select
				else
					state.mode = msg.select
				end
			elseif msg.toggle_sync then
				state.sync = not state.sync
			end
		end
	end,

	view = function(state: State)
		local function tile(caption: string, width: number?, ...)
			return ui.column {
				class = "tile",
				width = width,
				grow = width == nil,
				state.captions and ui.text { caption, class = "tile-caption" },
				...,
			}
		end

		local hero = tile(
			"moonframe",
			520,
			ui.text { "Native apps, laid out in Luau", class = "hero-title" },
			ui.text { "Every panel, chart and card here is a component. Rust and GPUI draw it.", class = "hero-line", wrap = true },
			ui.row {
				gap = "small",
				wrap = true,
				pill { label = "Live reload", icon = "zap", tone = "accent", size = "normal" },
				pill { label = "Sandboxed", icon = "shield", tone = "success", size = "normal" },
				pill { label = "Providers", icon = "plug", tone = "warning", size = "normal" },
				pill { label = "120 components", icon = "layers", size = "normal" },
			},
			ui.row { gap = "small", align = "center", ui.text { "Search anything", class = "muted" }, kbd { keys = "cmd-k", small = true } }
		)

		local forecast = tile(
			"forecast-chart",
			nil,
			ui.row {
				gap = "large",
				metric { label = "Cash today", value = "$46.8M", size = "hero" },
				metric { label = "Lowest week", value = "$26.4M", note = "Week 10" },
				metric { label = "Free at the low", value = "$11.4M", note = "above the floor", tone = "success" },
			},
			forecast_chart { bars = BARS, max = 60, floor = 15, ticks = { "$60M", "$30M", "$0" }, height = 170, bar = 22 }
		)

		local kpis = tile(
			"kpi-cell",
			340,
			kpi_cell { title = "Readers", icon = "user", tint = "#7c5cff", value = "24,816", delta = "+6.4%", good = true, caption = "All active subscribers" },
			kpi_cell { title = "Unsubscribe rate", icon = "log-out", tint = "#3b82f6", value = "0.9%", delta = "+0.2%", good = false, caption = "187 left this month" }
		)

		local calendar = tile("month-grid", 420, ui.text { "October 2026", weight = "bold", size = "large" }, month_grid { weeks = month(), selected = "5-31" })

		local growth = tile(
			"dot-chart",
			nil,
			ui.text { "Signups per day", weight = "medium" },
			dot_chart { columns = COLUMNS, max = 120, rows = 8, ticks = { "120", "60", "0" }, labels = { "Oct 2", "Oct 12", "Oct 22", "Oct 31" }, size = 12 }
		)

		local sources = tile(
			"share-bar · ratio-row",
			420,
			share_bar { width = 370, shares = {
				{ share = 44, color = "#7c5cff", label = "Website" },
				{ share = 20, color = "#60a5fa", label = "Referrals" },
				{ share = 16, color = "#f59e0b", label = "Social" },
				{ share = 20, color = "#a3e635", label = "Imported" },
			} },
			ratio_row { label = "Website signup form", icon = "globe", count = "604", percent = "44%", share = 44, color = "#7c5cff", width = 370 },
			ratio_row { label = "Referral program", icon = "link", count = "276", percent = "20%", share = 20, color = "#60a5fa", width = 370 },
			ratio_row { label = "Canada", leading = country_code { code = "CA" }, count = "986", percent = "5%" }
		)

		local today = tile(
			"day-strip · task-row",
			560,
			day_strip { days = DAYS, selected = "d1" },
			task_row { title = "Water the plants", state = "done", date = "Sat, Oct 31" },
			task_row { title = "Finish the quarterly review", state = "progress", date = "Sat, Oct 31", time = "17:00", tags = { { label = "Work", color = "#3b6fd8" } } },
			task_row { title = "Call Sam about the offsite", state = "important", date = "Sat, Oct 31", time = "16:00", due = true, tags = { { label = "Work", color = "#3b6fd8" } } },
			task_row { title = "Dinner with Lea", event = true, date = "Sat, Oct 31", time = "19:00" }
		)

		local raise = tile(
			"progress-stack · avatar-stack",
			nil,
			ui.row {
				align = "end",
				ui.text { "$41.3M", size = "title", weight = "bold" },
				ui.text { "/ $75M", size = "title", class = "muted" },
				ui.spacer(),
				ui.text { "$6M short", tone = "danger", weight = "medium" },
			},
			progress_stack {
				total = 75,
				width = 500,
				marker = { value = 69, label = "Pace $69M" },
				labels = { "$0", "$25M", "$50M", "$75M" },
				segments = {
					{ value = 41.3, color = "accent", hatch = false },
					{ value = 8.5, color = "accent/45", hatch = false },
					{ value = 12, color = "accent/60", hatch = true },
					{ value = 13.2, color = "danger", hatch = true },
				},
			},
			ui.row {
				gap = "large",
				avatar_stack { people = { { name = "Ada Moss" }, { name = "Ben Ruiz" }, { name = "Cleo Park" } }, count = 38 },
				avatar_stack { people = { { name = "Dev Shah" }, { name = "Eli Grant" } }, count = 6 },
			}
		)

		local dates = tile(
			"date-tile",
			360,
			ui.row { gap = "normal", date_tile { month = "Nov", day = "2" }, ui.column { gap = "none", ui.text { "Send the deck", weight = "medium" }, ui.text { "Today", class = "muted" } } },
			ui.row { gap = "normal", date_tile { month = "Nov", day = "9", tone = "danger" }, ui.column { gap = "none", ui.text { "Missing one letter", weight = "medium" }, ui.text { "In 7 days", tone = "danger" } } },
			ui.row { gap = "normal", date_tile { month = "Jan", day = "4", tone = "accent" }, ui.column { gap = "none", ui.text { "Q4 distribution", weight = "medium" }, ui.text { "$21.2M", class = "mono" } } }
		)

		local editor = tile(
			"code · file-tree",
			nil,
			ui.row {
				gap = "large",
				align = "start",
				ui.column {
					width = 210,
					shrink = false,
					file_tree {
						dirs = {
							[""] = { "dapp", "dcrates", "fCargo.toml", "fREADME.md" },
							["app"] = { "dmods", "fmain.luau", "fstyle.luau" },
						},
						open = { app = true },
						selected = "app/main.luau",
						marks = { ["app/main.luau"] = "M" },
					},
				},
				ui.column { grow = true, height = 300, code { lines = CODE, path = "main.luau" } },
			}
		)

		local controls = tile(
			"tabs · segmented · switch · chips",
			420,
			tabs { variant = "pills", selected = state.tab, tabs = {
				{ id = "board", label = "Board", icon = "layout-dashboard" },
				{ id = "list", label = "List", icon = "list" },
				{ id = "timeline", label = "Timeline", icon = "history" },
			} },
			segmented { selected = state.mode, options = {
				{ id = "light", label = "Light", icon = "sun" },
				{ id = "dark", label = "Dark", icon = "moon" },
				{ id = "system", label = "System", icon = "laptop" },
			} },
			switch { label = "Sync with the bank feeds", on = state.sync, msg = { toggle_sync = true } },
			chips { selected = { "open", "mine" }, chips = {
				{ id = "open", label = "Open", count = 42 },
				{ id = "mine", label = "Mine", count = 8 },
				{ id = "agent", label = "Has agent", count = 3 },
			} },
			ui.row {
				gap = "normal",
				align = "center",
				status_dot { status = "working", chip = true, label = "Working" },
				status_dot { status = "blocked", chip = true },
				diff_counts { added = 38, removed = 16, files = 3 },
			},
			meter { value = 0.68, label = "Quota", width = 360 }
		)

		local board = tile(
			"kanban-column · board-card",
			nil,
			ui.row {
				gap = "normal",
				align = "start",
				kanban_column {
					title = "Qualified",
					color = "#3b82f6",
					count = 2,
					total = "$111K",
					board_card { title = "Pinewood Labs", subtitle = "Team rollout, 3 sites", badge = "30%", amount = "$68,000", date = "Dec 5", owner = "TO", owner_color = "#f59e0b" },
					board_card { title = "Solstice Power", subtitle = "Field crew licences", badge = "35%", amount = "$43,000", date = "Nov 27", owner = "SL", owner_color = "#ec4899" },
				},
				kanban_column {
					title = "Negotiation",
					color = "#f59e0b",
					count = 2,
					total = "$170K",
					board_card { title = "Lumen Retail", subtitle = "Platform renewal", badge = "At risk", badge_tone = "danger", tone = "danger", amount = "$98,000", date = "Nov 13", owner = "MK", owner_color = "#6366f1" },
					board_card { title = "Halden Freight", subtitle = "Fleet tracking", badge = "80%", amount = "$72,000", date = "Nov 26", owner = "RD", owner_color = "#22c55e" },
				},
				kanban_column {
					title = "Closed won",
					color = "#22c55e",
					count = 1,
					total = "$120K",
					board_card { title = "Quill Works", subtitle = "Enterprise plan", badge = "Won", badge_tone = "success", amount = "$120,000", date = "Oct 21", owner = "SL", owner_color = "#ec4899" },
				},
			}
		)

		local chat = tile(
			"chat-message · thinking · composer",
			460,
			chat_message { from = "user", text = "Which deal should I look at first?" },
			chat_message {
				author = "Sales agent",
				text = "Start with Lumen Retail: the largest deal at risk, and it closes soonest.",
				thinking { seconds = 6 },
				numbered_list { items = { "Send the revised pricing", "Book a call with their new lead", "Offer a two-year term" } },
			},
			ui.spacer(),
			composer { agent = "Sales agent", placeholder = "Ask the sales agent anything…", value = "" }
		)

		local revenue = tile(
			"mini-bars",
			300,
			ui.text { "Closed revenue", weight = "medium" },
			mini_bars { height = 60, bar = 22, bars = {
				{ label = "Jul", value = 180, future = false },
				{ label = "Aug", value = 240, future = false },
				{ label = "Sep", value = 210, future = false },
				{ label = "Oct", value = 300, future = false },
				{ label = "Nov", value = 420, future = false },
				{ label = "Dec", value = 380, future = true },
			} }
		)

		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			class = `backdrop-{state.backdrop}`,
			ui.column {
				class = "board",
				shrink = false,
				ui.row { class = "board-row", align = "stretch", hero, forecast, kpis },
				ui.row { class = "board-row", align = "stretch", calendar, growth, sources },
				ui.row { class = "board-row", align = "stretch", today, raise, dates },
				ui.row { class = "board-row", align = "stretch", editor, controls },
				ui.row { class = "board-row", align = "stretch", board, chat, revenue },
			},
		}
	end,
}
