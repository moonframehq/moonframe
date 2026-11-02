-- Sessions' shell: the sessions on the left, the chosen one's shell in a
-- card in the middle, and its changes on the right, under a hidden title
-- bar whose traffic lights sit over the sessions list. The side panes
-- fold from the mods' buttons, which message the shell; the shell tells
-- the session mod, so its header makes room for the traffic lights.

-- The menus after the app menu (About, Quit); "View" merges into the
-- host's. A typed local, so items with `msg` and `command` mix.
local MENUS: { { title: string, items: { any } } } = {
	{ title = "Session", items = {
		{ label = "Find a Session…", msg = "search", to = "sessions", shortcut = "secondary-f" },
		{ label = "Only Sessions with Changes", msg = "changed-only", to = "sessions" },
	} },
	{ title = "View", items = {
		{ label = "Changes", msg = { tab = "changes" }, to = "changes", shortcut = "secondary-1" },
		{ label = "Files", msg = { tab = "files" }, to = "changes", shortcut = "secondary-2" },
		"-",
		{ label = "Show or Hide Sessions", msg = "toggle-left", shortcut = "secondary-s" },
		{ label = "Show or Hide Changes", msg = "toggle-right", shortcut = "secondary-b" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 20 } },

	about = {
		version = "0.1",
		description = "Coding sessions across your repositories: a shell in each, and its changes.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous)
		return previous or { left = true, right = true }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-right" then
			state.right = not state.right
		else
			return
		end
		return state, { { to = "session", msg = { panes = { left = state.left, right = state.right } } } }
	end,

	view = function(state)
		local body = ui.hsplit {
			id = "body",
			-- The cards meet with no line; a handle shows on hover.
			divider = false,
			ui.pane { size = 300, min = 230, max = 440, fixed = true, visible = state.left, class = "side", ui.region "left" },
			ui.pane { class = "middle", ui.region "main" },
			ui.pane { size = 380, min = 260, max = 600, fixed = true, visible = state.right, class = "side", ui.region "right" },
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
