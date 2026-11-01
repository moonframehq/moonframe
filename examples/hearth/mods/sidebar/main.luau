-- The sidebar: the org switcher under the traffic lights, a search field,
-- the pages in sections (Monitor, Analyze, Triage) with Code Review raised
-- and marked by an orange edge, pinned dashboards, then Sources, Settings
-- and who is signed in. Only Code Review is drawn in this example.

local kbd = use "kbd"

type State = { page: string, query: string }

-- A typed list: some rows carry a tag or provider marks, some not.
local SECTIONS: { { title: string?, count: string?, items: { any } } } = {
	{ items = { { id = "overview", label = "Overview", icon = "house" } } },
	{ title = "Monitor", items = {
		{ id = "services", label = "Services", icon = "server" },
		{ id = "infrastructure", label = "Infrastructure", icon = "square-terminal", marks = { "cloud", "globe" } },
	} },
	{ title = "Analyze", items = {
		{ id = "explore", label = "Explore", icon = "layers" },
		{ id = "web", label = "Web Analytics", icon = "chart-column" },
		{ id = "dashboards", label = "Dashboards", icon = "layout-dashboard" },
	} },
	{ title = "Triage", items = {
		{ id = "errors", label = "Errors", icon = "circle-alert" },
		{ id = "alerts", label = "Alerts", icon = "bell" },
		{ id = "review", label = "Code Review", icon = "code", tag = "BETA" },
		{ id = "security", label = "Security", icon = "shield", tag = "BETA" },
	} },
	{ title = "Pinned dashboards", count = "4", items = {
		{ id = "pin-ingest", label = "Ingest Service", icon = "layout-dashboard", pinned = true },
		{ id = "pin-query", label = "Query Engine", icon = "layout-dashboard", pinned = true },
		{ id = "pin-cache", label = "Query Engine Cache", icon = "layout-dashboard", pinned = true },
		{ id = "pin-volume", label = "Ingest Volume (Internal)", icon = "layout-dashboard", pinned = true },
	} },
}

local FOOT: { any } = {
	{ id = "sources", label = "Sources", icon = "plug" },
	{ id = "settings", label = "Settings", icon = "settings" },
}

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 0, 8, 8, 8 }, gap = 0 },
		[".side-org"] = { padding = { 4, 6, 4, 4 }, gap = 9, radius = 7 },
		[".side-org:hover"] = { background = "list-hover" },
		[".side-logo"] = { radius = 8 },
		[".side-org-name"] = { weight = "bold" },
		[".side-org-project"] = { size = "small", color = "accent" },
		[".side-org-region"] = { size = "small", color = "muted-foreground" },
		[".side-search"] = { radius = 7, border = true, border_color = "border", background = "background", padding = { 0, 6, 0, 9 }, gap = 7, margin = { 10, 0, 18, 0 } },
		[".side-heading"] = { size = "small", color = "muted-foreground", padding = { 22, 8, 7, 8 } },
		[".side-row"] = { padding = { 0, 8 }, radius = 6, gap = 11 },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { background = "raised", color = "accent" },
		[".side-edge"] = { background = "accent", radius = 1 },
		[".side-icon"] = { color = "foreground" },
		[".side-icon-pinned"] = { color = "muted-foreground" },
		[".side-tag"] = { size = "small", color = "muted-foreground", border = true, border_color = "border", radius = 4, padding = { 1, 5 } },
		[".side-tag-on"] = { border = "none" },
		[".side-mark"] = { color = "muted-foreground", size = "small" },
		[".side-kbd"] = { background = "raised", radius = 4, padding = { 1, 5 } },
		[".side-count"] = { size = "small", color = "muted-foreground" },
		[".side-user"] = { border = "top", border_color = "border", padding = { 0, 14, 0, 16 }, gap = 10 },
		[".side-initials"] = { background = "#c79a78", color = "#2a1f17", radius = 5 },
		[".side-initials-text"] = { size = "small", weight = "medium" },
	},

	init = function(previous: State?): State
		-- Only Code Review is drawn here, so every launch opens on it.
		local state: State = previous or { page = "review", query = "" }
		state.page = "review"
		return state
	end,

	update = function(state: State, msg)
		if type(msg) == "table" then
			if msg.page then
				state.page = msg.page
				return state, { { to = "review", msg = { page = msg.page } } }
			elseif msg.msg == "query" then
				state.query = msg.value
			end
		end
	end,

	view = function(state: State, app)
		local data = app.hearth
		local org = data and data.org or { name = "Hearth", project = "core", region = "EU" }
		local user = data and data.user or { name = "", initials = "" }

		local function row(item: any)
			local selected = item.id == state.page
			return ui.row {
				class = "side-row",
				height = if item.pinned then 30 else 33,
				align = "center",
				selected = selected,
				msg = { page = item.id },
				selected and ui.row { class = "side-edge", width = 2, float = { top = 7, bottom = 7, left = 0 } },
				ui.icon { name = item.icon :: any, size = "small", class = if item.pinned then "side-icon-pinned" else "side-icon" },
				ui.text { item.label, truncate = true },
				ui.spacer(),
				item.tag and ui.text { item.tag, class = if selected then { "side-tag", "side-tag-on" } else "side-tag" },
				item.marks and ui.row {
					gap = "small",
					ui.icon { name = item.marks[1], class = "side-mark", style = { color = "#e8955a" } },
					ui.icon { name = item.marks[2], class = "side-mark" },
				},
			}
		end

		local rows = {}
		for _, section in SECTIONS do
			if section.title then
				table.insert(rows, ui.row {
					class = "side-heading",
					align = "center",
					ui.text { string.upper(section.title) },
					ui.spacer(),
					section.count and ui.text { section.count, class = "side-count" },
				})
			end
			for _, item in section.items do
				table.insert(rows, row(item))
			end
		end
		local foot = {}
		for _, item in FOOT do
			table.insert(foot, row(item))
		end

		return ui.column {
			grow = true,
			gap = "none",
			ui.row { height = 28, drag = true, shrink = false },
			ui.column {
				class = "side",
				grow = true,
				scroll = true,
				ui.column {
					shrink = false,
					gap = "none",
					ui.row {
						class = "side-org",
						align = "center",
						msg = "switch-org",
						ui.image { src = "mascot.png", width = 30, height = 30, class = "side-logo" },
						ui.column {
							gap = "none",
							ui.text { org.name, class = "side-org-name" },
							ui.row {
								gap = "small",
								ui.text { org.project, class = "side-org-project" },
								ui.text { `· {org.region}`, class = "side-org-region" },
							},
						},
						ui.spacer(),
						ui.icon { name = "chevrons-up-down", size = "small", tone = "muted" },
					},
					ui.row {
						class = "side-search",
						height = 30,
						align = "center",
						ui.icon { name = "search", size = "small", tone = "muted" },
						ui.input { id = "search", value = state.query, placeholder = "Search", change = "query", grow = true, mono = true, class = "bare-input" },
						ui.row { class = "side-kbd", kbd { keys = "cmd-k", small = true, plain = true } },
					},
					table.unpack(rows),
				},
				ui.spacer(),
				ui.column { gap = "none", shrink = false, table.unpack(foot) },
			},
			ui.row {
				class = "side-user",
				height = 50,
				shrink = false,
				align = "center",
				ui.row {
					class = "side-initials",
					width = 24,
					height = 24,
					align = "center",
					justify = "center",
					ui.text { user.initials, class = "side-initials-text" },
				},
				ui.text { user.name, truncate = true },
				ui.spacer(),
				ui.icon { name = "circle-help", tone = "muted" },
			},
		}
	end,
}
