-- Workbench's shell: two cards side by side, a terminal and an editor, and
-- the project's changes and files on the right, under a hidden title bar
-- whose traffic lights sit in the terminal card's header.

-- The menus after the app menu (About, Quit); "View" merges into the
-- host's. A typed local, so items with `msg` and `command` mix.
local MENUS: { { title: string, items: { any } } } = {
	{ title = "File", items = {
		{ label = "New Shell", msg = "new", to = "agent", shortcut = "secondary-t" },
		{ label = "Close File", msg = "close-current", to = "editor", shortcut = "secondary-w" },
	} },
	{ title = "View", items = {
		{ label = "Changes", msg = { tab = "changes" }, to = "files", shortcut = "secondary-1" },
		{ label = "Files", msg = { tab = "files" }, to = "files", shortcut = "secondary-2" },
		"-",
		{ label = "Show or Hide the Editor", msg = "toggle-editor", shortcut = "secondary-e" },
		{ label = "Show or Hide Files", msg = "toggle-right", shortcut = "secondary-b" },
		"-",
		{ label = "Split Right", msg = { split = "right" }, to = "agent", shortcut = "secondary-d" },
		{ label = "Split Below", msg = { split = "below" }, to = "agent", shortcut = "secondary-shift-d" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 22, y = 29 } },

	about = {
		version = "0.1",
		description = "An agent's terminal, an editor and the project's files, side by side.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous)
		return previous or { right = true, editor = true }
	end,

	update = function(state, msg)
		if msg == "toggle-right" then
			state.right = not state.right
		elseif msg == "toggle-editor" then
			state.editor = not state.editor
		end
	end,

	view = function(state)
		return ui.hsplit {
			id = "body",
			-- The cards meet with no line; a handle shows on hover.
			divider = false,
			ui.pane { size = 560, min = 320, class = "card-pane", ui.region "left" },
			ui.pane { min = 320, visible = state.editor, class = "card-pane", ui.region "main" },
			ui.pane { size = 340, min = 240, max = 560, fixed = true, visible = state.right, class = "side", ui.region "right" },
		}
	end,
}
