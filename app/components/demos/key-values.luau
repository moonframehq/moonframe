-- Branch details: click a branch to show its properties.
local key_values = use "key-values"

local BRANCHES: { any } = {
	{
		name = "main",
		rows = { { "Branch", "main" }, { "Ahead", "2 commits" }, { "Changed", "14 files" }, { "Agent", "none" } },
	},
	{
		name = "fix/menus",
		rows = { { "Branch", "fix/menus" }, { "Ahead", "5 commits" }, { "Changed", "3 files" }, { "Agent", "Claude, working" } },
	},
	{
		name = "feature/inspect",
		rows = { { "Branch", "feature/inspect" }, { "Ahead", "12 commits" }, { "Changed", "41 files" }, { "Agent", "Codex, blocked" } },
	},
}

return mod {
	init = function(previous)
		return previous or { selected = 1 }
	end,

	update = function(state, msg)
		if msg.branch then
			state.selected = msg.branch
		end
	end,

	view = function(state)
		local items = {}
		for index, branch in BRANCHES do
			table.insert(items, ui.item { branch.name, selected = index == state.selected, msg = { branch = index } })
		end
		return ui.row {
			gap = "large",
			ui.column { gap = "none", width = 160, table.unpack(items) },
			key_values { width = 70, rows = BRANCHES[state.selected].rows },
		}
	end,
}
