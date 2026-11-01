-- Ledger's shell: a sidebar on the window's grey, and the overview on a
-- large white sheet beside it.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 18 }, size = { width = 1600, height = 1000 } },

	about = {
		version = "0.1",
		description = "Cash, commitments and funds at a glance. Shows sample data.",
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
			ui.pane { size = 290, min = 230, max = 380, fixed = true, visible = state.left, class = "frame", ui.region "left" },
			ui.pane { class = { "frame", "sheet-pane" }, ui.column { class = "sheet", grow = true, ui.region "main" } },
		}
	end,
}
