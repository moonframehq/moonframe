-- Worktrees: click a row to select it; its details show underneath.
local list = use "list"

local ROWS: { any } = {
	{ id = "main", label = "main", detail = "~/code/app", badge = "3" },
	{ id = "feature", label = "feature/inspect", detail = "~/code/app-inspect", badge = "new" },
	{ id = "fix", label = "fix/menus", detail = "~/code/app-menus", badge = "1" },
	{ id = "docs", label = "docs/components", detail = "~/code/app-docs" },
}

return mod {
	init = function(previous)
		return previous or { selected = "main" }
	end,

	update = function(state, msg)
		if msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state)
		local current = ROWS[1]
		for _, row in ROWS do
			if row.id == state.selected then
				current = row
			end
		end
		return ui.column {
			gap = "normal",
			width = 300,
			list { selected = state.selected, rows = ROWS },
			ui.text { `{current.label} at {current.detail}`, tone = "muted", size = "small" },
		}
	end,
}
