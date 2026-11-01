-- Hearth's shell: a dark sidebar on the left (the traffic lights in its top
-- strip), and on the right a bar of breadcrumbs over the page. Everything
-- is set in the monospace font; the page title alone is sans.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Go", items = {
		{ label = "Analytics", msg = { tab = "analytics" }, to = "review", shortcut = "secondary-1" },
		{ label = "Pull Requests", msg = { tab = "pulls" }, to = "review", shortcut = "secondary-2" },
		{ label = "CI", msg = { tab = "ci" }, to = "review", shortcut = "secondary-3" },
		{ label = "Issues", msg = { tab = "issues" }, to = "review", shortcut = "secondary-4" },
		{ label = "Settings", msg = { tab = "settings" }, to = "review", shortcut = "secondary-5" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-b" },
		"-",
		{ label = "Last 7 Days", msg = { range = "7" }, to = "review" },
		{ label = "Last 30 Days", msg = { range = "30" }, to = "review" },
		{ label = "Last 90 Days", msg = { range = "90" }, to = "review" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 14 }, size = { width = 1480, height = 1000 } },

	about = {
		version = "0.1",
		description = "Code review analytics in an observability tool: pull requests reviewed, issues caught, CI and repositories. Sample data.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous)
		return previous or { left = true }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		end
	end,

	view = function(state)
		return ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 252, min = 210, max = 340, fixed = true, visible = state.left, class = { "frame-side", "app" }, ui.region "left" },
			ui.pane {
				min = 720,
				class = { "frame-main", "app" },
				ui.column {
					grow = true,
					gap = "none",
					-- Mounted rather than a region: a region row keeps each member to
					-- its own width, and the bar spans the page.
					ui.row { height = 61, shrink = false, gap = "none", ui.mount "topbar" },
					ui.region "main",
				},
			},
		}
	end,
}
