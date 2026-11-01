-- The sidebar: the pages at the top (Overview, Workflows, Templates, Runs &
-- Logs, Analytics), the account's own rows at the bottom, and who is signed
-- in under a rule. Choosing a page tells the overview; only Overview is
-- drawn in this example.

type State = { page: string }

local PAGES: { any } = {
	{ id = "overview", label = "Overview", icon = "layout-grid" },
	{ id = "workflows", label = "Workflows", icon = "workflow" },
	{ id = "templates", label = "Templates", icon = "layers" },
	{ id = "runs", label = "Runs & Logs", icon = "list" },
	{ id = "analytics", label = "Analytics", icon = "chart-line" },
}

local ACCOUNT: { any } = {
	{ id = "notifications", label = "Notifications", icon = "bell" },
	{ id = "invite", label = "Invite teammates", icon = "user-plus" },
	{ id = "settings", label = "Settings", icon = "settings" },
}

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 0, 18, 0, 18 }, gap = 0 },
		[".side-row"] = { padding = { 0, 10 }, radius = 10, gap = 12 },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { color = "foreground" },
		[".side-icon"] = { color = "#7a7a7a" },
		[".side-icon:on"] = { color = "foreground" },
		[".side-label"] = { color = "label" },
		[".side-label:on"] = { color = "foreground", weight = "medium" },
		[".side-rule"] = { background = "#ffffff12" },
		[".side-user"] = { padding = { 0, 26, 0, 22 }, gap = 12 },
		[".side-user:hover"] = { background = "list-hover" },
		[".side-mark"] = { background = "#3a3a3a", radius = 15 },
		[".side-mark-dot"] = { background = "#8a8a8a", radius = 3 },
		[".side-name"] = { weight = "bold" },
	},

	init = function(previous: State?): State
		return previous or { page = "overview" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.page then
			state.page = msg.page
			return state, { { to = "overview", msg = { page = msg.page } } }
		end
	end,

	view = function(state: State)
		local function row(item: any)
			local on = item.id == state.page
			return ui.row {
				class = "side-row",
				height = 40,
				align = "center",
				selected = on,
				msg = { page = item.id },
				ui.icon { name = item.icon :: any, class = "side-icon", state = if on then "on" else nil },
				ui.text { item.label, class = "side-label", state = if on then "on" else nil },
			}
		end
		local top = {}
		for _, item in PAGES do
			table.insert(top, row(item))
		end
		local bottom = {}
		for _, item in ACCOUNT do
			table.insert(bottom, row(item))
		end

		-- The workspace mark: four dots in a round tile, like the app icon.
		local mark = ui.row {
			class = "side-mark",
			width = 30,
			height = 30,
			shrink = false,
			ui.row { class = "side-mark-dot", width = 6, height = 6, float = { top = 7, left = 12 } },
			ui.row { class = "side-mark-dot", width = 6, height = 6, float = { top = 12, left = 7 } },
			ui.row { class = "side-mark-dot", width = 6, height = 6, float = { top = 12, left = 17 } },
			ui.row { class = "side-mark-dot", width = 6, height = 6, float = { top = 17, left = 12 } },
		}

		return ui.column {
			grow = true,
			gap = "none",
			ui.row { height = 48, drag = true, shrink = false },
			ui.column { class = "side", shrink = false, style = { gap = 6 }, table.unpack(top) },
			ui.spacer(),
			ui.column { class = "side", shrink = false, style = { gap = 6 }, table.unpack(bottom) },
			ui.row { class = "side-rule", height = 1, shrink = false, style = { margin = { 16, 0, 0, 0 } } },
			ui.row {
				class = "side-user",
				height = 72,
				shrink = false,
				align = "center",
				msg = { page = "settings" },
				mark,
				ui.text { "Mira S.", class = "side-name" },
			},
		}
	end,
}
