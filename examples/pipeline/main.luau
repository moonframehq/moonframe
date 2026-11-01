-- Tally's shell: the sidebar on the window's own colour, and beside it the
-- top bar over one white card holding the board and, on the right, the
-- agent's panel (which folds away).

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Deal", items = {
		{ label = "New Deal", msg = "new-deal", to = "board", shortcut = "secondary-n" },
		{ label = "Search…", msg = "focus", to = "topbar", shortcut = "secondary-k" },
	} },
	{ title = "View", items = {
		{ label = "Board", msg = { view = "board" }, to = "board", shortcut = "secondary-1" },
		{ label = "List", msg = { view = "list" }, to = "board", shortcut = "secondary-2" },
		"-",
		{ label = "Show or Hide the Sidebar", msg = "toggle-left", shortcut = "secondary-s" },
		{ label = "Show or Hide the Agent", msg = "toggle-agent", shortcut = "secondary-j" },
		{ label = "New Chat", msg = "reset", to = "agent", shortcut = "secondary-shift-n" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 22 }, size = { width = 1760, height = 1080 } },

	about = {
		version = "0.1",
		description = "Deals on a board by stage, the quarter's figures, and a demo agent that reads them.",
		website = "https://github.com/penso/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous)
		return previous or { left = true, agent = true }
	end,

	update = function(state, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-agent" then
			state.agent = not state.agent
		end
	end,

	view = function(state)
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 224, min = 190, max = 360, fixed = true, visible = state.left, class = "frame", ui.region "left" },
			ui.pane {
				class = "frame",
				ui.column {
					grow = true,
					gap = "none",
					ui.row { height = 52, shrink = false, gap = "none", ui.region { name = "top", direction = "row" } },
					ui.column {
						class = "sheet",
						grow = true,
						ui.hsplit {
							id = "work",
							ui.pane { min = 480, ui.region "main" },
							ui.pane { size = 350, min = 280, max = 560, fixed = true, visible = state.agent, ui.region "right" },
						},
					},
				},
			},
		}
		-- Send feedback (the built-in mods/feedback): ask an agent to change this app.
		return ui.column { grow = true, gap = "none", body, ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
