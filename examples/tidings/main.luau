-- Tidings's shell: one title row across the window (search and compose over
-- the sidebar, then history and the open tabs), and under it the sidebar on
-- the window's own tint beside the mail, which draws its white cards.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Mail", items = {
		{ label = "New Message", msg = "compose", to = "mail", shortcut = "c" },
		{ label = "Search", msg = "search", to = "mail", shortcut = "/" },
		"-",
		{ label = "Next Email", msg = "next", to = "mail", shortcut = "j" },
		{ label = "Previous Email", msg = "previous", to = "mail", shortcut = "k" },
		{ label = "Open Email", msg = "open", to = "mail", shortcut = "enter" },
		{ label = "Back to the List", msg = "back", to = "mail", shortcut = "u" },
		"-",
		{ label = "Mark as Done", msg = "done", to = "mail", shortcut = "e" },
		{ label = "Mark as Unread", msg = "unread", to = "mail", shortcut = "shift-u" },
	} },
	{ title = "View", items = {
		{ label = "List Beside the Email", msg = { layout = "split" }, to = "mail", shortcut = "secondary-1" },
		{ label = "One Line per Email", msg = { layout = "rows" }, to = "mail", shortcut = "secondary-2" },
		{ label = "Switch Layout", msg = "switch-layout", to = "mail", shortcut = "l" },
		"-",
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 15 }, size = { width = 1440, height = 920 } },

	about = {
		version = "0.1",
		description = "Mail in two layouts: the list beside the email, or one line per email. Shows sample data.",
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
		local body = ui.column {
			class = "frame",
			grow = true,
			gap = "none",
			ui.row {
				height = 44,
				shrink = false,
				gap = "none",
				ui.mount { "topbar", id = "bar", props = { left = state.left } },
			},
			ui.hsplit {
				id = "body",
				divider = false,
				ui.pane { size = 232, min = 200, max = 320, fixed = true, visible = state.left, ui.region "left" },
				ui.pane { ui.region "main" },
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
