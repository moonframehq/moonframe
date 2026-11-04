-- Hue and saturation as an instrument draws them: a picture's chroma as
-- dots in a circle, with the primaries marked. The engine measures; this
-- only draws, at whatever rate it publishes.

type Props = {
	points: { number },
	size: number?,
	color: string?,
	targets: boolean?,
}

-- The primaries and secondaries' places, in fractions of the radius, for
-- Rec. 709 at 75 %.
local TARGETS = {
	{ x = 0.15, y = 0.62, color = "#ef4444" },
	{ x = -0.46, y = 0.37, color = "#eab308" },
	{ x = -0.31, y = -0.25, color = "#22c55e" },
	{ x = -0.15, y = -0.62, color = "#22d3ee" },
	{ x = 0.46, y = -0.37, color = "#3b82f6" },
	{ x = 0.31, y = 0.25, color = "#d946ef" },
}
local RING = 48
-- Each dot is a line of its own, and `ui.paths` takes 500 with the marks.
local MOST_DOTS = 480
local MARK = 4

return component {
	title = "Vectorscope",
	category = "Data",
	icon = "circle-dot",
	description = "A picture's chroma as dots in a circle, with a skin-tone line and the primaries and secondaries marked.",
	when = "Judging colour balance and saturation from points an engine measured and published (up to 480, thinned out; more are left out). For levels across the picture use `waveform`; for tones use `histogram`.",
	props = {
		["points"] = "flat list x1, y1, x2, y2, ...: Cb and Cr, each -1 to 1",
		["size"] = "pixels across (default 200)",
		["color"] = "the dots' colour (default a soft green)",
		["targets"] = "false hides the marks for the primaries and secondaries",
	},
	styles = {
		[".vectorscope"] = { background = "#0b0d10", radius = 6, clip = true },
	},
	example = {
		size = 180,
		points = { 0, 0, 0.1, 0.2, 0.2, 0.3, 0.1, 0.1, -0.1, -0.15, -0.2, -0.1, -0.1, 0.05, 0, 0 },
	},

	build = function(props: Props)
		local size = props.size or 200
		local middle = size / 2
		local radius = middle - 4
		local function at(x: number, y: number): (number, number)
			return middle + x * radius, middle - y * radius
		end
		local ring = table.create(RING * 2 + 2)
		for step = 0, RING do
			local angle = step / RING * 2 * math.pi
			local x, y = at(math.cos(angle), math.sin(angle))
			table.insert(ring, x)
			table.insert(ring, y)
		end
		local left, centre_y = at(-1, 0)
		local right = at(1, 0)
		local centre_x, top = at(0, 1)
		local _, bottom = at(0, -1)
		-- The skin-tone line, 123 degrees round from the blue axis.
		local skin_x, skin_y = at(-0.54, 0.84)
		local paths = {
			{ points = ring, color = "#ffffff33", stroke = 1 },
			{ points = { left, centre_y, right, centre_y }, color = "#ffffff1f", stroke = 1 },
			{ points = { centre_x, top, centre_x, bottom }, color = "#ffffff1f", stroke = 1 },
			{ points = { middle, middle, skin_x, skin_y }, color = "#f59e0b55", stroke = 1 },
		}
		if props.targets ~= false then
			for _, target in TARGETS do
				local x, y = at(target.x, target.y)
				table.insert(paths, {
					points = { x - MARK, y - MARK, x + MARK, y - MARK, x + MARK, y + MARK, x - MARK, y + MARK, x - MARK, y - MARK },
					color = target.color,
					stroke = 1,
				})
			end
		end
		local color = props.color or "#86efac"
		-- Dots: neighbours in the list are not neighbours in the picture, so
		-- a line between them would mean nothing.
		for index = 1, math.min(#props.points - 1, MOST_DOTS * 2), 2 do
			local x, y = at(props.points[index], props.points[index + 1])
			table.insert(paths, { points = { x - 1, y, x + 1, y }, color = color, stroke = 2 })
		end
		return ui.paths { class = "vectorscope", width = size, height = size, paths = paths }
	end,
}
