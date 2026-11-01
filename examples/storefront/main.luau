-- Storefront's shell: one light-grey window. Across the top, the title bar
-- holds the traffic lights, the sidebar toggle, history arrows and the
-- browser-style tabs (mods/topbar). Under it, the settings sidebar and a
-- white sheet with the open tab's page.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "File", items = {
		{ label = "New Tab", msg = "new-tab", to = "topbar", shortcut = "secondary-t" },
		{ label = "Close Tab", msg = "close-current", to = "topbar", shortcut = "secondary-w" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		"-",
		{ label = "Back", msg = "back", to = "topbar", shortcut = "secondary-[" },
		{ label = "Forward", msg = "forward", to = "topbar", shortcut = "secondary-]" },
		"-",
		{ label = "Find in Audit Log", msg = "find", to = "audit", shortcut = "secondary-f" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 17 }, size = { width = 1440, height = 900 } },

	about = {
		version = "0.1",
		description = "A store's admin, opened at its audit log: filter, search, page and export every activity. Sample data.",
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
		local body = ui.column {
			class = "frame",
			grow = true,
			gap = "none",
			ui.row { height = 46, shrink = false, gap = "none", ui.region { name = "top", direction = "row" } },
			ui.hsplit {
				id = "body",
				divider = false,
				ui.pane { size = 214, min = 180, max = 320, fixed = true, visible = state.left, class = "frame", ui.region "left" },
				ui.pane { class = { "frame", "sheet-pane" }, ui.column { class = "sheet", grow = true, ui.region "main" } },
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		-- It sits at the title bar's right end, clear of the pagination under the log.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { top = 8, right = 16 }, ui.mount "feedback" } }
	end,
}
