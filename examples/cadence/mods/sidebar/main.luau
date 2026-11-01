-- The sidebar: the brand beside the traffic lights, the workspace's pages
-- (only Dashboard is in this example), recent workflow runs, the month's
-- usage with Upgrade, and who is signed in.

local avatar = use "avatar"

type State = { selected: string, starred: { [string]: boolean } }

type Page = { id: string, label: string, icon: string, badge: string?, tone: string?, trailing: string? }

local WORKSPACE: { Page } = {
	{ id = "dashboard", label = "Dashboard", icon = "layout-dashboard" },
	{ id = "library", label = "Workflow Library", icon = "workflow", badge = "28", tone = "blue" },
	{ id = "canvas", label = "Workflow Canvas", icon = "frame", trailing = "zap" },
	{ id = "templates", label = "Templates", icon = "layers" },
	{ id = "analytics", label = "Analytics", icon = "chart-column" },
	{ id = "team", label = "Team Members", icon = "user", badge = "35+", tone = "green" },
	{ id = "integrations", label = "Integrations", icon = "plug" },
}

local AGENTS: { Page } = {
	{ id = "settings", label = "Settings", icon = "settings" },
	{ id = "help", label = "Help & Support", icon = "circle-help" },
}

local RUNS = {
	{ id = "support", label = "Email support router workflow", color = "#f0606a" },
	{ id = "leads", label = "Lead scoring pipeline Workflow", color = "#f2b632" },
	{ id = "weekly", label = "Weekly report gen workflow", color = "#35c3d8" },
	{ id = "crm", label = "CRM sync workflow", color = "#f59152" },
}

local function heading(label: string, action: any?)
	return ui.row {
		class = "side-heading",
		align = "center",
		ui.icon { name = "circle-dashed", size = "small" },
		ui.text { label },
		ui.spacer(),
		action,
	}
