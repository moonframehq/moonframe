-- A small trend line against a threshold: the threshold drawn across,
-- dashed, with its label in a chip at the right end, the stretches that
-- breach it redrawn in the danger colour, and a solid dot on the latest
-- value. The box is a fixed size, so the pieces laid over the line (the
-- breaches, the dot, the chip) are placed with the line's own arithmetic.

type Props = {
	points: { number },
	threshold: number,
	label: string?,
	breach: string?,
	width: number?,
	height: number?,
	min: number?,
	max: number?,
	dot: boolean?,
	stroke: number?,
	tooltip: string?,
}

-- What `ui.line` keeps around its plot for the stroke and its dots.
local DOT = 3

return component {
	title = "Threshold Line",
	category = "Data",
	icon = "activity",
	description = "A trend line with a dashed threshold labelled in a chip, breaching stretches in red, and a dot on the latest value.",
	when = "A metric against its target or limit in a dashboard card: % within SLA against 95%, latency against 2s, lag against 5 minutes. For a line without a target use `ui.line`; for tiny bars use `sparkline`; for bars against a floor use `forecast-chart`.",
	props = {
		["points"] = "the values, oldest first, evenly spaced",
		["threshold"] = "the target or limit, drawn across dashed",
		["label"] = "the chip at the threshold's right end (\"95%\", \"2S\"); no chip without it",
		["breach"] = "\"above\" or \"below\": which side of the threshold is bad; those stretches turn danger (default none)",
		["width"] = "pixels, chip included (default 220)",
		["height"] = "pixels (default 56)",
		["min"] = "the value at the bottom (default the lowest point or threshold, with room)",
		["max"] = "the value at the top (default the highest point or threshold, with room)",
		["dot"] = "false hides the dot on the latest value",
		["stroke"] = "line width in pixels (default 1.5)",
		["tooltip"] = "text under the pointer",
	},
	styles = {
		[".threshold-line-stroke"] = { color = "success", border_color = "muted-foreground/45" },
		[".threshold-line-breach"] = { color = "danger" },
		[".threshold-line-dot"] = { background = "success", radius = 4 },
		[".threshold-line-dot-breach"] = { background = "danger" },
		[".threshold-line-chip"] = { background = "muted-foreground/20", radius = 4, padding = { 0, 4 } },
		[".threshold-line-chip-text"] = { size = "small", color = "muted-foreground", weight = "medium" },
	},
	example = {
		points = { 97.2, 96.9, 95.8, 95.7, 95.6, 96.4, 96.2, 96.6, 97.4 },
		threshold = 95,
		label = "95%",
		breach = "below",
		width = 320,
		height = 64,
	},

	build = function(props: Props)
		local points = props.points
		local width = props.width or 220
		local height = props.height or 56
		local stroke = props.stroke or 1.5
		local threshold = props.threshold
		local room = if props.label then 8 + 7 * #props.label + 4 else 0
		local plot = width - room

		local least, most = threshold, threshold
		for _, value in points do
			least = math.min(least, value)
			most = math.max(most, value)
		end
		local spread = math.max(most - least, 1e-6)
		local min = props.min or (least - spread * 0.1)
		local max = props.max or (most + spread * 0.1)
		local span = if max - min == 0 then 1 else max - min

		local pad = stroke + DOT
		local inner_w = math.max(1, plot - 2 * pad)
		local inner_h = math.max(1, height - 2 * pad)
		local steps = math.max(#points, 2) - 1
		local function x(index: number): number
			return pad + (index - 1) / steps * inner_w
		end
		local function y(value: number): number
			return pad + (1 - math.clamp((value - min) / span, 0, 1)) * inner_h
		end
		local function breaches(value: number): boolean
			if props.breach == "above" then
				return value > threshold
			elseif props.breach == "below" then
				return value < threshold
			end
			return false
		end

		local layers = {}
		-- Runs of segments with an end past the threshold, redrawn over it.
		local start: number? = nil
		for index = 1, #points do
			local bad = index < #points and (breaches(points[index]) or breaches(points[index + 1]))
			if bad and not start then
				start = index
			elseif not bad and start then
				local from = start
				local run = {}
				for at = from, index do
					table.insert(run, points[at])
				end
				local length = (index - from) / steps * inner_w
				table.insert(layers, ui.row {
					float = { top = 0, left = x(from) - pad },
					width = length + 2 * pad,
					height = height,
					ui.line { points = run, min = min, max = max, stroke = stroke, class = "threshold-line-breach" },
				})
				start = nil
			end
		end

		local last = points[#points]
		if props.dot ~= false and last then
			table.insert(layers, ui.row {
				class = if breaches(last) then { "threshold-line-dot", "threshold-line-dot-breach" } else "threshold-line-dot",
				float = { top = y(last) - 4, left = x(#points) - 4 },
				width = 8,
				height = 8,
			})
		end
		if props.label then
			table.insert(layers, ui.row {
				class = "threshold-line-chip",
				float = { top = y(threshold) - 9, left = plot + 6 },
				height = 18,
				align = "center",
				ui.text { props.label, class = "threshold-line-chip-text" },
			})
		end

		return ui.row {
			class = "threshold-line",
			width = width,
			height = height,
			shrink = false,
			gap = "none",
			tooltip = props.tooltip,
			ui.line {
				points = points,
				min = min,
				max = max,
				baseline = threshold,
				stroke = stroke,
				width = plot,
				height = height,
				class = "threshold-line-stroke",
			},
			table.unpack(layers),
		}
	end,
}
