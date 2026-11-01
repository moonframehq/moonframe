-- The sidebar: the brand beside the traffic lights, then the pages in
-- sections (Main menu, Manage, Resources, Settings), the current one
-- raised. Choosing one tells the pages; Audience and Learn are drawn here.

type State = { page: string }

-- A typed list: some rows have a count, some not.
local SECTIONS: { { title: string, items: { any } } } = {
	{ title = "Main menu", items = {
		{ id = "overview", label = "Overview", icon = "layout-dashboard" },
		{ id = "audience", label = "Audience", icon = "user" },
		{ id = "campaigns", label = "Campaigns", icon = "send" },
		{ id = "content", label = "Content", icon = "file-text" },
		{ id = "engagement", label = "Engagement", icon = "chart-pie" },
		{ id = "revenue", label = "Revenue", icon = "hash" },
	} },
	{ title = "Manage", items = {
		{ id = "subscribers", label = "Subscribers", icon = "user", count = "24.8k" },
		{ id = "automations", label = "Automations", icon = "workflow" },
		{ id = "segments", label = "Segments", icon = "filter" },
		{ id = "integrations", label = "Integrations", icon = "plug" },
	} },
	{ title = "Resources", items = {
		{ id = "analytics", label = "Analytics", icon = "chart-column" },
		{ id = "reports", label = "Reports", icon = "file-text" },
		{ id = "learn", label = "Learn", icon = "book-open" },
	} },
	{ title = "Settings", items = {
		{ id = "help", label = "Help Center", icon = "circle-help" },
		{ id = "settings", label = "System Settings", icon = "settings" },
	} },
}

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 0, 14, 16, 14 }, gap = 2 },
		[".side-brand"] = { padding = { 0, 4 }, gap = 8 },
		[".side-logo"] = { background = "foreground", color = "background", radius = 7 },
		[".side-name"] = { size = "large", weight = "bold" },
		[".side-heading"] = { size = "small", color = "muted-foreground", case = "upper", padding = { 18, 10, 6, 10 } },
		[".side-row"] = { padding = { 7, 10 }, radius = 9, gap = 10 },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { background = "#00000010" },
		[".side-icon"] = { color = "muted-foreground" },
		[".side-count"] = { size = "small", border = true, border_color = "border", radius = 6, padding = { 0, 6 }, background = "background" },
		[".side-rule"] = { background = "border", margin = { 10, 6, 0, 6 } },
	},

	-- The page is published on `inkwell/page`, which the pages and the top
	-- bar follow; it goes out once at launch too, so they start in step.
	init = function(previous: State?)
		local state: State = previous or { page = "audience" }
		return state, { { to = "topic:inkwell/page", msg = state.page } }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.page then
			state.page = msg.page
			return state, { { to = "topic:inkwell/page", msg = msg.page } }
		end
	end,

	view = function(state: State, app)
		local window = app.window
		local rows = {
			ui.row {
				class = "side-brand",
				drag = true,
				height = 52,
				align = "center",
				window.platform == "macos" and not window.fullscreen and ui.row { width = 66 },
				ui.row {
					class = "side-logo",
					width = 26,
					height = 26,
					align = "center",
					justify = "center",
					ui.icon { name = "mail", size = "small" },
				},
				ui.text { "Inkwell", class = "side-name" },
			},
		}
		for index, section in SECTIONS do
			if index > 1 then
				table.insert(rows, ui.row { class = "side-rule", height = 1 })
			end
			table.insert(rows, ui.text { section.title, class = "side-heading" })
			for _, item in section.items do
				table.insert(rows, ui.row {
					class = "side-row",
					align = "center",
					selected = item.id == state.page,
					msg = { page = item.id },
					ui.icon { name = item.icon :: any, size = "small", class = "side-icon" },
					ui.text { item.label },
					ui.spacer(),
					item.count and ui.text { item.count, class = "side-count" },
				})
			end
		end
		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column { class = "side", shrink = false, table.unpack(rows) },
		}
	end,
}
