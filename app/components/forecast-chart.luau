-- A forecast in bars: one bar per period from today, the change over each
-- above it, a floor drawn across as a dashed line with what lies under it
-- hatched (held, not free to use), the lowest period flagged, and an axis.
-- Bars share the width; the plot's height is fixed.

type Bar = {
	label: string,
	value: number,
	delta: string?,
	note: string?,
	current: boolean?,
	low: boolean?,
	tooltip: string?,
}

type Props = {
	bars: { Bar },
	max: number,
	floor: number?,
	ticks: { string }?,
	height: number?,
	bar: number?,
	event: string?,
	to: string?,
}

return component {
	title = "Forecast Chart",
	category = "Data",
	icon = "chart-column",
	description = "Bars over periods with each change above them, a dashed floor with the held part hatched, the low period flagged, and a value axis.",
	when = "A balance or capacity projected over weeks or months against a minimum (cash against a floor, stock against a reorder point). For counts per bucket, stacked by kind, use `bar-chart`; for a small trend use `sparkline` or `ui.line`.",
	props = {
		["bars"] = "list of { label, value, delta? (\"+9.2\"), note? (text over the bar instead), current? (drawn dark), low? (flagged), tooltip? }",
		["max"] = "the value at the top of the plot",
		["floor"] = "a value drawn across, dashed; the part of each bar under it is hatched",
		["ticks"] = "axis labels from the top (max) down to 0, evenly spaced",
		["height"] = "the plot's height in pixels (default 260)",
		["bar"] = "each bar's width in pixels (default 28)",
		["event"] = "key of the message a bar sends: { [event] = index }; none without it",
		["to"] = "mod to send it to, if not the drawing one",
	},
	styles = {
		[".forecast"] = { gap = 10 },
		[".forecast-axis"] = { gap = 0 },
		[".forecast-tick"] = { size = "small", color = "muted-foreground", font = "mono" },
		[".forecast-bar"] = { gap = 4 },
		[".forecast-fill"] = { background = "accent", radius = 6 },
		[".forecast-current"] = { background = "foreground" },
		[".forecast-delta"] = { size = "small", font = "mono", color = "muted-foreground" },
		[".forecast-up"] = { color = "accent" },
		[".forecast-low"] = { background = "danger", color = "#ffffff", radius = 4, padding = { 1, 5 } },
		[".forecast-low-text"] = { size = "small", weight = "bold" },
		[".forecast-floor"] = { color = "danger" },
		[".forecast-held"] = { color = "muted-foreground/30", background = "muted-foreground/6" },
		[".forecast-label"] = { size = "small", color = "muted-foreground" },
		[".forecast-label-now"] = { color = "foreground", weight = "bold" },
		[".forecast-label-low"] = { color = "danger", weight = "medium" },
	},
	example = {
		max = 60,
		floor = 15,
		height = 160,
		ticks = { "$60M", "$30M", "$0" },
		-- Every bar with the same keys, for the type checker.
		bars = {
			{ label = "Today", value = 46.8, delta = "$46.8M", current = true, low = false },
			{ label = "Nov 2", value = 44, delta = "−2.8", current = false, low = false },
			{ label = "Nov 9", value = 53.2, delta = "+9.2", current = false, low = false },
			{ label = "Nov 16", value = 36.4, delta = "−16.8", current = false, low = false },
			{ label = "Nov 23", value = 21.1, delta = "−15.3", current = false, low = true },
			{ label = "Nov 30", value = 33, delta = "+11.9", current = false, low = false },
		},
	},

	build = function(props: Props)
		local height = props.height or 260
		local floor = props.floor or 0
		local top_room = 44
		local scale = height / props.max
		local columns = {}
		for index, bar in props.bars do
			local value = math.max(0, math.min(bar.value, props.max))
			local held = math.min(value, floor)
			local free = value - held
			local text = bar.note or bar.delta
			local up = bar.delta and string.sub(bar.delta, 1, 1) == "+"
			local fill = { "forecast-fill" }
			if bar.current then
				table.insert(fill, "forecast-current")
			end
			table.insert(columns, ui.column {
				class = "forecast-bar",
				grow = true,
				align = "center",
				justify = "end",
				height = height + top_room,
				tooltip = bar.tooltip,
				msg = if props.event then { [props.event] = index } else nil,
				to = props.to,
				bar.low and ui.row { class = "forecast-low", ui.text { "LOW", class = "forecast-low-text" } },
				text and ui.text {
					text,
					class = if up then { "forecast-delta", "forecast-up" } elseif bar.note then { "forecast-delta", "forecast-label-now" } else "forecast-delta",
				},
				free > 0 and ui.row { class = fill, width = props.bar or 28, height = math.max(2, free * scale) },
				floor > 0 and ui.line { points = { 0, 0 }, dashed = true, stroke = 1, height = 2, class = "forecast-floor" },
				held > 0 and ui.hatch { class = "forecast-held", width = props.bar or 28, height = held * scale, gap = 5 },
			})
		end
		local labels = {}
		for _, bar in props.bars do
			local class = { "forecast-label" }
			if bar.current then
				table.insert(class, "forecast-label-now")
			elseif bar.low then
				table.insert(class, "forecast-label-low")
			end
			table.insert(labels, ui.row { grow = true, justify = "center", ui.text { string.upper(bar.label), class = class, wrap = false } })
		end
		local ticks = {}
		local given: { string } = props.ticks or {}
		for _, tick in given do
			table.insert(ticks, ui.text { tick, class = "forecast-tick" })
		end
		return ui.column {
			class = "forecast",
			ui.row {
				gap = "small",
				#ticks > 0 and ui.column {
					class = "forecast-axis",
					shrink = false,
					ui.row { height = top_room - 8 },
					ui.column { height = height + 16, justify = "between", align = "end", table.unpack(ticks) },
				},
				ui.row { grow = true, gap = "none", align = "end", table.unpack(columns) },
			},
			ui.row {
				gap = "none",
				#ticks > 0 and ui.row { width = 40, shrink = false },
				table.unpack(labels),
			},
		}
	end,
}
