-- Pick a filter: the old page fades and folds away as the new one slides in.
local segmented = use "segmented"
local swap_view = use "swap-view"

local PAGES = {
	open = { "Fix pagination", "Cache MR pages", "Retry flaky test" },
	mine = { "Cache MR pages" },
	done = { "Rename the ledger", "Drop the old API" },
}

return mod {
	init = function(previous)
		return previous or { filter = "open" }
	end,

	update = function(state, msg)
		if typeof(msg) == "table" and msg.select then
			state.filter = msg.select
		end
	end,

	view = function(state)
		local rows = {}
		for i, title in PAGES[state.filter] do
			rows[i] = ui.item { title }
		end
		return ui.column {
			gap = "normal",
			segmented {
				selected = state.filter,
				options = {
					{ id = "open", label = "Open" },
					{ id = "mine", label = "Mine" },
					{ id = "done", label = "Done" },
				},
			},
			swap_view { key = state.filter, width = 240, table.unpack(rows) },
		}
	end,
}
