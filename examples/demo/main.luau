-- The demo gallery's shell: the gallery fills the window, under the
-- traffic lights.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Demos", items = {
		{ label = "Open All", msg = { open_all = true }, to = "provider:demos", shortcut = "secondary-shift-o" },
		{ label = "Close All", msg = { close_all = true }, to = "provider:demos" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 26 }, size = { width = 1340, height = 900 } },

	about = {
		version = "0.1",
		description = "Every Moonframe example app in one place, to open, compare and fix.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	view = function()
		local body = ui.column { grow = true, gap = "none", ui.region "main" }
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
