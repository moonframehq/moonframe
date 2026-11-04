-- A picture's levels across its width, as an instrument draws them: each
-- column's darkest and brightest value per channel, overlaid, or side by
-- side as a parade. The engine measures; this only draws, at whatever rate
-- it publishes.

type Band = {
	low: { number },
	high: { number },
}

type Channel = {
	low: { number }?,
	high: { number }?,
	bands: { Band }?,
	color: string?,
	name: string?,
}

type Props = {
	channels: { Channel },
	parade: boolean?,
	width: number?,
	height: number?,
	lines: { number }?,
}

local LINES: { number } = { 0, 0.25, 0.5, 0.75, 1 }
-- The most points `ui.paths` takes in one line.
local MOST_POINTS = 200

return component {
	title = "Waveform",
	category = "Data",
	icon = "activity",
	description = "A picture's levels across its width: each column's lowest and highest value per channel, overlaid or as an RGB parade, over graticule lines.",
	when = "Setting exposure or light against a picture's levels, from values an engine measured and published (a few hundred columns). For the spread of tones with no position use `histogram`; for hue and saturation use `vectorscope`.",
	props = {
		["channels"] = "list of { low, high (each 0 to 1, one per column), color?, name? }; or { bands = list of { low, high } } for a real picture, each band a slice of the rows, as one range per column fills in the gaps between its tones",
		["parade"] = "side by side instead of overlaid",
		["width"] = "pixels (default 320)",
		["height"] = "pixels (default 140)",
		["lines"] = "levels for graticule lines, 0 to 1 (default 0, 0.25, 0.5, 0.75, 1)",
	},
	styles = {
		[".waveform"] = { background = "#0b0d10", radius = 6, clip = true },
	},
	example = {
		width = 280,
		height = 120,
		parade = true,
		channels = {
			{ color = "#ef4444", low = { 0.1, 0.2, 0.3, 0.2, 0.1, 0.15 }, high = { 0.5, 0.7, 0.8, 0.6, 0.4, 0.5 } },
			{ color = "#22c55e", low = { 0.2, 0.25, 0.35, 0.3, 0.2, 0.2 }, high = { 0.6, 0.75, 0.9, 0.7, 0.5, 0.6 } },
			{ color = "#3b82f6", low = { 0.05, 0.1, 0.2, 0.1, 0.05, 0.1 }, high = { 0.4, 0.5, 0.6, 0.4, 0.3, 0.35 } },
		},
	},

	build = function(props: Props)
		local width = props.width or 320
		local height = props.height or 140
		local paths = {}
		local lines: { number } = props.lines or LINES
		for _, level in lines do
			local y = (1 - level) * (height - 1)
			table.insert(paths, { points = { 0, y, width, y }, color = "#ffffff22", stroke = 1 })
		end
		local count = #props.channels
		for index, channel in props.channels do
			local left = if props.parade then (index - 1) * width / count else 0
			local span = if props.parade then width / count else width
			local color = channel.color or "#e5e7eb"
			-- A column's picture is rarely one range: a dark hill under a
			-- bright sky leaves a gap, which only bands can show.
			local bands: { Band } = channel.bands or { { low = channel.low or {}, high = channel.high or {} } }
			for _, band in bands do
				local columns = #band.low
				local points = {}
				for column = 1, columns do
					local x = left + (column - 0.5) * span / columns
					local top = (1 - math.clamp(band.high[column] or 0, 0, 1)) * height
					local bottom = (1 - math.clamp(band.low[column] or 0, 0, 1)) * height
					-- Down one column and up the next: one line for many columns.
					local first, second = top, bottom
					if column % 2 == 0 then
						first, second = bottom, top
					end
					table.insert(points, x)
					table.insert(points, first)
					table.insert(points, x)
					table.insert(points, second)
					if #points >= MOST_POINTS * 2 then
						table.insert(paths, { points = points, color = color, stroke = 1 })
						points = { x, second }
					end
				end
				if #points >= 4 then
					table.insert(paths, { points = points, color = color, stroke = 1 })
				end
			end
		end
		return ui.paths { class = "waveform", width = width, height = height, paths = paths }
	end,
}
