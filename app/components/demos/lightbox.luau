-- Open the picture over the window: pinch or ⌘-wheel to zoom, Escape to close.
local lightbox = use "lightbox"

local PICTURES = {
	{ src = "demos/scan.jpg", title = "Scan" },
}

return mod {
	init = function(previous)
		return previous or { open = false, zoom = nil :: number?, center = nil :: any }
	end,

	update = function(state, msg)
		if msg == "open" then
			state.open = true
		elseif msg == "close" then
			state.open = false
			state.zoom, state.center = nil, nil
		elseif type(msg) == "table" and msg.msg == "viewed" then
			state.zoom, state.center = msg.zoom, msg.center
		end
	end,

	view = function(state)
		return ui.column {
			ui.button { "Open the picture", msg = "open" },
			lightbox {
				open = state.open, dismiss = "close", pictures = PICTURES,
				zoom = state.zoom, center = state.center, change = "viewed",
			},
		}
	end,
}
