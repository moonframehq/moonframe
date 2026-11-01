-- Vigil's shell: the sidebar on the window's own grey, and beside it one
-- darker sheet holding the overview. The traffic lights sit over the
-- sidebar's top. The Actions pill floats over the sheet's corner.

local kbd = use "kbd"

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Go", items = {
		{ label = "Overview", msg = { page = "overview" }, to = "sidebar", shortcut = "secondary-1" },
		{ label = "Workflows", msg = { page = "workflows" }, to = "sidebar", shortcut = "secondary-2" },
		{ label = "Runs & Logs", msg = { page = "runs" }, to = "sidebar", shortcut = "secondary-3" },
		{ label = "Analytics", msg = { page = "analytics" }, to = "sidebar", shortcut = "secondary-4" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		"-",
		{ label = "Last Hour", msg = { range = "1H" }, to = "overview" },
		{ label = "Last 24 Hours", msg = { range = "24H" }, to = "overview" },
		{ label = "Last 7 Days", msg = { range = "7D" }, to = "overview" },
		{ label = "Last 30 Days", msg = { range = "30D" }, to = "overview" },
		"-",
		{ label = "Live Feed", msg = { tab = "live" }, to = "overview" },
		{ label = "Upcoming Schedule", msg = { tab = "upcoming" }, to = "overview" },
		"-",
		{ label = "Actions…", msg = "actions", to = "overview", shortcut = "secondary-/" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 18 }, size = { width = 1500, height = 960 } },

	about = {
		version = "0.1",
		description = "Workflow operations at a glance: SLA, latency, lag, errors and a live feed of runs. Sample data, simulated live.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	styles = {
		[".actions"] = { background = "chip", radius = 18, padding = { 6, 6, 6, 14 }, gap = 10, shadow = "0 6px 18px #00000066" },
		[".actions:hover"] = { background = "chip-hover" },
		[".actions-label"] = { weight = "medium" },
	},

	init = function(previous)
		return previous or { left = true }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		end
	end,

	view = function(state)
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 262, min = 200, max = 340, fixed = true, visible = state.left, class = "frame", ui.region "left" },
			ui.pane {
				min = 900,
				class = "frame",
				ui.column {
					class = "sheet",
					grow = true,
					ui.region "main",
					ui.row {
						class = "actions",
						float = { at = "bottom-right", inset = 14 },
						align = "center",
						msg = "actions",
						to = "overview",
						tooltip = "Run an action on the selected run",
						ui.text { "Actions", class = "actions-label" },
						kbd { keys = { "/" }, small = true },
					},
				},
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
