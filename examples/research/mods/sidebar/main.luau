-- The sidebar: the workspace, New and search, the main places, recent
-- projects and teams (only "Savings launch, Q1" is in this example), the
-- plan's credits, and who is signed in.

local avatar = use "avatar"
local icon_button = use "icon-button"

type State = { selected: string }

local PLACES = {
	{ id = "home", label = "Home", icon = "house" },
	{ id = "projects", label = "Projects", icon = "folder" },
	{ id = "templates", label = "Templates", icon = "layout-dashboard" },
	{ id = "artifacts", label = "Artifacts", icon = "layers" },
	{ id = "tasks", label = "Tasks", icon = "list-checks", count = "2" },
}

local RECENT = {
	{ id = "savings", label = "Savings launch, Q1", icon = "folder" },
	{ id = "pricing", label = "Q3 pricing review", icon = "folder" },
	{ id = "board", label = "Board update, Nov", icon = "folder" },
	{ id = "hiring", label = "Hiring plan 2027", icon = "folder" },
}

local TEAMS = {
	{ id = "product", label = "Product", icon = "user" },
	{ id = "growth", label = "Growth", icon = "user" },
}

local TOTAL = 2000
local BEFORE = 1154 -- credits used before this run
local SEGMENTS = 24

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 0, 10, 10, 14 }, gap = 2 },
		[".side-brand"] = { padding = { 6, 6 }, gap = 8 },
		[".side-logo"] = { background = "foreground", color = "background", radius = 7 },
		[".side-new"] = { background = "background", border = true, border_color = "border", radius = 17, padding = { 0, 12 }, gap = 6 },
		[".side-new:hover"] = { background = "secondary" },
		[".side-search"] = { background = "background", border = true, border_color = "border", radius = 17, color = "muted-foreground" },
		[".side-search:hover"] = { background = "secondary" },
		[".side-heading"] = { size = "small", color = "muted-foreground", padding = { 16, 8, 6, 8 } },
		[".side-row"] = { padding = { 0, 8 }, radius = 8, gap = 10 },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { background = "list-active" },
		[".side-icon"] = { color = "muted-foreground" },
		[".side-count"] = { size = "small", color = "muted-foreground" },
		[".side-credits"] = { background = "background", radius = 12, border = true, border_color = "border", padding = { 12, 12, 10, 12 }, gap = 8, shadow = "0 1px 2px #00000008" },
		[".side-credits-icon"] = { background = "secondary", radius = 6, color = "muted-foreground" },
		[".side-pro"] = { background = "secondary", radius = 7, padding = { 2, 8 } },
		[".side-big"] = { size = "large", weight = "bold" },
		[".side-seg"] = { background = "accent", radius = 1 },
		[".side-seg-off"] = { background = "border", radius = 1 },
		[".side-link"] = { size = "small", color = "accent", weight = "medium" },
		[".side-user"] = { padding = { 10, 2, 0, 2 }, gap = 10 },
	},

	init = function(previous: State?): State
		return previous or { selected = "savings" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state: State, app)
		local window = app.window
		local run = app.research and app.research.run

		local function row(item: any)
			return ui.row {
				class = "side-row",
				height = 32,
				align = "center",
				selected = state.selected == item.id,
				msg = { select = item.id },
				tooltip = if item.id == "savings" then nil else "Only “Savings launch, Q1” is in this example",
				ui.icon { name = item.icon :: any, class = "side-icon" },
				ui.text { item.label, truncate = true },
				ui.spacer(),
				item.count and ui.text { item.count, class = "side-count" },
			}
		end

		local rows = {}
		for _, item in PLACES do
			table.insert(rows, row(item))
		end
		table.insert(rows, ui.text { "Recent", class = "side-heading" })
		for _, item in RECENT do
			table.insert(rows, row(item))
		end
		table.insert(rows, ui.text { "Teams", class = "side-heading" })
		for _, item in TEAMS do
			table.insert(rows, row(item))
		end

		local left = TOTAL - BEFORE - (if run then run.used else 86)
		local on = math.floor(SEGMENTS * (TOTAL - left) / TOTAL + 0.5) -- the bar is what is spent
		local segments = {}
		for i = 1, SEGMENTS do
			segments[i] = ui.row { class = if i <= on then "side-seg" else "side-seg-off", width = 5, height = 8 }
		end

		return ui.column {
			grow = true,
			gap = "none",
			ui.row { height = if window.platform == "macos" and not window.fullscreen then 34 else 8, drag = true },
			ui.column {
				class = "side",
				grow = true,
				ui.row {
					class = "side-brand",
					align = "center",
					ui.row {
						class = "side-logo",
						width = 22,
						height = 22,
						align = "center",
						justify = "center",
						ui.icon { name = "sparkles", size = "small" },
					},
					ui.text { "Sorrel", weight = "medium" },
					ui.spacer(),
					ui.icon { name = "chevron-down", size = "small", class = "side-icon" },
					icon_button { icon = "panel-left", tooltip = "Hide the sidebar", msg = "toggle-left", to = "shell", size = "small" },
				},
				ui.row {
					gap = "normal",
					align = "center",
					shrink = false,
					style = { padding = { 6, 0, 10, 0 } },
					ui.row {
						class = "side-new",
						grow = true,
						height = 34,
						align = "center",
						msg = "toggle",
						to = "thread",
						tooltip = "Run the demo again",
						ui.icon { name = "plus", size = "small" },
						ui.text { "New" },
					},
					ui.row {
						class = "side-search",
						width = 34,
						height = 34,
						align = "center",
						justify = "center",
						msg = "focus",
						to = "topbar",
						ui.icon { name = "search", size = "small" },
					},
				},
				ui.column { grow = true, scroll = true, gap = "none", ui.column { gap = "none", shrink = false, table.unpack(rows) } },
				ui.column {
					class = "side-credits",
					ui.row {
						align = "center",
						gap = "normal",
						ui.row { class = "side-credits-icon", width = 24, height = 24, align = "center", justify = "center", ui.icon { name = "zap", size = "small" } },
						ui.text { "Credits" },
						ui.spacer(),
						ui.row { class = "side-pro", ui.text { "Pro", size = "small", weight = "medium" } },
					},
					ui.row {
						align = "end",
						gap = "small",
						ui.text { tostring(left), class = "side-big" },
						ui.text { "left of 2,000", class = "small-muted" },
					},
					ui.row { style = { gap = 2 }, table.unpack(segments) },
					ui.row {
						align = "center",
						ui.text { "Resets Nov 1", class = "small-muted" },
						ui.spacer(),
						ui.text { "Upgrade", class = "side-link" },
					},
				},
				ui.row {
					class = "side-user",
					align = "center",
					avatar { name = "Noor Haddad", initial = "NH", shape = "circle", color = "#c08457" },
					ui.column {
						gap = "none",
						grow = true,
						ui.text { "Noor Haddad", size = "small", weight = "medium" },
						ui.text { "noor@sorrel.example", class = "small-muted", truncate = true },
					},
					icon_button { icon = "settings", tooltip = "Settings (not in this example)", size = "small", msg = { select = "settings" } },
				},
			},
		}
	end,
}
