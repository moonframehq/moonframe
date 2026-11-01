-- Vertical bars, one per bucket, each stacked from its series' values,
-- with labels under them, an optional scale on the left and a legend.
-- Series take a tone (accent, success, warning, danger, muted, or one of
-- the chart's palette `c1`..`c6`) or a hex colour. With `ticks`, the
-- scale labels each step and dashed gridlines run behind the bars; with
-- `grow`, the buckets share the width they are given; a `dim` bucket (a
-- period still running) is drawn faded.

type Series = {
	name: string,
	tone: string?,
	color: string?,
}

type Bucket = {
	label: string?,
	values: { number },
	tooltip: string?,
	dim: boolean?,
}

type Props = {
	buckets: { Bucket },
	series: { Series }?,
	height: number?,
	bar: number?,
	gap: number?,
	max: number?,
	axis: boolean?,
	legend: boolean?,
	unit: string?,
	every: number?,
	slot: number?,
	ticks: number?,
	grid: boolean?,
	grow: boolean?,
}

local TONES = {
	accent = true,
	success = true,
	warning = true,
	danger = true,
	muted = true,
	c1 = true,
	c2 = true,
	c3 = true,
	c4 = true,
	c5 = true,
	c6 = true,
}

local function format(value: number, unit: string?): string
	local text = if value >= 1000 then `{math.floor(value / 100 + 0.5) / 10}k` else tostring(math.floor(value + 0.5))
	return text .. (unit or "")
end

return component {
	title = "Bar Chart",
	category = "Data",
	icon = "chart-column",
	description = "Vertical bars per bucket, stacked by series with toned colours, axis labels, an optional scale, and a legend.",
	when = "Counts over time or per category: tokens per day, runs per hour, alerts per device, stacked by kind. For a tiny live trend in a status bar use `sparkline`; for one fraction use `meter`; for one headline number use `stat-tile`.",
	props = {
		["buckets"] = "list of { label, values = { one number per series }, tooltip, dim (drawn faded: a period still running) }",
		["series"] = "list of { name, tone (accent|success|warning|danger|muted|c1..c6), color (hex) }; bottom of the stack first",
		["height"] = "plot height in pixels (default 120)",
		["bar"] = "bar width in pixels (default 18)",
		["gap"] = "pixels between bucket slots (default 4)",
		["slot"] = "pixels each bucket takes, bar plus room for its label (default 30)",
		["max"] = "value at the top (default the largest stack)",
		["axis"] = "show the top value and 0 on the left",
		["unit"] = "suffix for the scale, e.g. \"k\" or \"%\"",
		["legend"] = "false hides the legend",
		["every"] = "label every Nth bucket (default 1)",
		["ticks"] = "steps on the scale: labels from 0 to `max` at each step, with dashed gridlines (implies `axis`)",
		["grid"] = "false hides the gridlines `ticks` draws",
		["grow"] = "the buckets share the width given instead of taking `slot` pixels each",
	},
	styles = {
		[".bar-chart"] = { gap = 8 },
		[".bar-chart-plot"] = { border = "bottom", border_color = "border" },
		[".bar-chart-bar"] = { radius = 2 },
		[".bar-chart-dim"] = { opacity = 0.45 },
		[".bar-chart-grid"] = { color = "muted-foreground/25" },
		[".bar-chart-empty"] = { background = "muted-foreground/10", radius = 2 },
		[".bar-chart-label"] = { size = "small", color = "muted-foreground" },
		[".bar-chart-scale"] = { size = "small", color = "muted-foreground", font = "mono" },
		[".bar-chart-legend"] = { gap = 14 },
		[".bar-chart-key"] = { gap = 6 },
		[".bar-chart-swatch"] = { radius = 2 },
		[".bar-chart-accent"] = { background = "accent" },
		[".bar-chart-success"] = { background = "success" },
		[".bar-chart-warning"] = { background = "warning" },
		[".bar-chart-danger"] = { background = "danger" },
		[".bar-chart-muted"] = { background = "muted-foreground/50" },
		[".bar-chart-c1"] = { background = "#5b8def" },
		[".bar-chart-c2"] = { background = "#22a99a" },
		[".bar-chart-c3"] = { background = "#e3814f" },
		[".bar-chart-c4"] = { background = "#8b7cf6" },
		[".bar-chart-c5"] = { background = "#3fae6a" },
		[".bar-chart-c6"] = { background = "#d9649a" },
	},
	usage = [[local bar_chart = use "bar-chart"

bar_chart {
	axis = true, unit = "k", height = 120,
	series = { { name = "Input", tone = "c1" }, { name = "Output", tone = "c4" }, { name = "Cache", tone = "muted" } },
	buckets = {
		{ label = "Mon", values = { 42, 18, 12 } },
		{ label = "Tue", values = { 55, 22, 20 } },
		-- ...
	},
}]],
	example = {
		height = 120,
		axis = true,
		unit = "k",
		series = {
			{ name = "Input", tone = "c1" },
			{ name = "Output", tone = "c4" },
			{ name = "Cache", tone = "muted" },
		},
		buckets = {
			{ label = "Mon", values = { 42, 18, 12 } },
			{ label = "Tue", values = { 55, 22, 20 } },
			{ label = "Wed", values = { 31, 12, 9 } },
			{ label = "Thu", values = { 68, 30, 26 } },
			{ label = "Fri", values = { 74, 35, 31 } },
			{ label = "Sat", values = { 12, 4, 3 } },
			{ label = "Sun", values = { 8, 2, 2 } },
		},
	},

	build = function(props: Props)
		local series: { Series } = props.series or { { name = "Value", tone = "accent" } }
		local height = props.height or 120
		local bar = props.bar or 18
		local gap = props.gap or 4
		-- Each bucket takes a slot at least as wide as its label needs.
		local slot = math.max(bar, props.slot or 30)
		local every = props.every or 1

		local max = props.max or 0
		if not props.max then
			for _, bucket in props.buckets do
				local total = 0
				for _, value in bucket.values do
					total += value
				end
				max = math.max(max, total)
			end
		end
		max = math.max(max, 1)

		local ticks = props.ticks
		local function swatch(index: number, width: number, h: number, dim: boolean?)
			local item = series[index] or {} :: any
			local tone = item.tone or "accent"
			local class = { "bar-chart-bar", if TONES[tone] then `bar-chart-{tone}` else "bar-chart-accent" }
			if dim then
				table.insert(class, "bar-chart-dim")
			end
			return ui.row {
				class = class,
				width = width,
				height = h,
				style = if item.color then { background = item.color } else nil,
			}
		end

		local columns = {}
		for index, bucket in props.buckets do
			-- Top of the stack first: the column draws downwards.
			local pieces = {}
			local total = 0
			for s = #bucket.values, 1, -1 do
				local value = math.max(0, bucket.values[s] or 0)
				total += value
				local h = math.floor(value / max * height + 0.5)
				if h > 0 then
					table.insert(pieces, swatch(s, bar, h, bucket.dim))
				end
			end
			if #pieces == 0 then
				pieces[1] = ui.row { class = "bar-chart-empty", width = bar, height = 2 }
			end
			local tip = bucket.tooltip
			if tip == nil then
				tip = `{bucket.label or ""} {format(total, props.unit)}`
			end
			columns[index] = ui.row {
				width = if props.grow then nil else slot,
				grow = props.grow,
				height = height,
				justify = "center",
				align = "end",
				tooltip = tip,
				ui.column {
					width = bar,
					gap = "none",
					justify = "end",
					style = { gap = 1 },
					table.unpack(pieces),
				},
			}
		end

		local labels = {}
		local labelled = false
		for index, bucket in props.buckets do
			local shown = bucket.label and (index - 1) % every == 0
			labelled = labelled or shown == true
			labels[index] = ui.row {
				width = if props.grow then nil else slot,
				grow = props.grow,
				justify = "center",
				shown and ui.text { bucket.label :: string, class = "bar-chart-label", wrap = false },
			}
		end

		-- Gridlines: one-pixel rows spread from the top to the bottom, each
		-- holding a dashed line, floated behind the bars.
		local grid = nil
		local steps = {}
		if ticks and ticks > 0 then
			local lines = {}
			for step = ticks, 0, -1 do
				table.insert(steps, ui.row {
					height = 1,
					align = "center",
					justify = "end",
					ui.text { format(max * step / ticks, props.unit), class = "bar-chart-scale", wrap = false },
				})
				table.insert(lines, ui.column {
					height = 1,
					justify = "center",
					ui.line { points = { 0, 0 }, dashed = true, stroke = 1, height = 1, class = "bar-chart-grid" },
				})
			end
			if props.grid ~= false then
				grid = ui.column { float = "fill", justify = "between", gap = "none", table.unpack(lines) }
			end
		end

		local plot = ui.row {
			class = "bar-chart-plot",
			align = "end",
			height = height,
			style = { gap = gap },
			grid,
			table.unpack(columns),
		}
		local scale = if #steps > 0
			then ui.column {
				height = height,
				justify = "between",
				shrink = false,
				style = { padding = { 0, 2, 0, 0 } },
				table.unpack(steps),
			}
			elseif props.axis
			then ui.column {
				height = height,
				justify = "between",
				align = "end",
				ui.text { format(max, props.unit), class = "bar-chart-scale" },
				ui.text { "0", class = "bar-chart-scale" },
			}
			else nil

		local keys = {}
		if props.legend ~= false and #series > 1 then
			for index, item in series do
				keys[index] = ui.row {
					class = "bar-chart-key",
					align = "center",
					swatch(index, 8, 8),
					ui.text { item.name, class = "bar-chart-label" },
				}
			end
		end

		return ui.row {
			grow = props.grow,
			ui.column {
				class = "bar-chart",
				grow = props.grow,
				ui.row {
					gap = if #steps > 0 then "normal" else "small",
					scale,
					ui.column {
						gap = "small",
						grow = props.grow,
						plot,
						labelled and ui.row { style = { gap = gap }, table.unpack(labels) },
					},
				},
				#keys > 0 and ui.row { class = "bar-chart-legend", wrap = true, table.unpack(keys) },
			},
		}
	end,
}