end

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 0, 10, 10, 10 }, gap = 2 },
		[".side-top"] = { padding = { 0, 12, 0, 0 }, gap = 8, border = "bottom", border_color = "border" },
		[".side-logo"] = { background = "ink", color = "background", radius = 6 },
		[".side-brand"] = { weight = "bold", size = "large" },
		[".side-toggle"] = { color = "muted-foreground", radius = 6, padding = 4 },
		[".side-toggle:hover"] = { background = "list-hover", color = "foreground" },
		[".side-heading"] = { size = "small", color = "muted-foreground", padding = { 14, 8, 6, 8 }, gap = 8 },
		[".side-row"] = { padding = { 7, 8 }, radius = 8, gap = 10, color = "#4a4a50" },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { background = "muted", color = "foreground" },
		[".side-icon"] = { color = "#6b6b72" },
		[".side-badge"] = { radius = 9, padding = { 1, 7 }, size = "small" },
		[".side-badge-blue"] = { background = "#4f6bed1f", color = "#4f6bed" },
		[".side-badge-green"] = { background = "#22c55e24", color = "#16a34a" },
		[".side-flame"] = { color = "#f97316" },
		[".side-run"] = { padding = { 6, 8 }, radius = 8, gap = 10 },
		[".side-run:hover"] = { background = "list-hover" },
		[".side-run-tile"] = { radius = 4, color = "#ffffff" },
		[".side-star"] = { color = "#b4b4ba" },
		[".side-star-on"] = { color = "#f2b632" },
		[".side-plus"] = { background = "muted", radius = 5, padding = 2, color = "muted-foreground" },
		[".side-plus:hover"] = { color = "foreground" },
		[".side-usage"] = { background = "background", radius = 10, border = true, border_color = "border", padding = 10, gap = 4 },
		[".side-small"] = { size = "small", color = "muted-foreground" },
		[".side-upgrade"] = { background = "muted", radius = 7, padding = { 6, 0 } },
		[".side-upgrade:hover"] = { background = "#ebebed" },
		[".side-user"] = { padding = { 12, 6, 4, 6 }, gap = 10 },
	},

	init = function(previous: State?): State
		return previous or { selected = "dashboard", starred = {} }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.select then
			state.selected = msg.select
		elseif type(msg) == "table" and msg.star then
			state.starred[msg.star] = not state.starred[msg.star]
		end
	end,

	view = function(state: State, app)
		local window = app.window
		local mac = window.platform == "macos" and window.titlebar == "hidden" and not window.fullscreen

		local function page(item: Page, trailing: any?)
			local badge: any = nil
			if item.badge then
				badge = ui.row {
					class = { "side-badge", `side-badge-{item.tone or "blue"}` },
					hug = true,
					ui.text { item.badge },
				}
			elseif item.trailing then
				badge = ui.icon { name = item.trailing :: any, size = "small", class = "side-flame" }
			end
			return ui.row {
				class = "side-row",
				align = "center",
				selected = state.selected == item.id,
				msg = { select = item.id },
				tooltip = if item.id == "dashboard" then nil else "Only the dashboard is in this example",
				ui.icon { name = item.icon :: any, size = "small", class = "side-icon" },
				ui.text { item.label },
				ui.spacer(),
				badge,
				trailing,
			}
		end

		local rows: { any } = { heading("Workspaces") }
		for _, item in WORKSPACE do
			table.insert(rows, page(item))
		end
		table.insert(rows, heading("Agent Managements"))
		for _, item in AGENTS do
			table.insert(rows, page(item))
		end
		table.insert(rows, ui.row {
			class = "side-row",
			align = "center",
			command = "toggle-theme",
			tooltip = "Switch light and dark",
			ui.icon { name = "monitor", size = "small", class = "side-icon" },
			ui.text { "Appearance" },
			ui.spacer(),
			ui.icon { name = if app.dark then "moon" else "sun", size = "small", class = "side-icon" },
		})

		local runs: { any } = {}
		for _, run in RUNS do
			local on = state.starred[run.id] == true
			table.insert(runs, ui.row {
				class = "side-run",
				align = "center",
				ui.row {
					class = "side-run-tile",
					width = 16,
					height = 16,
					align = "center",
					justify = "center",
					shrink = false,
					style = { background = run.color },
					ui.icon { name = "workflow", size = "small" },
				},
				ui.row { grow = true, ui.text { run.label, truncate = true } },
				ui.row {
					msg = { star = run.id },
					tooltip = if on then "Unstar" else "Star",
					ui.icon { name = if on then "star-fill" else "star", size = "small", class = if on then "side-star-on" else "side-star" },
				},
			})
		end

		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "side-top",
				height = 56,
				shrink = false,
				align = "center",
				drag = true,
				ui.row { width = if mac then 78 else 12 },
				ui.row {
					class = "side-logo",
					width = 22,
					height = 22,
					align = "center",
					justify = "center",
					ui.icon { name = "workflow", size = "small" },
				},
				ui.text { "Cadence", class = "side-brand" },
				ui.spacer(),
				ui.row {
					class = "side-toggle",
					msg = "toggle-left",
					to = "shell",
					tooltip = "Hide the sidebar",
					ui.icon { name = "panel-left", size = "small" },
				},
			},
			ui.column {
				class = "side",
				grow = true,
				ui.column { gap = "none", shrink = false, table.unpack(rows) },
				ui.spacer(),
				heading("Workflow Runs", ui.row {
					class = "side-plus",
					tooltip = "New run",
					msg = "create",
					to = "dashboard",
					ui.icon { name = "plus", size = "small" },
				}),
				ui.column { gap = "none", shrink = false, table.unpack(runs) },
				ui.column {
					class = "side-usage",
					shrink = false,
					ui.row {
						align = "center",
						gap = "small",
						ui.icon { name = "zap", size = "small", class = "side-icon" },
						ui.text { "891/1000", weight = "medium" },
					},
					ui.text { "Upgrade for unlimited use", class = "side-small" },
					ui.row {
						class = "side-upgrade",
						justify = "center",
						msg = { select = "upgrade" },
						tooltip = "Not in this example",
						ui.text { "Upgrade", weight = "medium" },
					},
				},
				ui.row {
					class = "side-user",
					align = "center",
					shrink = false,
					avatar { name = "Mira Holt", initial = "MH", shape = "circle", color = "#3a3a40" },
					ui.column {
						gap = "none",
						grow = true,
						ui.text { "Mira Holt", weight = "medium" },
						ui.text { "mira@cadence.example", class = "side-small", truncate = true },
					},
					ui.icon { name = "chevron-down", size = "small", class = "side-icon" },
				},
			},
		}
	end,
}
