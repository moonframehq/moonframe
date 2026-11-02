-- A picture to look at closely: it fits its space until the user zooms
-- (pinch, or ⌘/Ctrl with the wheel, about the pointer), the wheel and a
-- drag pan it, and a double-click goes between fitted and 100 %. The mod
-- keeps `zoom` and `center` and gets them back as they change, so its own
-- buttons can set them too. `overlay` draws over the picture in fractions
-- of it: a crop frame, picked spots, a grid.

type Props = {
	src: string,
	zoom: number?,
	center: { number }?,
	change: any,
	overlay: { any }?,
	controls: boolean?,
	min: number?,
	max: number?,
	width: number?,
	height: number?,
	id: string?,
}

type State = {
	natural: { number }?,
	fit: number?,
	-- Where the picture was when a drag started.
	start: { number }?,
}

-- How much a pixel of ⌘-wheel zooms, and each button press.
local WHEEL = 0.004
local STEP = 1.25

local function message(change: any, zoom: number?, center: { number }): any
	local msg = {}
	if typeof(change) == "table" then
		for key, field in change do
			msg[key] = field
		end
	else
		msg.msg = change
	end
	msg.zoom = zoom
	msg.center = center
	return msg
end

local function percent(zoom: number): string
	return `{math.floor(zoom * 100 + 0.5)}%`
end

