-- Live frames to look at closely: `ui.video` that fits its space until the
-- user zooms (pinch, or ⌘/Ctrl with the wheel), the
-- wheel and a drag pan it, a double-click goes between fitted and 100 %
-- and a press reads the pixel under it. The mod keeps `zoom` and `center`.
--
-- The picture is a native view: nothing can be drawn over it, so the
-- controls sit under it, and crop frames and guides are `guides`, which
-- the host draws in the picture.

type Props = {
	source: string,
	zoom: number?,
	center: { number }?,
	change: any,
	stats: any,
	probed: any,
	guides: { any }?,
	color: string?,
	extended: boolean?,
	black: number?,
	white: number?,
	gain: number?,
	gamma: number?,
	controls: boolean?,
	max: number?,
	width: number?,
	height: number?,
	id: string?,
}

type State = {
	natural: { number }?,
	fit: number?,
	density: number?,
	probe: { number }?,
	start: { number }?,
}

-- How far a pixel of wheel pans, against a pixel of drag.
local WHEEL = 1
local STEP = 1.25

local function with(message: any, fields: { [string]: any }): any
	local msg = {}
	if typeof(message) == "table" then
		for key, field in message do
			msg[key] = field
		end
	else
		msg.msg = message
	end
	for key, field in fields do
		msg[key] = field
	end
	return msg
end

