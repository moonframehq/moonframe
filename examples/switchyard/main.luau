-- Switchyard's shell: worktrees on the left, the chats with the console
-- under them in a card in the middle, and the worktree's files, changes and
-- checks on the right, under a hidden title bar whose traffic lights sit
-- over the worktrees. The side panes and the console fold from the menus
-- and the mods' buttons, which message the shell.

local MENUS: { { title: string, items: { any } } } = {
	{ title = "Chat", items = {
		{ label = "New Chat", msg = "new-chat", to = "chat", shortcut = "secondary-t" },
		{ label = "Close Chat", msg = "close-chat", to = "chat", shortcut = "secondary-w" },
		"-",
		{ label = "Next Chat", msg = "next-chat", to = "chat", shortcut = "ctrl-tab" },
		{ label = "Previous Chat", msg = "previous-chat", to = "chat", shortcut = "ctrl-shift-tab" },
	} },
	{ title = "View", items = {
		{ label = "Files", msg = { tab = "files" }, to = "inspector", shortcut = "secondary-1" },
		{ label = "Changes", msg = { tab = "changes" }, to = "inspector", shortcut = "secondary-2" },
		{ label = "Checks", msg = { tab = "checks" }, to = "inspector", shortcut = "secondary-3" },
		"-",
		{ label = "Show or Hide Worktrees", msg = "toggle-left", shortcut = "secondary-s" },
		{ label = "Show or Hide Inspector", msg = "toggle-right", shortcut = "secondary-b" },
		{ label = "Show or Hide Console", msg = "toggle-bottom", shortcut = "secondary-j" },
	} },
	{ title = "Window", items = {
		{ label = "Minimize", command = "minimize", shortcut = "secondary-m" },
		{ label = "Zoom", command = "maximize" },
	} },
}

type State = { left: boolean, right: boolean, bottom: boolean }

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 18, y = 18 }, size = { width = 1500, height = 940 } },

	about = {
		version = "0.1",
		description = "A workspace for coding agents: chats, shell commands, a terminal and your worktrees' changes.",
		website = "https://github.com/moonframehq/moonframe",
		copyright = "MIT licensed",
	},

	menus = MENUS,

	init = function(previous: State?): State
		return previous or { left = true, right = true, bottom = true }
	end,

	update = function(state: State, msg)
		if msg == "toggle-left" then
			state.left = not state.left
		elseif msg == "toggle-right" then
			state.right = not state.right
		elseif msg == "toggle-bottom" then
			state.bottom = not state.bottom
		elseif msg == "show-bottom" then
			state.bottom = true
		end
	end,

	view = function(state: State)
		local middle = ui.column { grow = true, class = "card",
			ui.vsplit { id = "middle",
				ui.pane { min = 200, ui.mount "chat" },
				ui.pane { size = 240, min = 120, max = 600, fixed = true, visible = state.bottom, class = "console-pane", ui.mount "console" },
			},
		}
		local body = ui.hsplit {
			id = "body",
			divider = false,
			ui.pane { size = 290, min = 220, max = 440, fixed = true, visible = state.left, class = "side", ui.mount "workspaces" },
			ui.pane { class = "middle", middle },
			ui.pane { size = 360, min = 260, max = 600, fixed = true, visible = state.right, class = "side", ui.mount "inspector" },
		}
		return ui.column { grow = true, gap = "none", body,
			ui.row { class = "feedback-corner", float = { at = "bottom-right", inset = 20 }, ui.mount "feedback" } }
	end,
}
