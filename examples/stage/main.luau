-- Stage's shell: the board fills the window, with no chrome but the
-- traffic lights, on a soft backdrop. View > Captions hides the tiles'
-- labels for clean stills.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "View", items = {
		{ label = "Show or Hide Captions", msg = "captions", to = "board", shortcut = "secondary-l" },
		{ label = "Backdrop: Mist", msg = { backdrop = "mist" }, to = "board" },
		{ label = "Backdrop: Dusk", msg = { backdrop = "dusk" }, to = "board" },
		{ label = "Backdrop: Paper", msg = { backdrop = "paper" }, to = "board" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 18 }, size = { width = 1720, height = 1120 } },

	about = {
		version = "0.1",
		description = "Moonframe's components on a bento board, with sample data, for stills.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	view = function()
		local body = ui.column { grow = true, gap = "none", ui.region "main" }
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
