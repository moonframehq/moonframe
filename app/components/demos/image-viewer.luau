-- A scan to inspect and crop: pinch or ⌘-scroll to zoom about the
-- pointer, scroll or drag to pan, double-click for 100 % and back. The
-- crop frame is drawn over the picture in fractions of it, so it stays on
-- the same part of the scan at any zoom; drag inside it to move it.
local image_viewer = use "image-viewer"

return mod {
	init = function(previous)
		return previous
			or {
				zoom = nil :: number?,
				center = { 0.5, 0.5 },
				crop = { 0.12, 0.18, 0.62, 0.6 },
				-- Where the frame was when a drag started.
				from = nil :: { number }?,
			}
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		if msg.viewed then
			state.zoom = msg.zoom
			state.center = msg.center
		elseif msg.crop then
			if msg.phase == "down" then
				state.from = { state.crop[1], state.crop[2] }
			elseif state.from then
				-- dx and dy are screen pixels; the frame is in fractions of
				-- the picture, as wide as `width` pixels on screen.
				local scale_x = msg.width / state.crop[3]
				local scale_y = msg.height / state.crop[4]
				state.crop[1] = math.clamp(state.from[1] + msg.dx / scale_x, 0, 1 - state.crop[3])
				state.crop[2] = math.clamp(state.from[2] + msg.dy / scale_y, 0, 1 - state.crop[4])
				if msg.phase == "up" then
					state.from = nil
				end
			end
		end
	end,

	view = function(state)
		local crop = state.crop
		return ui.column {
			width = 560,
			height = 360,
			image_viewer {
				-- Relative to the components folder, where the viewer is.
				src = "demos/scan.jpg",
				zoom = state.zoom,
				center = state.center,
				change = { viewed = true },
				controls = true,
				overlay = {
					ui.column {
						area = { left = crop[1], top = crop[2], width = crop[3], height = crop[4] },
						style = { border = "all", border_color = "#ffffff", background = "#ffffff14" },
						pointer = { crop = true },
						cursor = "move",
					},
				},
			},
		}
	end,
}
