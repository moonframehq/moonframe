-- Lines changed: the steppers set added and removed; the switches show
-- the files count and the zero sides.
local diff_counts = use "diff-counts"
local stepper = use "stepper"
local switch = use "switch"

return mod {
	init = function(previous)
		return previous or { added = 38, removed = 16, files = true, zero = false }
	end,

	update = function(state, msg)
		if msg == "toggle-files" then
			state.files = not state.files
		elseif msg == "toggle-zero" then
			state.zero = not state.zero
		elseif type(msg) == "table" and msg.added then
			state.added = math.max(0, state.added + msg.added)
		elseif type(msg) == "table" and msg.removed then
			state.removed = math.max(0, state.removed + msg.removed)
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			ui.row {
				gap = "large",
				stepper { label = "Added", value = state.added, min = 0, decrement = { added = -8 }, increment = { added = 8 } },
				stepper { label = "Removed", value = state.removed, min = 0, decrement = { removed = -4 }, increment = { removed = 4 } },
			},
			ui.row {
				gap = "normal",
				switch { on = state.files, label = "Files", small = true, msg = "toggle-files" },
				switch { on = state.zero, label = "Show zero", small = true, msg = "toggle-zero" },
			},
			ui.row {
				gap = "large",
				align = "center",
				diff_counts { added = state.added, removed = state.removed, files = if state.files then 3 else nil, zero = state.zero, size = "normal" },
				diff_counts { added = state.added, removed = state.removed, zero = state.zero },
			},
		}
	end,
}
