-- Actions over a branch list: New adds a branch, Refresh re-sorts the
-- list, Remove deletes the last branch.
local toolbar = use "toolbar"

return mod {
	init = function(previous)
		return previous or { branches = { "main", "fix/menus", "feature/inspect" }, next = 1 }
	end,

	update = function(state, msg)
		if msg == "new" then
			table.insert(state.branches, `agent/task-{state.next}`)
			state.next += 1
		elseif msg == "refresh" then
			table.sort(state.branches)
		elseif msg == "remove" then
			table.remove(state.branches)
		end
	end,

	view = function(state)
		local rows = {}
		for _, branch in state.branches do
			table.insert(rows, ui.item { branch, detail = "~/code/ledger-api" })
		end
		return ui.column {
			gap = "normal",
			width = 300,
			toolbar {
				buttons = {
					{ label = "New", msg = "new" },
					{ label = "Refresh", msg = "refresh" },
					{ label = "Remove", msg = "remove", variant = "danger" },
				},
			},
			ui.column { gap = "none", table.unpack(rows) },
		}
	end,
}
