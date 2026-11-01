-- Lines over the same days or weeks, solid or dashed (actual against
-- predicted), with faint columns behind them, a value axis and labels.
-- Each period is a slot that lights up under the pointer and, clicked,
-- picks that period: a rule and dots mark it, and the card the mod passes
-- (the figures for that day) floats beside it. Views cannot measure, so
-- the plot takes its size in pixels.

type Series = { name: string?, points: { number }, color: string?, dashed: boolean? }

type Props = {
	labels: { string },
	series: { Series },
	bars: { number }?,
	max: number,
	min: number?,
	ticks: { string }?,
	width: number?,
	height: number?,
	selected: number?,
	card: any,
	card_width: number?,
	tips: { string }?,
	event: string?,
	to: string?,
}

return component {
	title = "Line Chart",
	category = "Data",
	icon = "activity",
	description = "Solid and dashed lines over periods with faint columns behind, an axis, labels, and a picked period with a rule, dots and a floating card.",
	when = "A measure over days or weeks compared with a forecast or a target (actual against predicted, this year against last), where the user picks a day to read its figures. For several readings on one scale with a legend and a crosshair use `series-chart`; for bars against a floor use `forecast-chart`; for stacked counts use `bar-chart`; for a bare trend use `ui.line` or `sparkline`.",
	props = {
		["labels"] = "one label per period, under the plot",
		["series"] = "list of { name?, points (one per period), color? (default accent), dashed? }; the first is drawn under the next",
		["bars"] = "faint columns behind the lines, one value per period",
		["max"] = "the value at the top of the plot",
		["min"] = "the value at the bottom (default 0)",
		["ticks"] = "axis labels from the top (max) down to the bottom, evenly spaced",
		["width"] = "the plot's width in pixels (default 600)",
		["height"] = "the plot's height in pixels (default 180)",
		["selected"] = "index of the picked period: a rule, dots, and `card`",
		["card"] = "element floating beside the picked period (its figures)",
		["card_width"] = "the card's width in pixels, to keep it inside the plot (default 170)",
		["tips"] = "tooltip text per period, shown under the pointer",
		["event"] = "key of the message a period sends, default \"point\": { point = index }",
		["to"] = "mod to send it to, if not the drawing one",
	},
	styles = {
		[".line-chart"] = { gap = 10 },
		[".line-chart-tick"] = { size = "small", font = "mono", color = "muted-foreground" },
		[".line-chart-label"] = { size = "small", color = "muted-foreground" },
		[".line-chart-label-on"] = { size = "small", color = "foreground", weight = "medium" },
		[".line-chart-slot"] = { radius = 4 },
		[".line-chart-slot:hover"] = { background = "foreground/4" },
		[".line-chart-bar"] = { background = "muted-foreground/7", radius = { 3, 3, 0, 0 } },
		[".line-chart-base"] = { background = "border" },
		[".line-chart-rule"] = { background = "muted-foreground/50" },
		[".line-chart-marker"] = { color = "foreground", size = "small" },
		[".line-chart-point"] = { radius = 4, border = true, border_color = "background" },
		[".line-chart-card"] = { background = "background", radius = 8, border = true, border_color = "border", shadow = "0 6px 16px -4px #00000022", padding = 10, gap = 6 },
	},
	example = {
		labels = { "21 Oct", "22 Oct", "23 Oct", "24 Oct", "25 Oct", "26 Oct", "27 Oct", "28 Oct" },
		series = {
			{ name = "Predicted", points = { 6, 4, 6, 7, 5, 3, 5, 4 }, color = "#3f9a5c", dashed = true },
			{ name = "Actual", points = { 3, 6, 11, 9, 8, 9, 9, 3 }, color = "#3f73b5", dashed = false },
		},
		bars = { 6, 5, 6, 7, 6, 6, 6, 5 },
		max = 15,
		ticks = { "15", "10", "5", "0" },
		width = 520,
		height = 160,
		selected = 4,
	},

	build = function(props: Props)
		local labels = props.labels
		local count = math.max(#labels, 1)
		local width = props.width or 600
		local height = props.height or 180
		local low = props.min or 0
		-- `math.clamp` raises when its bounds cross: a `max` under `min`
		-- (all-negative data with the default `min = 0`) draws flat instead.
		local high = math.max(props.max, low)
		local span = math.max(high - low, 0.0001)
		local slot = width / count
		local event = props.event or "point"
		local selected = props.selected

		local function y(value: number): number
			return height - height * (math.clamp(value, low, high) - low) / span
		end

		-- The slots: a faint column each, lit under the pointer, picked on a click.
		local slots = {}
		for index = 1, count do
			local bar = props.bars and props.bars[index]
			table.insert(slots, ui.column {
				class = "line-chart-slot",
				width = slot,
				height = height,
				align = "center",
				justify = "end",
				shrink = false,
				selected = selected == index,
				tooltip = props.tips and props.tips[index],
				msg = { [event] = index },
				to = props.to,
				bar and ui.row { class = "line-chart-bar", width = math.floor(slot * 0.62), height = math.max(0, height - y(bar)) },
			})
		end

		-- The lines, each over the whole plot, from the first slot's middle to the last's.
		local layers = {}
		for _, series in props.series do
			table.insert(layers, ui.column {
				float = { top = 0, left = slot / 2 },
				width = math.max(1, width - slot),
				height = height,
				ui.line {
					points = series.points,
					min = low,
					max = props.max,
					dashed = series.dashed,
					stroke = 1.5,
					width = math.max(1, width - slot),
					height = height,
					style = { color = series.color or "accent" },
				},
			})
		end

		-- The picked period: a rule, a dot on each line, and the card.
		if selected and selected >= 1 and selected <= count then
			local x = slot * (selected - 1) + slot / 2
			table.insert(layers, ui.row { class = "line-chart-rule", float = { top = 0, left = x }, width = 1, height = height })
			for _, series in props.series do
				local value = series.points[selected]
				if value then
					table.insert(layers, ui.row {
						class = "line-chart-point",
						float = { top = y(value) - 4, left = x - 4 },
						width = 8,
						height = 8,
						style = { background = series.color or "accent" },
					})
				end
			end
			if props.card then
				local card_width = props.card_width or 170
				local left = if x + 14 + card_width <= width then x + 14 else x - 14 - card_width
				table.insert(layers, ui.column { class = "line-chart-card", float = { top = 6, left = math.max(0, left) }, width = card_width, props.card })
			end
		end

		local base = ui.row { class = "line-chart-base", float = { top = height, left = 0 }, width = width, height = 1 }

		local names = {}
		for index, label in labels do
			local on = selected == index
			table.insert(names, ui.column {
				width = slot,
				shrink = false,
				align = "center",
				gap = "none",
				ui.text { if on then "▲" else " ", class = "line-chart-marker" },
				ui.text { label, class = if on then "line-chart-label-on" else "line-chart-label", wrap = false },
			})
		end

		local ticks = {}
		local tick_labels: { string } = props.ticks or {}
		for _, tick in tick_labels do
			table.insert(ticks, ui.text { tick, class = "line-chart-tick" })
		end

		return ui.row {
			class = "line-chart",
			align = "start",
			#ticks > 0 and ui.column {
				width = 22,
				height = height,
				shrink = false,
				ui.column {
					float = { top = -8, left = 0 },
					width = 22,
					height = height + 16,
					align = "end",
					justify = "between",
					gap = "none",
					table.unpack(ticks),
				},
			},
			ui.column {
				gap = "none",
				shrink = false,
				ui.column {
					width = width,
					height = height,
					gap = "none",
					shrink = false,
					ui.row { gap = "none", width = width, height = height, table.unpack(slots) },
					base,
					table.unpack(layers),
				},
				ui.row { gap = "none", width = width, table.unpack(names) },
			},
		}
	end,
}
