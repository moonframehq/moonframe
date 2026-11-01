-- Cadence's shell: a pale sidebar on the left; on the right the top bar
-- (where you are, search, Ask AI, the bell and its drawer) over the
-- dashboard on a soft grey.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Workflow", items = {
		{ label = "New Workflow", msg = "create", to = "dashboard", shortcut = "secondary-n" },
		{ label = "Search…", msg = "focus", to = "topbar", shortcut = "secondary-k" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide Notifications", msg = "toggle", to = "topbar", shortcut = "secondary-shift-b" },
		{ label = "Mark All Notifications Read", msg = "read-all", to = "topbar" },
		{ label = "Restore Sample Notifications", msg = "reset", to = "topbar" },
		"-",
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 22 }, size = { width = 1500, height = 960 } },

	about = {
		version = "0.1",
		description = "Workflow runs, success and tokens at a glance, with a notification drawer. Sample data.",
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
			ui.pane { size = 248, min = 210, max = 340, fixed = true, visible = state.left, class = "side-pane", ui.region "left" },
			ui.pane {
				class = "main-pane",
				ui.column {
					grow = true,
					gap = "none",
					ui.row { height = 56, shrink = false, gap = "none", ui.mount "topbar" },
					ui.column { grow = true, gap = "none", ui.region "main" },
				},
			},
		}
	end,
}
