-- How values spread across a range, as filled curves, one per channel,
-- drawn over each other: a photo's red, green and blue, response times.
-- Markers stand on it at fractions of the range; with `drag`, each can be
-- dragged along it (levels: the shadow and highlight points).

type Channel = {
	values: { number },
	color: string?,
	name: string?,
}

type Marker = {
	id: string,
	value: number,
	color: string?,
	tooltip: string?,
}

type Props = {
	channels: { Channel },
	width: number?,
	height: number?,
	max: number?,
	log: boolean?,
	markers: { Marker }?,
	drag: any,
	shade: boolean?,
	to: string?,
}

-- A line keeps this much room inside its box, for its stroke and dots:
-- the stroke's width plus 3 pixels (see `ui.line`).
local STROKE = 1
local INSET = STROKE + 3
-- How wide a marker's handle is to grab, in pixels.
local GRIP = 12

return component {
	title = "Histogram",
	category = "Data",
	icon = "chart-column",
	description = "Filled curves of counts across a range, one per channel and overlaid in their colours, with optional log scale and markers that can be dragged along it.",
	when = "The spread of a picture's tones (red, green, blue, luminance), of latencies or of scores, where the shape matters more than any count; with draggable markers it is a levels control. For counts per labelled bucket use `bar-chart`; for a trend over time use `ui.line` or `series-chart`.",
	props = {
		["channels"] = "list of { values (counts, evenly spread from the range's start to its end), color?, name? }, drawn first to last",
		["width"] = "pixels (default 256)",
		["height"] = "pixels (default 96)",
		["max"] = "the count at the top (default the largest)",
		["log"] = "scale counts by their logarithm, so small ones still show",
		["markers"] = "list of { id, value (0 to 1 along the range), color?, tooltip? }: lines across it",
		["drag"] = "message a marker's drag sends, with `marker` (its id), `value` (0 to 1) and `phase` added; markers are fixed without it",
		["shade"] = "dim what lies outside the first and last marker",
		["to"] = "mod to send `drag` to, if not the drawing one",
	},
	styles = {
		[".histogram"] = { background = "muted-foreground/8", radius = 6, clip = true },
		[".histogram-marker"] = { background = "foreground" },
		[".histogram-shade"] = { background = "background/55" },
		[".histogram-grip"] = {},
		[".histogram-knob"] = { background = "foreground", radius = 999 },
	},
	example = {
		width = 256,
		height = 90,
		channels = {
			{ color = "#ef4444", values = { 2, 6, 14, 30, 52, 70, 64, 48, 40, 52, 66, 50, 30, 16, 8, 3 } },
			{ color = "#22c55e", values = { 1, 4, 10, 22, 40, 60, 74, 66, 50, 44, 46, 38, 24, 12, 5, 2 } },
			{ color = "#3b82f6", values = { 4, 10, 22, 40, 58, 62, 50, 40, 36, 34, 30, 22, 14, 8, 4, 1 } },
		},
		markers = { { id = "shadow", value = 0.08 }, { id = "highlight", value = 0.9 } },
		shade = true,
	},

	build = function(props: Props)
		local width = props.width or 256
		local height = props.height or 96
		local inner = width - 2 * INSET
		local scale: (number) -> number = if props.log then function(count: number): number
			return math.log(1 + math.max(count, 0))
		end else function(count: number): number
			return math.max(count, 0)
		end
		local top = 0
		if props.max then
			top = scale(props.max)
		else
			for _, channel in props.channels do
				for _, count in channel.values do
					top = math.max(top, scale(count))
				end
			end
		end
		if top <= 0 then
			top = 1
		end
		local layers = {}
		for index, channel in props.channels do
			local points = table.create(#channel.values)
			for at, count in channel.values do
				points[at] = scale(count)
			end
			local color = channel.color or "accent"
			layers[index] = ui.line {
				points = points,
				min = 0,
				max = top,
				area = true,
				stroke = STROKE,
				width = width,
				height = height,
				style = { color = color, background = `{color}/30` },
			}
		end
		local markers: { Marker } = props.markers or {}
		local overlays = {}
		-- What lies outside the first and last marker, dimmed.
		if props.shade and #markers >= 2 then
			local low = math.clamp(markers[1].value, 0, 1)
			local high = math.clamp(markers[#markers].value, 0, 1)
			table.insert(overlays, ui.row {
				class = "histogram-shade",
				float = { top = 0, left = 0 },
				width = INSET + low * inner,
				height = height,
			})
			table.insert(overlays, ui.row {
				class = "histogram-shade",
				float = { top = 0, right = 0 },
				width = INSET + (1 - high) * inner,
				height = height,
			})
		end
		for _, marker in markers do
			local x = INSET + math.clamp(marker.value, 0, 1) * inner
			local line_style = marker.color and { background = marker.color } or nil
			local draggable = props.drag ~= nil
			local msg = nil
			if draggable then
				msg = {}
				if typeof(props.drag) == "table" then
					for key, field in props.drag do
						msg[key] = field
					end
				else
					msg.msg = props.drag
				end
				msg.marker = marker.id
			end
			table.insert(overlays, ui.row {
				class = "histogram-grip",
				float = { top = 0, left = x - GRIP / 2 },
				width = GRIP,
				height = height,
				justify = "center",
				tooltip = marker.tooltip,
				pointer = msg,
				to = if draggable then props.to else nil,
				range = if draggable
					then { min = 0, max = 1, value = marker.value, speed = 1 / inner }
					else nil,
				cursor = if draggable then "ew-resize" else nil,
				ui.row { class = "histogram-marker", style = line_style, width = 1, height = height },
				draggable and ui.row {
					class = "histogram-knob",
					style = marker.color and { background = marker.color } or nil,
					width = 8,
					height = 8,
					float = { bottom = 2, left = GRIP / 2 - 4 },
				},
			})
		end
		local stacked = {}
		for index, layer in layers do
			stacked[index] = if index == 1 then layer else ui.row { float = "fill", layer }
		end
		for _, overlay in overlays do
			table.insert(stacked, overlay)
		end
		return ui.row {
			class = "histogram",
			width = width,
			height = height,
			table.unpack(stacked),
		}
	end,
}
