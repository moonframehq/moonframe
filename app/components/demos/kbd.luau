-- A shortcut sheet: switch between caps, small caps and plain glyphs.
local kbd = use "kbd"
local segmented = use "segmented"

local SHORTCUTS = {
	{ "Command palette", "cmd-shift-p" },
	{ "Save all", "cmd-k cmd-s" },
	{ "Next tab", "ctrl-tab" },
	{ "Close pane", "cmd-alt-w" },
	{ "Delete line", "cmd-backspace" },
	{ "Zoom out", "cmd--" },
}

return mod {
	init = function(previous)
		return previous or { look = "caps" }
	end,

	update = function(state, msg)
		if msg.select then
			state.look = msg.select
		end
	end,

	view = function(state)
		local rows = {}
		for _, shortcut in SHORTCUTS do
			table.insert(
				rows,
				ui.row {
					align = "center",
					ui.row { grow = true, ui.text { shortcut[1] } },
					kbd { keys = shortcut[2], small = state.look == "small", plain = state.look == "plain" },
				}
			)
		end
		return ui.column {
			gap = "normal",
			width = 300,
			segmented {
				small = true,
				selected = state.look,
				options = {
					{ id = "caps", label = "Caps" },
					{ id = "small", label = "Small" },
					{ id = "plain", label = "Plain" },
				},
			},
			ui.column { gap = "small", table.unpack(rows) },
			ui.row { gap = "small", align = "center", ui.text { "From a list:", tone = "muted", size = "small" }, kbd { keys = { "⌘", "K" } } },
		}
	end,
}
