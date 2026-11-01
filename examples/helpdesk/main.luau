-- Tandem's shell: three panes, as a help desk lays them out. The sidebar
-- (brand, workspace, inbox views, the person signed in) on a pale lavender,
-- the ticket list beside it, and the open ticket on the right.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Ticket", items = {
		{ label = "New Ticket", msg = "new", to = "tickets", shortcut = "secondary-n" },
		{ label = "Search…", msg = "focus", to = "sidebar", shortcut = "secondary-k" },
		"-",
		{ label = "Next Ticket", msg = { step = 1 }, to = "tickets", shortcut = "secondary-down" },
		{ label = "Previous Ticket", msg = { step = -1 }, to = "tickets", shortcut = "secondary-up" },
	} },
	{ title = "View", items = {
		{ label = "All Tickets", msg = { view = "all" }, to = "sidebar", shortcut = "secondary-1" },
		{ label = "All Open", msg = { view = "open" }, to = "sidebar", shortcut = "secondary-2" },
		{ label = "Assigned to Me", msg = { view = "mine" }, to = "sidebar", shortcut = "secondary-3" },
		{ label = "Waiting on Employee", msg = { view = "waiting" }, to = "sidebar", shortcut = "secondary-4" },
		"-",
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 18 }, size = { width = 1440, height = 1010 } },

	about = {
		version = "0.1",
		description = "An IT help desk inbox with an assistant that answers in chat and checks devices. Sample data.",
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
		local body = ui.hsplit {
			id = "body",
			ui.pane { size = 258, min = 220, max = 340, fixed = true, visible = state.left, class = "side-pane", ui.region "left" },
			ui.pane { size = 430, min = 360, max = 560, fixed = true, class = "list-pane", ui.region "list" },
			ui.pane { min = 520, class = "detail-pane", ui.region "detail" },
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
