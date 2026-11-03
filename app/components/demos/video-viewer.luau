-- A live picture from an engine, with a crop frame the host draws in it.
-- Previews start nothing, so it shows where the frames would go here;
-- `examples/framebench` runs it against a real ring.
local video_viewer = use "video-viewer"

return mod {
	init = function(previous)
		return previous or { zoom = nil :: number?, center = { 0.5, 0.5 } }
	end,

	update = function(state, msg)
		if typeof(msg) == "table" and msg.viewed then
			state.zoom = msg.zoom
			state.center = msg.center
		end
	end,

	view = function(state)
		return video_viewer {
			source = "preview",
			width = 420,
			height = 280,
			controls = true,
			zoom = state.zoom,
			center = state.center,
			change = { viewed = true },
			guides = { { x = 0.1, y = 0.1, width = 0.8, height = 0.8, color = "#ffcc00" } },
		}
	end,
}
