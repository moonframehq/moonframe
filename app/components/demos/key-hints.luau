-- Footer key hints: switch between plain glyphs and bordered caps, and
-- choose how they are justified.
local key_hints = use "key-hints"
local segmented = use "segmented"
local switch = use "switch"

return mod {
	init = function(previous)
		return previous or { caps = false, justify = "start" }
	end,

	update = function(state, msg)
		if msg == "toggle-caps" then
			state.caps = not state.caps
		elseif type(msg) == "table" and msg.select then
			state.justify = msg.select
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "normal",
				align = "center",
				switch { on = state.caps, label = "Caps", small = true, msg = "toggle-caps" },
				segmented {
					selected = state.justify,
					small = true,
					options = {
						{ id = "start", label = "Start" },
						{ id = "center", label = "Center" },
						{ id = "between", label = "Between" },
					},
				},
			},
			key_hints {
				caps = state.caps,
				justify = state.justify,
				hints = {
					{ keys = "up down", label = "choose" },
					{ keys = "enter", label = "open" },
					{ keys = "cmd-enter", label = "open in new tab" },
					{ keys = "escape", label = "close" },
				},
			},
		}
	end,
}
