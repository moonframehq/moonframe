-- The settings sidebar: a way back to the store, then the admin's pages in
-- groups under small headings. The page in front (from `store/tab`) is the
-- white row; a click opens a page in the tab in front.

type State = { page: string }

local SECTIONS: { { title: string, items: { any } } } = {
	{ title = "Sales Channels", items = {
		{ id = "online-store", label = "Online store", icon = "monitor" },
		{ id = "point-of-sale", label = "Point of sale", icon = "keyboard" },
		{ id = "social", label = "Social and integrations", icon = "git-branch" },
	} },
	{ title = "Customers", items = {
		{ id = "customers", label = "Customers", icon = "user" },
		{ id = "segments", label = "Segments", icon = "chart-pie" },
	} },
	{ title = "Content", items = {
		{ id = "domains", label = "Domains", icon = "globe" },
		{ id = "brand", label = "Brand", icon = "palette" },
		{ id = "notifications", label = "Notifications", icon = "bell" },
	} },
	{ title = "Operations", items = {
		{ id = "products", label = "Products", icon = "tag" },
		{ id = "inventory", label = "Inventory", icon = "layers" },
		{ id = "orders", label = "Orders", icon = "calendar" },
	} },
	{ title = "Automations", items = {
		{ id = "automations", label = "Automations", icon = "workflow" },
		{ id = "ai-tools", label = "AI tools", icon = "sparkles" },
	} },
	{ title = "Apps", items = {
		{ id = "apps", label = "Apps", icon = "layout-dashboard" },
		{ id = "integrations", label = "Integrations", icon = "plug" },
	} },
	{ title = "System", items = {
		{ id = "audit", label = "Audit log", icon = "history" },
	} },
}

return mod {
	region = "left",
	title = "Sidebar",
	subscribe = { "store/tab" },

	styles = {
		[".side"] = { padding = { 4, 8, 12, 10 }, gap = 0 },
		[".side-back"] = { padding = { 6, 8 }, radius = 7, gap = 10, color = "muted-foreground" },
		[".side-back:hover"] = { background = "list-hover", color = "foreground" },
		[".side-group"] = { border = "top", border_color = "border", margin = { top = 6 }, padding = { top = 6 }, gap = 0 },
		[".side-heading"] = { size = "small", color = "muted-foreground", padding = { 2, 8, 3, 8 } },
		[".side-row"] = { padding = { 5, 8 }, radius = 7, gap = 10, color = "foreground/72" },
		[".side-row:hover"] = { background = "list-hover", color = "foreground" },
		[".side-row:selected"] = { background = "background", color = "foreground", shadow = "0 1px 2px #0000000d" },
		[".side-label"] = { weight = "medium" },
	},

	init = function(previous: State?): State
		return previous or { page = "audit" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.topic == "store/tab" and msg.value then
			state.page = msg.value.page or ""
		end
	end,

	view = function(state: State)
		local groups = {}
		for _, section in SECTIONS do
			local rows = { ui.text { section.title, class = "side-heading" } }
			for _, item in section.items do
				table.insert(rows, ui.row {
					class = "side-row",
					align = "center",
					selected = state.page == item.id,
					msg = { open = item.id, label = item.label, icon = item.icon },
					to = "topbar",
					ui.icon { name = item.icon :: any },
					ui.text { item.label, class = "side-label", truncate = true },
				})
			end
			table.insert(groups, ui.column { class = "side-group", table.unpack(rows) })
		end
		return ui.column {
			class = "side",
			grow = true,
			ui.row {
				class = "side-back",
				align = "center",
				msg = { open = "home" },
				to = "topbar",
				ui.icon { name = "arrow-left" },
				ui.text { "Back to app", class = "side-label" },
			},
			ui.column { grow = true, scroll = true, gap = "none", ui.column { gap = "none", shrink = false, table.unpack(groups) } },
		}
	end,
}
