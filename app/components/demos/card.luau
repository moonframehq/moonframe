-- A worktrees card: Add checks out another worktree, × removes one, and
-- clicking a row selects it.
local card = use "card"

local CANDIDATES = { "fix/rounding", "chore/deps", "feat/export-csv", "docs/api" }

return mod {
	init = function(previous)
		return previous or { worktrees = { "main", "feat/multi-currency" }, selected = "main", next = 1 }
	end,

	update = function(state, msg)
		if msg == "add" then
			local name = CANDIDATES[state.next]
			if name then
				table.insert(state.worktrees, name)
				state.next += 1
				state.selected = name
			end
		elseif typeof(msg) == "table" then
			if msg.remove then
				local i = table.find(state.worktrees, msg.remove)
				if i then
					table.remove(state.worktrees, i)
				end
			elseif msg.open then
				state.selected = msg.open
			end
		end
	end,

	view = function(state)
		local rows = {}
		for i, name in state.worktrees do
			rows[i] = ui.row {
				align = "center",
				gap = "small",
				ui.column { grow = true, gap = "none", ui.item { name, detail = `~/w/{name}`, selected = name == state.selected, msg = { open = name } } },
				ui.button { icon = "close", small = true, variant = "ghost", msg = { remove = name } },
			}
		end
		if #rows == 0 then
			rows[1] = ui.text { "No worktrees.", size = "small", tone = "muted" }
		end
		return ui.column {
			width = 400,
			card {
				title = "Worktrees",
				subtitle = `{#state.worktrees} checked out`,
				actions = {
					ui.button { "Add", icon = "plus", small = true, variant = "ghost", msg = "add", disabled = CANDIDATES[state.next] == nil },
				},
				table.unpack(rows),
			},
		}
	end,
}
