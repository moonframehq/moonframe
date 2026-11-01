-- A utilization meter drawn as a comb of thin vertical ticks: those up to
-- the value in a colour, the rest dim. The ticks share whatever width the
-- meter is given, so it needs no measuring.

type Props = {
	value: number,
	color: string?,
	ticks: number?,
	height: number?,
	tooltip: string?,
}

return component {
	title = "Tick Meter",
	category = "Data",
	icon = "chart-column",
	description = "A utilization bar made of thin vertical ticks, filled in a colour up to the value and dim after it.",
	when = "A resource's utilization in a dense, technical dashboard (CPU, GPU, memory, a queue's depth), where a striped instrument reads better than a smooth bar. For a compact bar with its percentage use `meter`; for a quota with a reset use `usage-bar`; for task progress use `progress`.",
	props = {
		["value"] = "0 to 1: the share of ticks lit",
		["color"] = "the lit ticks' colour (default accent)",
		["ticks"] = "how many ticks across (default 110)",
		["height"] = "pixels (default 26)",
		["tooltip"] = "text under the pointer (default the percentage)",
	},
	styles = {
		[".tick-meter"] = { gap = 2 },
		[".tick-meter-tick"] = { background = "muted-foreground/18" },
		[".tick-meter-on"] = { background = "accent" },
	},
	example = { value = 0.57, color = "#3b82f6" },

	build = function(props: Props)
		local count = math.max(1, math.floor(props.ticks or 110))
		local lit = math.round(math.clamp(props.value or 0, 0, 1) * count)
		local color = props.color
		local ticks = table.create(count)
		for index = 1, count do
			local on = index <= lit
			ticks[index] = ui.row {
				class = if on then { "tick-meter-tick", "tick-meter-on" } else "tick-meter-tick",
				grow = true,
				style = if on and color then { background = color } else nil,
			}
		end
		return ui.row {
			class = "tick-meter",
			height = props.height or 26,
			align = "stretch",
			tooltip = props.tooltip or `{math.round((props.value or 0) * 100)}%`,
			table.unpack(ticks),
		}
	end,
}
