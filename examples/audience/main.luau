-- Inkwell's shell: the sidebar on the window's own grey, and on the right a
-- bar of breadcrumbs over one white card holding the page. The traffic
-- lights sit in the sidebar's top row, beside the brand.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Go", items = {
		{ label = "Overview", msg = { page = "overview" }, to = "sidebar", shortcut = "secondary-1" },
		{ label = "Audience", msg = { page = "audience" }, to = "sidebar", shortcut = "secondary-2" },
		{ label = "Campaigns", msg = { page = "campaigns" }, to = "sidebar", shortcut = "secondary-3" },
		{ label = "Learn", msg = { page = "learn" }, to = "sidebar", shortcut = "secondary-4" },	} },
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		"-",
		{ label = "Last 7 Days", msg = { range = "7" }, to = "audience" },
		{ label = "Last 30 Days", msg = { range = "30" }, to = "audience" },
		{ label = "Last 90 Days", msg = { range = "90" }, to = "audience" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 22 }, size = { width = 1600, height = 1180 } },

	about = {
		version = "0.1",
		description = "A newsletter's audience at a glance: readers, growth, sources and reading habits, and Learn: a course, guides and live sessions. Sample data.",
		website = "https://github.com/moonframehq/moonframe",
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
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 252, min = 200, max = 360, fixed = true, visible = state.left, class = "frame", ui.region "left" },
			ui.pane {
				min = 640,
				class = "frame",
				ui.column {
					grow = true,
					gap = "none",
					ui.row { height = 56, shrink = false, gap = "none", ui.region { name = "top", direction = "row" } },
					ui.column { class = "page-card", grow = true, ui.region "main" },
				},
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
