-- The sidebar: the firm, a search, the sections (Capital, Investors,
-- Portfolio, Connected) with counts, the next big date, and who is
-- signed in.

local avatar = use "avatar"
local kbd = use "kbd"
local date_tile = use "date-tile"

type State = { selected: string }

local SECTIONS: { { title: string, items: { any } } } = {
	{ title = "Capital", items = {
		{ id = "overview", label = "Overview", icon = "layout-dashboard" },
		{ id = "funds", label = "Funds", icon = "layers" },
		{ id = "loans", label = "Loan book", icon = "building-2" },
		{ id = "calls", label = "Capital calls", icon = "calendar", count = 2 },
		{ id = "distributions", label = "Distributions", icon = "download" },
	} },
	{ title = "Investors", items = {
		{ id = "investors", label = "Investors", icon = "user" },
		{ id = "fundraising", label = "Fundraising", icon = "chart-column" },
		{ id = "pipeline", label = "Pipeline", icon = "list" },
		{ id = "dataroom", label = "Data room", icon = "file-text" },
	} },
	{ title = "Portfolio", items = {
		{ id = "properties", label = "Properties", icon = "map" },
		{ id = "watchlist", label = "Watchlist", icon = "triangle-alert", count = 3 },
		{ id = "reports", label = "Reports", icon = "chart-pie" },
	} },
	{ title = "Connected", items = {
		{ id = "bank", label = "Bank feeds", icon = "building-2" },
		{ id = "admin", label = "Fund admin", icon = "server" },
	} },
}

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 46, 14, 12, 14 }, gap = 0 },
		[".side-org"] = { padding = { 4, 6, 12, 6 }, gap = 10 },
		[".side-logo"] = { background = "foreground", color = "background", radius = 9 },
		[".side-search"] = { background = "background", border = true, border_color = "border", radius = 10, padding = { 7, 10 }, gap = 8 },
		[".side-heading"] = { size = "small", weight = "medium", color = "muted-foreground", padding = { 18, 10, 6, 10 } },
		[".side-row"] = { padding = { 7, 10 }, radius = 9, gap = 12 },
		[".side-row:hover"] = { background = "#0000000a" },
		[".side-row:selected"] = { background = "background", border = true, border_color = "border", shadow = "0 1px 2px #0000000a" },
		[".side-icon"] = { color = "muted-foreground" },
		[".side-count"] = { size = "small", color = "muted-foreground", border = true, border_color = "border", radius = 6, padding = { 0, 6 } },
		[".side-next"] = { background = "secondary", radius = 14, padding = 6, gap = 6, border = true, border_color = "border" },
		[".side-next-head"] = { padding = { 4, 6, 0, 6 } },
		[".side-next-card"] = { background = "background", radius = 10, padding = 10, gap = 12, border = true, border_color = "border" },
		[".side-person"] = { padding = { 12, 6, 0, 6 }, gap = 10 },
		[".muted-small"] = { size = "small", color = "muted-foreground" },
		[".mono"] = { font = "mono" },
	},

	init = function(previous: State?): State
		return previous or { selected = "overview" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state: State, app)
		local fund = app.fund
		local rows = {}
		for _, section in SECTIONS do
			table.insert(rows, ui.text { string.upper(section.title), class = "side-heading" })
			for _, item in section.items do
				table.insert(rows, ui.row {
					class = "side-row",
					align = "center",
					selected = state.selected == item.id,
					msg = { select = item.id },
					ui.icon { name = item.icon :: any, class = if state.selected == item.id then nil else "side-icon", tone = if state.selected == item.id then "accent" else nil },
					ui.text { item.label },
					ui.spacer(),
					item.count and ui.text { tostring(item.count), class = "side-count" },
				})
			end
		end
		local org = fund and fund.org or { name = "…", detail = "", initials = "" }
		local person = fund and fund.person
		local next = fund and fund.next
		return ui.column {
			class = "side",
			grow = true,
			ui.row {
				class = "side-org",
				drag = true,
				align = "center",
				ui.row { class = "side-logo", width = 34, height = 34, align = "center", justify = "center", ui.icon { name = "building-2" } },
				ui.column {
					gap = "none",
					ui.text { org.name, weight = "medium" },
					ui.text { org.detail, class = "muted-small" },
				},
				ui.spacer(),
				ui.icon { name = "chevrons-up-down", size = "small", class = "side-icon" },
			},
			ui.row {
				class = "side-search",
				align = "center",
				ui.icon { name = "search", size = "small", class = "side-icon" },
				ui.text { "Search", class = "side-icon" },
				ui.spacer(),
				kbd { keys = { "/" }, small = true },
			},
			ui.column { grow = true, scroll = true, gap = "none", ui.column { gap = "none", shrink = false, table.unpack(rows) } },
			next and ui.column {
				class = "side-next",
				ui.row {
					class = "side-next-head",
					ui.text { "UP NEXT", class = "muted-small" },
					ui.spacer(),
					ui.text { next.when, class = "muted-small" },
				},
				ui.row {
					class = "side-next-card",
					date_tile { month = next.month, day = next.day },
					ui.column {
						gap = "none",
						ui.text { next.title, weight = "medium" },
						ui.text { next.amount, class = "mono" },
						ui.text { next.detail, class = "muted-small" },
					},
				},
			},
			person and ui.row {
				class = "side-person",
				align = "center",
				avatar { name = person.name, shape = "circle", size = "large" },
				ui.column { gap = "none", ui.text { person.name, weight = "medium" }, ui.text { person.role, class = "muted-small" } },
				ui.spacer(),
				ui.icon { name = "ellipsis-vertical", size = "small", class = "side-icon" },
			},
		}
	end,
}
