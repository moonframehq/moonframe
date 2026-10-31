-- An empty worktree list: "New worktree" adds one and the list replaces the
-- empty state; "Clear" empties it again.
local empty_state = use "empty-state"

local NAMES = { "feat/multi-currency", "fix/rounding", "chore/deps" }

return mod {
	init = function(previous)
		return previous or { worktrees = {} :: { string } }
	end,

	update = function(state, msg)
		if msg == "new" then
			local name = NAMES[#state.worktrees % #NAMES + 1]
			table.insert(state.worktrees, name)
		elseif msg == "clear" then
			state.worktrees = {}
		end
	end,

	view = function(state)
		if #state.worktrees == 0 then
			return ui.column {
				width = 360,
				empty_state {
					icon = "inbox",
					title = "No worktrees",
					message = "Create one to work on two branches at once.",
					action = "New worktree",
					msg = "new",
				},
			}
		end
		local rows: { any } = {}
		for i, name in state.worktrees do
			rows[i] = ui.item { name, detail = `~/w/{name}` }
		end
		return ui.column {
			width = 360,
			gap = "small",
			ui.column { gap = "none", table.unpack(rows) },
			ui.row {
				ui.button { "New worktree", icon = "plus", small = true, variant = "outline", msg = "new" },
				ui.button { "Clear", small = true, variant = "ghost", msg = "clear" },
			},
		}
	end,
}
