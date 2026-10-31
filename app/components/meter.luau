-- A thin bar filled to a fraction, with its percentage: memory, disk,
-- quota. The fill turns to the warning color past `warn`.

type Props = {
	value: number,
	width: number?,
	warn: number?,
	label: string?,
}

return component {
	title = "Meter",
	category = "Data",
	icon = "battery-medium",
	description = "A bar filled to a fraction from 0 to 1, with the percentage beside it; it warns past a threshold.",
	example = { value = 0.62, width = 48, label = "Memory" },

	build = function(props: Props)
		local width = props.width or 40
		local value = math.clamp(props.value, 0, 1)
		local hot = value > (props.warn or 0.85)
		return ui.row {
			align = "center",
			gap = "small",
			props.label and ui.text { props.label, size = "small", tone = "muted" },
			ui.row {
				width = width,
				height = 4,
				style = { background = "muted-foreground/20", radius = 3 },
				ui.row {
					width = value * width,
					height = 4,
					style = { background = if hot then "warning" else "accent", radius = 3 },
				},
			},
			ui.row {
				width = 32,
				justify = "end",
				ui.text { `{math.floor(value * 100 + 0.5)}%`, size = "small", tone = "muted" },
			},
		}
	end,
}
