-- Emberline's shell: the sidebar on the left, and the report beside it
-- under a thin header naming the page. The traffic lights sit in the
-- sidebar's top row, before the workspace switcher.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Go", items = {
		{ label = "Leads Report", msg = { page = "leads" }, to = "sidebar", shortcut = "secondary-1" },
		{ label = "Customers", msg = { page = "customers" }, to = "sidebar", shortcut = "secondary-2" },
		{ label = "Companies", msg = { page = "companies" }, to = "sidebar", shortcut = "secondary-3" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		"-",
		{ label = "Last 3 Months", msg = { growth = "3m" }, to = "report" },
		{ label = "Last 6 Months", msg = { growth = "6m" }, to = "report" },
		{ label = "Last Year", msg = { growth = "1y" }, to = "report" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 20 }, size = { width = 1480, height = 1040 } },

	about = {
		version = "0.1",
		description = "A CRM's leads report: pipeline, revenue by month, leads by status, visits and deals. Sample data.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	-- `sidebar` tells the header whether the sidebar shows, so it can offer
	-- it back and leave room for the traffic lights.
	init = function(previous)
		local state = previous or { left = true }
		return state, { { to = "topic:sidebar", msg = state.left } }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
			return state, { { to = "topic:sidebar", msg = state.left } }
		end
	end,

	view = function(state)
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 248, min = 210, max = 340, fixed = true, visible = state.left, class = "frame", ui.region "left" },
			ui.pane {
				min = 720,
				class = if state.left then { "frame", "main-pane" } else "frame",
				ui.column {
					grow = true,
					gap = "none",
					ui.row { class = "topbar", height = 52, shrink = false, gap = "none", ui.mount "header" },
					ui.column { grow = true, ui.region "main" },
				},
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
