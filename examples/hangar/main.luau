-- Hangar's shell: the bays on the left, the chosen bay's terminal in the
-- middle, and on the right what changed there over the repository's history.
-- The side panes fold from the View menu.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Bay", items = {
		{ label = "New Bay…", msg = "add", to = "bays", shortcut = "secondary-n" },
		{ label = "Next Bay", msg = { step = 1 }, to = "bays", shortcut = "secondary-]" },
		{ label = "Previous Bay", msg = { step = -1 }, to = "bays", shortcut = "secondary-[" },
	} },
	{ title = "View", items = {
		{ label = "Show or Hide Bays", msg = "toggle-left", shortcut = "secondary-b" },
		{ label = "Show or Hide Changes", msg = "toggle-right", shortcut = "secondary-j" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 20 }, size = { width = 1440, height = 900 } },

	about = {
		version = "0.1",
		description = "Parallel agents on one repository, a worktree each.",
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
		return state
	end,

	view = function(state)
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 320, min = 240, max = 460, fixed = true, visible = state.left, class = "side", ui.region "left" },
			ui.pane { class = "middle", ui.region "main" },
			ui.pane {
				size = 420,
				min = 280,
				max = 640,
				fixed = true,
				visible = state.right,
				class = "side",
				ui.vsplit {
					id = "right",
					divider = false,
					ui.pane { ui.region "right" },
					ui.pane { size = 340, min = 160, fixed = true, ui.mount { "history", id = "graph", props = { compact = true } } },
				},
			},
		}
		return ui.column {
			grow = true,
			gap = "none",
			body,
			ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" },
		}
	end,
}
