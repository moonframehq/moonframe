-- Select some words: the bar shows over the field, and its buttons format them.
local selection_toolbar = use "selection-toolbar"

return mod {
	init = function(previous)
		return previous or { selected = 0 }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.msg == "moved" then
			state.selected = msg.selected
		end
	end,

	view = function(state)
		return ui.column {
			style = { padding = { 40, 0, 0, 0 } },
			selection_toolbar {
				field = "notes",
				selected = state.selected,
				ui.input {
					id = "notes",
					value = "Select some of these words.",
					multiline = true,
					select = "moved",
					grow = true,
				},
			},
		}
	end,
}
