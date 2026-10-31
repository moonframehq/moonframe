-- Appearance settings: click a row to flip its switch. "Reduce motion"
-- is disabled until "Animations" is on.
local switch = use "switch"

return mod {
	init = function(previous)
		return previous or { follow = true, contrast = false, animations = true, reduce = false }
	end,

	update = function(state, msg)
		if msg.flip then
			state[msg.flip] = not state[msg.flip]
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			switch {
				label = "Follow system theme",
				description = "Switch between light and dark with macOS.",
				on = state.follow,
				msg = { flip = "follow" },
			},
			switch { label = "High contrast", on = state.contrast, msg = { flip = "contrast" } },
			switch { label = "Animations", on = state.animations, msg = { flip = "animations" } },
			switch {
				label = "Reduce motion",
				small = true,
				on = state.reduce,
				disabled = not state.animations,
				msg = { flip = "reduce" },
			},
		}
	end,
}
