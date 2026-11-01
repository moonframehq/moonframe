-- Several series as lines over a value axis and a time axis, with a
-- legend. Moving the pointer along the plot draws a hairline at the nearest
-- sample with its values in a tooltip; a click pins a dashed crosshair there,
-- with a dot on each line and a card of the values. The plot has a pixel
-- size, because a view cannot measure where the samples fall.

type Series = { name: string, color: string, points: { number } }
type Label = { index: number, text: string }

type Props = {
	series: { Series },
	width: number?,
	height: number?,
	min: number?,
	max: number?,
	ticks: { string }?,
	labels: { Label }?,
	titles: { string }?,
	unit: string?,
	pinned: number?,
	event: string?,
	to: string?,
	legend: boolean?,
}

local AXIS = 40

return component {
	title = "Series Chart",
	category = "Data",
	icon = "activity",
	description = "Lines for several series over a value and a time axis, with a legend, a hover hairline with values, and a pinned dashed crosshair with a card.",
	when = "Several readings over the same period compared on one scale (CPU, GPU and memory over 24 hours; requests and errors by hour), where the reader wants a sample's exact values. For counts stacked per bucket use `bar-chart`; for one small trend use `ui.line` or `sparkline`; for a balance against a floor use `forecast-chart`.",
	props = {
		["series"] = "list of { name, color, points }: the same number of points in each, oldest first",
		["width"] = "the plot's width in pixels (default 640); the axis adds 50",
		["height"] = "the plot's height in pixels (default 180)",
		["min"] = "the value at the bottom (default 0)",
		["max"] = "the value at the top (default 100)",
		["ticks"] = "value-axis labels from the top down, one grid line each (default 100%, 75%, 50%, 25%, 0%)",
		["labels"] = "time-axis labels: { index (the sample, from 1), text }",
		["titles"] = "a heading per sample for the tooltip and the card (\"TODAY, 06:00\")",
		["unit"] = "after each value (default \"%\")",
		["pinned"] = "the sample the crosshair is pinned to; none when unset",
		["event"] = "key of the message a click on the plot sends, default \"pin\": { pin = index }",
		["to"] = "mod to send it to, if not the drawing one",
		["legend"] = "false hides the legend",
	},
	styles = {
		[".series-chart"] = { gap = 10 },
		[".series-chart-legend"] = { gap = 18 },
		[".series-chart-key"] = { gap = 6, size = "small" },
		[".series-chart-tick"] = { color = "muted-foreground", size = "small" },
		[".series-chart-grid"] = { background = "border/70" },
		[".series-chart-hit"] = { border_color = "#00000000" },
		[".series-chart-hit:hover"] = { border_color = "muted-foreground/70" },
		[".series-chart-cross"] = { gap = 3 },
		[".series-chart-dash"] = { background = "muted-foreground/80" },
		[".series-chart-dot"] = { radius = 999, border = true, background = "background" },
		[".series-chart-tip"] = {
			background = "card",
			border = true,
			border_color = "border",
			padding = { 8, 10 },
			gap = 6,
			shadow = "0 6px 18px #00000055",
		},
		[".series-chart-tip-title"] = { color = "muted-foreground", size = "small" },
		[".series-chart-tip-row"] = { gap = 6, size = "small" },
	},
	example = {
		width = 420,
		height = 140,
		pinned = 7,
		series = {
			{ name = "CPU", color = "#3b82f6", points = { 52, 55, 54, 58, 56, 53, 57, 60, 59, 61 } },
			{ name = "GPU", color = "#22c55e", points = { 34, 35, 38, 42, 50, 61, 70, 72, 76, 74 } },
			{ name = "Memory", color = "#f97316", points = { 27, 28, 26, 25, 24, 23, 21, 22, 20, 21 } },
		},
		labels = { { index = 1, text = "12:00" }, { index = 4, text = "18:00" }, { index = 7, text = "00:00" }, { index = 10, text = "NOW" } },
	},
	usage = [[
local series_chart = use "series-chart"

series_chart {
	width = 640, height = 180, pinned = state.pinned, titles = app.metrics.times,
	series = {
		{ name = "CPU", color = "#3b82f6", points = history.cpu },
		{ name = "GPU", color = "#22c55e", points = history.gpu },
	},
	labels = { { index = 1, text = "12:00" }, { index = 97, text = "NOW" } },
}
-- update: msg.pin is the clicked sample's index; keep it in state.pinned
]],

	build = function(props: Props)
		local width = props.width or 640
		local height = props.height or 180
		local low = props.min or 0
		local high = props.max or 100
		local span = if high > low then high - low else 1
		local unit = props.unit or "%"
		local series: { Series } = props.series or {}
		local count = 0
		for _, line in series do
			count = math.max(count, #line.points)
		end
		local step = if count > 1 then width / (count - 1) else width
		local function x_of(index: number): number
			return math.round((index - 1) * step)
		end
		local function y_of(value: number): number
			return math.round(height - (math.clamp(value, low, high) - low) / span * height)
		end
		local function reading(value: number?): string
			return if value then `{math.round(value)}{unit}` else "–"
		end

		local ticks: { string } = props.ticks or { "100%", "75%", "50%", "25%", "0%" }
		local layers: { any } = {}
		local axis: { any } = {}
		for index, text in ticks do
			local y = if #ticks > 1 then math.round((index - 1) * (height - 1) / (#ticks - 1)) else 0
			table.insert(layers, ui.row { class = "series-chart-grid", width = width, height = 1, float = { top = y, left = 0 } })
			table.insert(axis, ui.row {
				float = { top = y - 8, left = 0 },
				width = AXIS - 6,
				height = 16,
				justify = "end",
				align = "center",
				ui.text { text, class = "series-chart-tick", wrap = false },
			})
		end

		for _, line in series do
			table.insert(layers, ui.column {
				float = { top = 0, left = 0 },
				width = width,
				height = height,
				ui.line { points = line.points, min = low, max = high, stroke = 1.5, width = width, height = height, style = { color = line.color } },
			})
		end

		-- The pinned sample: a dashed hairline, a ring on each line, a card.
		local pinned = props.pinned
		if pinned and pinned >= 1 and pinned <= count then
			local x = x_of(pinned)
			local dashes = table.create(math.floor(height / 6))
			for index = 1, math.floor(height / 6) do
				dashes[index] = ui.row { class = "series-chart-dash", width = 1, height = 3 }
			end
			table.insert(layers, ui.column { class = "series-chart-cross", float = { top = 0, left = x }, width = 1, height = height, table.unpack(dashes) })
			local rows: { any } = {}
			for _, line in series do
				local value = line.points[pinned]
				if value then
					table.insert(layers, ui.row {
						class = "series-chart-dot",
						width = 7,
						height = 7,
						float = { top = y_of(value) - 3, left = x - 3 },
						style = { border_color = line.color },
					})
				end
				table.insert(rows, ui.row {
					class = "series-chart-tip-row",
					align = "center",
					ui.row { width = 7, height = 7, style = { background = line.color } },
					ui.text { string.upper(line.name), wrap = false },
					ui.spacer(),
					ui.text { reading(value), wrap = false },
				})
			end
			local card_width = 144
			local left = if x + 14 + card_width > width then x - 14 - card_width else x + 14
			table.insert(layers, ui.column {
				class = "series-chart-tip",
				width = card_width,
				float = { top = math.round(height * 0.24), left = left },
				props.titles and ui.text { props.titles[pinned] or "", class = "series-chart-tip-title", wrap = false },
				table.unpack(rows),
			})
		end

		-- What takes the pointer: two half-columns per sample, meeting at it,
		-- each drawing the hairline on the side where the sample is.
		if count > 0 then
			local half = step / 2
			local hits = table.create(count * 2)
			local event = props.event or "pin"
			for index = 1, count do
				local lines = { if props.titles then props.titles[index] or "" else `#{index}` }
				for _, line in series do
					table.insert(lines, `{string.upper(line.name)}  {reading(line.points[index])}`)
				end
				local tip = table.concat(lines, "\n")
				local msg = { [event] = index }
				table.insert(hits, ui.column { class = "series-chart-hit", width = half, height = height, border = "right", tooltip = tip, msg = msg, to = props.to })
				table.insert(hits, ui.column { class = "series-chart-hit", width = half, height = height, border = "left", tooltip = tip, msg = msg, to = props.to })
			end
			table.insert(layers, ui.row {
				float = { top = 0, left = -math.round(half) },
				width = width + step,
				height = height,
				gap = "none",
				table.unpack(hits),
			})
		end

		local labels: { any } = {}
		local labels_in: { Label } = props.labels or {}
		for _, label in labels_in do
			table.insert(labels, ui.row {
				float = { top = 8, left = x_of(label.index) - 28 },
				width = 56,
				justify = "center",
				ui.text { label.text, class = "series-chart-tick", wrap = false },
			})
		end

		local legend: { any } = {}
		if props.legend ~= false then
			for _, line in series do
				table.insert(legend, ui.row {
					class = "series-chart-key",
					align = "center",
					ui.row { width = 8, height = 8, style = { background = line.color } },
					ui.text { string.upper(line.name), wrap = false },
				})
			end
		end

		return ui.column {
			class = "series-chart",
			hug = true,
			#legend > 0 and ui.row { class = "series-chart-legend", align = "center", table.unpack(legend) },
			ui.column {
				gap = "none",
				ui.row {
					gap = "none",
					ui.column { width = AXIS + 10, height = height, table.unpack(axis) },
					ui.column { width = width, height = height, table.unpack(layers) },
				},
				ui.row {
					gap = "none",
					height = 26,
					ui.row { width = AXIS + 10 },
					ui.column { width = width, height = 26, table.unpack(labels) },
				},
			},
		}
	end,
}
