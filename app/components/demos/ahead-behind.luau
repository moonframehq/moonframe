-- A branch's sync state: Commit adds one ahead, Fetch one behind, Push and
-- Pull clear a side; Synced shows a check when both are zero.
local ahead_behind = use "ahead-behind"
local switch = use "switch"

return mod {
	init = function(previous)
		return previous or { ahead = 2, behind = 1, synced = true }
	end,

	update = function(state, msg)
		if msg == "commit" then
			state.ahead += 1
		elseif msg == "fetch" then
			state.behind += 1
		elseif msg == "push" then
			state.ahead = 0
		elseif msg == "pull" then
			state.behind = 0
		elseif msg == "toggle-synced" then
			state.synced = not state.synced
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			ui.row {
				gap = "small",
				ui.button { "Commit", small = true, variant = "outline", msg = "commit" },
				ui.button { "Fetch", small = true, variant = "outline", msg = "fetch" },
				ui.button { "Push", icon = "arrow-up", small = true, variant = "ghost", msg = "push" },
				ui.button { "Pull", icon = "arrow-down", small = true, variant = "ghost", msg = "pull" },
			},
			ui.row { switch { on = state.synced, label = "Synced mark", small = true, msg = "toggle-synced" } },
			ui.row {
				gap = "small",
				align = "center",
				ui.icon "git-branch",
				ui.text { "feature/inspect", weight = "medium" },
				ahead_behind { ahead = state.ahead, behind = state.behind, synced = state.synced },
			},
		}
	end,
}
