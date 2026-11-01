-- Lantern's shell: the pages of projects down the left (with the night
-- scene at their foot), and the workspace beside them: a title row of
-- tabs, the grid of panes and an icon rail. The title bar is hidden; the
-- traffic lights sit in the sidebar's header.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Pane", items = {
		{ label = "Split Right", msg = { split = "right" }, to = "workspace", shortcut = "secondary-d" },
		{ label = "Split Down", msg = { split = "down" }, to = "workspace", shortcut = "secondary-shift-d" },
		{ label = "Zoom Pane", msg = "zoom", to = "workspace", shortcut = "secondary-shift-enter" },
		{ label = "Close Pane", msg = "close", to = "workspace", shortcut = "secondary-shift-w" },
		"-",
		{ label = "Show a Terminal", msg = { kind = "terminal" }, to = "workspace" },
		{ label = "Show Files", msg = { kind = "files" }, to = "workspace" },
		{ label = "Show Processes", msg = { kind = "processes" }, to = "workspace" },
		{ label = "Show Changes", msg = { kind = "changes" }, to = "workspace" },
	} },
	{ title = "View", items = {
		{ label = "New Tab", msg = "new-tab", to = "workspace", shortcut = "secondary-t" },
		{ label = "Previous Tab", msg = "previous", to = "workspace", shortcut = "secondary-alt-left" },
		{ label = "Next Tab", msg = "next", to = "workspace", shortcut = "secondary-alt-right" },
		"-",
		{ label = "Show or Hide Pages", msg = "toggle-left", shortcut = "secondary-b" },
		{ label = "Show or Hide the Rail", msg = "toggle-rail", to = "workspace" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 16, y = 18 }, size = { width = 1440, height = 920 } },

	about = {
		version = "0.1",
		description = "A Halloween terminal workspace: tabs of framed panes, pages of projects, and a pumpkin.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	styles = {
		[".lantern-side"] = { background = "sidebar", border = "right", border_color = "border" },
		[".lantern-main"] = { background = "gap" },
	},

	init = function(previous)
		return previous or { left = true }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
			return state, { { to = "workspace", msg = { sidebar = state.left } } }
		end
	end,

	view = function(state)
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 250, min = 250, max = 250, fixed = true, visible = state.left, class = "lantern-side", ui.region "left" },
			ui.pane { class = "lantern-main", ui.region "main" },
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