return component {
	title = "Video Viewer",
	category = "Content",
	icon = "image",
	description = "Live frames from another process that fit their space and zoom, pan by wheel or drag, go between fitted and 100 % on a double-click, and read the pixel under a press, with the controls under the picture.",
	when = "Previewing a camera or scanner, or inspecting a full-resolution frame, with an engine writing a frame ring (`ui.video`). The mod keeps `zoom` and `center`. For a picture in a file use `image-viewer`.",
	props = {
		["source"] = "the frame ring's name, as the engine made it",
		["zoom"] = "device pixels per picture pixel (1 is 100 %); nil fits it",
		["center"] = "the point of the picture in the middle, { x, y } fractions of it",
		["change"] = "message sent as the view changes, with `zoom` (nil when fitted), `center` and `viewed = true` added",
		["stats"] = "message sent each second with `fps`, `shown`, `skipped`, `latency` (ms), `live`, `width`, `height`, `format`, `frame`, `fit` and `density` added",
		["probed"] = "message sent with `x`, `y`, `source`, `display` and on a Bayer frame `channel` added, for the pixel pressed",
		["guides"] = "rectangles and lines the host draws in the picture: { x, y, width, height, color } in fractions of it",
		["color"] = "what the pixels are: srgb, display-p3, rec709, rec2020, linear-srgb, linear-p3, or none to send them to the screen as they are",
		["extended"] = "lets values above 1 reach a screen that can show them",
		["black"] = "the code value shown as black",
		["white"] = "the code value shown as white (4095 for 12 bits in 16); the format's full range when unset",
		["gain"] = "multiplies the result",
		["gamma"] = "a display gamma to encode with; 1 changes nothing",
		["controls"] = "a row under the picture: zoom out, the zoom, zoom in, fit, 100 %",
		["max"] = "the most zoom (default 32)",
		["width"] = "pixels; fills its parent when unset",
		["height"] = "pixels; fills its parent when unset",
		["id"] = "keeps its state apart from another viewer's in the same mod (default \"main\")",
	},
	styles = {
		[".video-viewer"] = { background = "#000000" },
		[".video-viewer-controls"] = { padding = { 4, 8 }, gap = 2, background = "background" },
		[".video-viewer-button"] = { padding = { 2, 6 }, radius = 4, color = "muted-foreground" },
		[".video-viewer-button:hover"] = { background = "muted-foreground/15" },
		[".video-viewer-zoom"] = { size = "small", color = "muted-foreground" },
	},
	example = { source = "preview", width = 420, height = 280, controls = true },

	init = function(): State
		return {}
	end,

	update = function(state: State, msg: any, props: Props)
		if typeof(msg) ~= "table" then
			return state
		end
		if msg.view == "stats" then
			if msg.width then
				state.natural = { msg.width, msg.height }
				state.fit = msg.fit
				state.density = msg.density
			end
			if props.stats then
				return state, { { to = "parent", msg = with(props.stats, msg) } }
			end
			return state
		elseif msg.view == "probed" then
			if props.probed then
				return state, { { to = "parent", msg = with(props.probed, msg) } }
			end
			return state
		end
		local natural, fit = state.natural, state.fit
		if not natural or not fit then
			return state
		end
		local zoom = props.zoom or fit
		local center = props.center or { 0.5, 0.5 }
		-- The pointer moves in points, the picture in device pixels.
		local density = state.density or 1
		local most = props.max or 32
		local function send(next_zoom: number?, next_center: { number })
			next_center = { math.clamp(next_center[1], 0, 1), math.clamp(next_center[2], 0, 1) }
			if not props.change then
				return state
			end
			return state, { { to = "parent", msg = with(props.change, { zoom = next_zoom, center = next_center, viewed = true }) } }
		end
		if msg.view == "wheel" then
			if msg.zoom ~= 0 then
				return send(math.clamp(zoom * (1 + msg.zoom), fit / 4, most), center)
			end
			if not props.zoom then
				return state
			end
			return send(zoom, {
				center[1] + msg.dx * WHEEL * density / (natural[1] * zoom),
				center[2] + msg.dy * WHEEL * density / (natural[2] * zoom),
			})
		elseif msg.view == "pan" then
			if msg.phase == "down" then
				state.start = center
				state.probe = { msg.x, msg.y }
				if msg.clicks == 2 then
					state.start = nil
					if props.zoom == nil then
						return send(1, center)
					end
					return send(nil, { 0.5, 0.5 })
				end
				return state
			end
			local start = state.start
			if msg.phase == "up" then
				state.start = nil
			end
			if not start or not props.zoom then
				return state
			end
			return send(zoom, {
				start[1] - msg.dx * density / (natural[1] * zoom),
				start[2] - msg.dy * density / (natural[2] * zoom),
			})
		elseif msg.view == "in" or msg.view == "out" then
			local by = if msg.view == "in" then STEP else 1 / STEP
			return send(math.clamp(zoom * by, fit / 4, most), center)
		elseif msg.view == "fit" then
			return send(nil, { 0.5, 0.5 })
		elseif msg.view == "actual" then
			return send(1, center)
		end
		return state
	end,

	build = function(props: Props, kept: any)
		local state: State = kept or {}
		local zoom = props.zoom
		local controls = if props.controls
			then ui.row {
				class = "video-viewer-controls",
				align = "center",
				ui.row { class = "video-viewer-button", msg = { view = "out" }, tooltip = "Zoom out", ui.icon "zoom-out" },
				ui.text { if zoom then `{math.floor(zoom * 100 + 0.5)}%` else "Fit", class = "video-viewer-zoom" },
				ui.row { class = "video-viewer-button", msg = { view = "in" }, tooltip = "Zoom in", ui.icon "zoom-in" },
				ui.row { class = "video-viewer-button", msg = { view = "fit" }, tooltip = "Fit", ui.icon "maximize" },
				ui.row { class = "video-viewer-button", msg = { view = "actual" }, tooltip = "100 %", ui.text "1:1" },
			}
			else nil
		return ui.column {
			width = props.width,
			height = props.height,
			grow = props.width == nil and props.height == nil,
			ui.column {
				class = "video-viewer",
				grow = true,
				wheel = { view = "wheel" },
				pointer = { view = "pan" },
				cursor = if zoom then "grab" else nil,
				ui.video {
					id = props.id,
					source = props.source,
					zoom = zoom,
					center = props.center,
					color = props.color :: any,
					extended = props.extended,
					black = props.black,
					white = props.white,
					gain = props.gain,
					gamma = props.gamma,
					guides = props.guides,
					probe = state.probe,
					stats = { view = "stats" },
					probed = { view = "probed" },
				},
			},
			controls,
		}
	end,
}
