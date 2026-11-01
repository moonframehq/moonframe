-- Gantry's shell: one dark, square-cornered window. The top bar runs across
-- it (org, the sections, notifications, the user), and under it the page
-- the console mod routes to: the services, one service's details, or the
-- other sections.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Go", items = {
		{ label = "Home", msg = { go = "home" }, to = "console", shortcut = "secondary-1" },
		{ label = "Services", msg = { go = "services" }, to = "console", shortcut = "secondary-2" },
		{ label = "Tasks", msg = { go = "tasks" }, to = "console", shortcut = "secondary-3" },
		{ label = "Docs", msg = { go = "docs" }, to = "console", shortcut = "secondary-4" },
		{ label = "Status", msg = { go = "status" }, to = "console", shortcut = "secondary-5" },
		{ label = "Changelog", msg = { go = "changelog" }, to = "console", shortcut = "secondary-6" },
	} },
	{ title = "Service", items = {
		{ label = "Live Logs", msg = "logs", to = "service", shortcut = "secondary-l" },
		{ label = "New Deploy", msg = "deploy", to = "service", shortcut = "secondary-shift-d" },
		"-",
		{ label = "Overview", msg = { tab = "overview" }, to = "service", shortcut = "secondary-alt-1" },
		{ label = "Resource", msg = { tab = "resource" }, to = "service", shortcut = "secondary-alt-2" },
		{ label = "Analytics", msg = { tab = "analytics" }, to = "service", shortcut = "secondary-alt-3" },
		{ label = "Settings", msg = { tab = "settings" }, to = "service", shortcut = "secondary-alt-4" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 22 }, size = { width = 1440, height = 980 } },

	about = {
		version = "0.1",
		description = "An infra console: services, their resources, usage and alerts. Shows simulated sample telemetry.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	view = function()
		return ui.column {
			grow = true,
			gap = "none",
			class = "window",
			ui.row { class = "topline", height = 56, shrink = false, gap = "none", ui.region { name = "top", direction = "row" } },
			ui.column { grow = true, gap = "none", ui.region { name = "main", gap = "none" } },
		}
	end,
}
