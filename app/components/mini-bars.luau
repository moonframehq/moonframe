-- A handful of small bars with their labels under them: a figure by month
-- in a header or a KPI strip. One bar can stand out; future periods show
-- as hatched outlines.

type Bar = { label: string, value: number, future: boolean? }

type Props = {
	bars: { Bar },
	highlight: number?,
	height: number?,
	bar: number?,
}

return component {
	title = "Mini Bars",
	category = "Data",
	icon = "chart-column",
	description = "A few small labelled bars, one highlighted, future ones hatched: a figure by period at a glance.",
	when = "A figure over a few periods in a KPI strip or a card header (revenue by month, signups by week). For a live trend in a status bar use `sparkline`; for a full chart with an axis use `bar-chart` or `forecast-chart`.",
	props = {
		["bars"] = "list of { label, value, future? }",
		["highlight"] = "index of the bar to stand out (default the last that is not future)",
		["height"] = "the tallest bar's height in pixels (default 40)",
		["bar"] = "each bar's width in pixels (default 18)",
	},
	styles = {
		[".mini-bars"] = { gap = 6 },
		[".mini-bars-bar"] = { background = "accent/25", radius = 3 },
		[".mini-bars-on"] = { background = "accent" },
		[".mini-bars-future"] = { color = "accent/50", border = true, border_color = "accent/40", radius = 3 },
		[".mini-bars-label"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		bars = {
			{ label = "Jul", value = 18, future = false },
			{ label = "Aug", value = 26, future = false },
			{ label = "Sep", value = 21, future = false },
			{ label = "Oct", value = 33, future = false },
			{ label = "Nov", value = 41, future = false },
			{ label = "Dec", value = 38, future = true },
		},
	},

	build = function(props: Props)
		local height = props.height or 40
		local width = props.bar or 18
		local most = 0
		local last = 0
		for index, bar in props.bars do
			most = math.max(most, bar.value)
			if not bar.future then
				last = index
			end
		end
		local on = props.highlight or last
		local columns = {}
		for index, bar in props.bars do
			local size = if most > 0 then math.max(3, bar.value / most * height) else 3
			local body
			if bar.future then
				body = ui.hatch { class = "mini-bars-future", width = width, height = size, gap = 4 }
			else
				body = ui.row {
					class = if index == on then { "mini-bars-bar", "mini-bars-on" } else "mini-bars-bar",
					width = width,
					height = size,
				}
			end
			table.insert(columns, ui.column {
				align = "center",
				gap = "small",
				ui.column { height = height, justify = "end", body },
				ui.text { bar.label, class = "mini-bars-label" },
			})
		end
		return ui.row { class = "mini-bars", align = "end", hug = true, table.unpack(columns) }
	end,
}