return component {
	title = "Image Viewer",
	category = "Content",
	icon = "image",
	description = "A picture that fits its space and zooms about the pointer (pinch or ⌘-wheel), pans by wheel or drag, goes between fitted and 100 % on a double-click, with an optional zoom pill and things drawn over the picture.",
	when = "Looking at a picture closely: a scan, a photo, a screenshot, a render, with tools drawn over it (a crop frame, picked spots). The mod keeps `zoom` and `center`. For a picture at a fixed size use `ui.image`; for an avatar use `avatar`.",
	props = {
		["src"] = "the picture: an absolute or `~/` path (a file the user picked or dropped, the image cache, the app's folders), or one relative to the components folder, which draws it",
		["zoom"] = "screen pixels per picture pixel (1 is 100 %); nil fits it",
		["center"] = "the point of the picture in the middle, { x, y } fractions of it; the middle when unset",
		["change"] = "message sent as the view changes, with `zoom` (nil when fitted) and `center` added",
		["overlay"] = "elements drawn over the picture; their `area` is in fractions of the picture, a `pointer` there takes presses before panning does, and their messages reach the mod as they are",
		["controls"] = "a pill in the corner: zoom out, the zoom, zoom in, fit, 100 %",
		["min"] = "the least zoom (default a quarter of fitted)",
		["max"] = "the most zoom (default 32, 3200 %)",
		["width"] = "pixels; fills its parent when unset",
		["height"] = "pixels; fills its parent when unset",
		["id"] = "keeps its state apart from another viewer's in the same mod (default \"main\")",
	},
	styles = {
		[".image-viewer"] = { background = "muted-foreground/8", clip = true },
		[".image-viewer-controls"] = {
			background = "background/90",
			border = "all",
			border_color = "border",
			radius = 999,
			padding = { 2, 4 },
			gap = 2,
			margin = 10,
			shadow = "0 2px 8px #00000033",
		},
		[".image-viewer-button"] = { padding = { 3, 6 }, radius = 999, color = "muted-foreground" },
		[".image-viewer-button:hover"] = { background = "list-hover", color = "foreground" },
		[".image-viewer-zoom"] = { size = "small", font = "mono", color = "foreground", padding = { 0, 4 } },
	},

	example = { src = "demos/scan.jpg", width = 420, height = 280, controls = true },

	init = function(previous: State?): State
		return previous or {}
	end,

	update = function(state: State, msg: any, props: Props)
		-- What `overlay` sends is the mod's: it is drawn in here, so its
		-- messages arrive here first, and go on as they came.
		if typeof(msg) ~= "table" or not msg.view then
			return state, { { to = "parent", msg = msg } }
		end
		if msg.view == "measure" then
			state.natural = { msg.width, msg.height }
			state.fit = msg.fit
			return state
		end
		local natural, fit = state.natural, state.fit
		if not natural or not fit then
			return state
		end
		local zoom = props.zoom or fit
		local center = props.center or { 0.5, 0.5 }
		if props.zoom == nil then
			center = { 0.5, 0.5 }
		end
		local least = props.min or fit / 4
		local most = props.max or 32
		local function send(next_zoom: number?, next_center: { number })
			next_center = { math.clamp(next_center[1], 0, 1), math.clamp(next_center[2], 0, 1) }
			return state, { { to = "parent", msg = message(props.change, next_zoom, next_center) } }
		end
		-- Zooms to `next_zoom`, keeping the picture's point under (x, y)
		-- (fractions of the box) where it is.
		local function zoom_about(next_zoom: number, x: number, y: number, box: { number })
			next_zoom = math.clamp(next_zoom, least, most)
			local across = (x - 0.5) * box[1]
			local down = (y - 0.5) * box[2]
			local u = center[1] + across / (natural[1] * zoom)
			local v = center[2] + down / (natural[2] * zoom)
			return send(next_zoom, {
				u - across / (natural[1] * next_zoom),
				v - down / (natural[2] * next_zoom),
			})
		end
		if msg.view == "wheel" then
			local box = { msg.width, msg.height }
			if msg.zoom ~= 0 then
				return zoom_about(zoom * (1 + msg.zoom), msg.x, msg.y, box)
			end
			if msg.secondary or msg.ctrl then
				return zoom_about(zoom * math.exp(msg.dy * WHEEL), msg.x, msg.y, box)
			end
			if props.zoom == nil then
				return state
			end
			return send(zoom, {
				center[1] - msg.dx / (natural[1] * zoom),
				center[2] - msg.dy / (natural[2] * zoom),
			})
		elseif msg.view == "pan" then
			local box = { msg.width, msg.height }
			if msg.phase == "down" then
				state.start = center
				if msg.clicks == 2 then
					state.start = nil
					if props.zoom == nil then
						return zoom_about(1, msg.x, msg.y, box)
					end
					return send(nil, { 0.5, 0.5 })
				end
				return state
			end
			local start = state.start
			if msg.phase == "up" then
				state.start = nil
			end
			if not start or props.zoom == nil then
				return state
			end
			return send(zoom, {
				start[1] - msg.dx / (natural[1] * zoom),
				start[2] - msg.dy / (natural[2] * zoom),
			})
		elseif msg.view == "in" or msg.view == "out" then
			local by = if msg.view == "in" then STEP else 1 / STEP
			local next_zoom = math.clamp(zoom * by, least, most)
			return send(next_zoom, center)
		elseif msg.view == "fit" then
			return send(nil, { 0.5, 0.5 })
		elseif msg.view == "actual" then
			return send(1, center)
		end
		return state
	end,

	build = function(props: Props, kept: any)
		local state: State = kept or {}
		local zoom = props.zoom or state.fit
		local controls = if props.controls
			then ui.row {
				class = "image-viewer-controls",
				align = "center",
				float = "bottom-right",
				hug = true,
				ui.row { class = "image-viewer-button", msg = { view = "out" }, tooltip = "Zoom out", ui.icon "zoom-out" },
				ui.text { if zoom then percent(zoom) else "Fit", class = "image-viewer-zoom" },
				ui.row { class = "image-viewer-button", msg = { view = "in" }, tooltip = "Zoom in", ui.icon "zoom-in" },
				ui.row { class = "image-viewer-button", msg = { view = "fit" }, tooltip = "Fit", ui.icon "maximize" },
				ui.row { class = "image-viewer-button", msg = { view = "actual" }, tooltip = "100 %", ui.text "1:1" },
			}
			else nil
		return ui.column {
			class = "image-viewer",
			width = props.width,
			height = props.height,
			grow = props.width == nil and props.height == nil,
			wheel = { view = "wheel" },
			pointer = { view = "pan" },
			cursor = if props.zoom then "grab" else nil,
			ui.column {
				float = "fill",
				ui.image {
					src = props.src,
					zoom = props.zoom,
					center = props.center,
					measure = { view = "measure" },
					overlay = props.overlay,
				},
			},
			controls,
		}
	end,
}
