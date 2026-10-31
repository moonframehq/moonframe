-- A workspace menu: click … to open it; a tile or row runs and closes it.
-- Teleport enables "Teleport back"; Close just closes the menu.
local action_tiles = use "action-tiles"

local LABELS = {
	worktree = "New worktree",
	["fan-out"] = "Fan out",
	rename = "Rename",
	teleport = "Teleport",
	["teleport-back"] = "Teleport back",
	checkpoints = "Checkpoints…",
	delete = "Delete worktree checkout",
}

return mod {
	init = function(previous)
		return previous or { open = true, teleported = false, last = nil }
	end,

	update = function(state, msg)
		if msg == "menu" then
			state.open = not state.open
		elseif msg == "dismiss" then
			state.open = false
		elseif typeof(msg) == "table" and msg.action then
			state.open = false
			if msg.action ~= "close" then
				state.last = LABELS[msg.action]
			end
			if msg.action == "teleport" then
				state.teleported = true
			elseif msg.action == "teleport-back" then
				state.teleported = false
			end
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			height = 420,
			ui.row {
				align = "center",
				ui.popover {
					open = state.open,
					dismiss = "dismiss",
					side = "bottom",
					align = "start",
					width = 344,
					content = action_tiles {
						title = "feat-multi-currency",
						subtitle = "feat/multi-currency",
						trailing = ui.badge { "working", tone = "warning" },
						tiles = {
							{ id = "worktree", icon = "plus", label = "New worktree", accent = true },
							{ id = "fan-out", icon = "git-compare", label = "Fan out" },
							{ id = "rename", icon = "pencil", label = "Rename" },
							{ id = "teleport", icon = "external-link", label = "Teleport", disabled = state.teleported },
							{ id = "teleport-back", icon = "arrow-down", label = "Teleport back", disabled = not state.teleported },
							{ id = "close", icon = "close", label = "Close" },
						} :: { any },
						rows = { { id = "checkpoints", icon = "history", label = "Checkpoints…", detail = "4" } },
						destructive = { id = "delete", icon = "trash-2", label = "Delete worktree checkout" },
					},
					ui.button { "feat-multi-currency", icon = "ellipsis", small = true, variant = "outline", msg = "menu" },
				},
				ui.text { if state.last then `Last action: {state.last}` else "Pick an action", size = "small", tone = "muted" },
			},
		}
	end,
}
