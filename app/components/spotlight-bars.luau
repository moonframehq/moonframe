-- Tall rounded columns, one per period, sharing the width they are given.
-- One is in the spotlight: painted with a gradient and its label lit, the
-- rest quiet. A click on a column sends { select = index } so the mod can
-- move the spotlight; the pointer brightens a column it rests on.

type Bar = {
	label: string?,
	value: number,
	tooltip: string?,
}

type Props = {
	bars: { Bar },
	selected: number?,
	height: number?,
	max: number?,
	bar: number?,
	event: string?,
	to: string?,
}

return component {
	title = "Spotlight Bars",
	category = "Data",
	icon = "chart-column",
	description = "Tall rounded columns sharing the width, one in the spotlight with a gradient and a lit label; a click moves it.",
	when = "One figure over a dozen periods where one period is the story (revenue by month, the best week), in a dashboard card that is wide and tall. For several series stacked use `bar-chart`; for a few small bars in a KPI strip use `mini-bars`; for a balance against a floor use `forecast-chart`.",
	props = {
		["bars"] = "list of { label, value, tooltip? }, left to right",
		["selected"] = "index of the bar in the spotlight (none when unset)",
		["height"] = "the tallest bar's height in pixels (default 220)",
		["max"] = "the value at the top (default the largest)",
		["bar"] = "each bar's width in pixels; unset, bars share the width",
		["event"] = "key of the message a click sends, default \"select\": { select = index }",
		["to"] = "mod to send it to, if not the drawing one",
	},
	styles = {
		[".spotlight-bars"] = { gap = 12 },
		[".spotlight-bars-column"] = { gap = 14 },
		[".spotlight-bars-bar"] = { radius = 22, border = true, border_color = "foreground/6" },
		[".spotlight-bars-idle"] = { background = "foreground/7" },
		[".spotlight-bars-idle:hover"] = { background = "foreground/12", border_color = "foreground/16" },
		[".spotlight-bars-on"] = { gradient = "180deg, #ff9a1f, #ff6400", border_color = "#ffffff00" },
		[".spotlight-bars-label"] = { font = "mono", color = "muted-foreground" },
		[".spotlight-bars-label-on"] = { color = "foreground" },
	},
	usage = [[local spotlight_bars = use "spotlight-bars"

spotlight_bars {
	selected = state.month, height = 220,
	bars = {
		{ label = "JAN", value = 90, tooltip = "$90,214" },
		{ label = "FEB", value = 145 },
		-- ...
	},
} -- sends { select = index }]],
	example = {
		selected = 5,
		height = 140,
		bars = {
			{ label = "JAN", value = 90 },
			{ label = "FEB", value = 145 },
			{ label = "MAR", value = 117 },
			{ label = "APR", value = 136 },
			{ label = "MAY", value = 193 },
			{ label = "JUN", value = 158 },
			{ label = "JUL", value = 112 },
			{ label = "AUG", value = 170 },
		},
	},

	build = function(props: Props)
		local height = props.height or 220
		local max = props.max or 0
		if not props.max then
			for _, bar in props.bars do
				max = math.max(max, bar.value)
			end
		end
		max = math.max(max, 1)
		local event = props.event or "select"

		local columns = {}
		for index, bar in props.bars do
			local on = index == props.selected
			local h = math.max(28, math.floor(math.clamp(bar.value, 0, max) / max * height + 0.5))
			columns[index] = ui.column {
				class = "spotlight-bars-column",
				grow = true,
				msg = { [event] = index },
				to = props.to,
				tooltip = bar.tooltip,
				ui.column {
					height = height,
					justify = "end",
					align = if props.bar then "center" else "stretch",
					ui.row {
						class = { "spotlight-bars-bar", if on then "spotlight-bars-on" else "spotlight-bars-idle" },
						width = props.bar,
						height = h,
					},
				},
				bar.label and ui.row {
					justify = "center",
					ui.text { bar.label :: string, class = if on then { "spotlight-bars-label", "spotlight-bars-label-on" } else "spotlight-bars-label" },
				},
			}
		end

		return ui.row { class = "spotlight-bars", align = "end", table.unpack(columns) }
	end,
}
