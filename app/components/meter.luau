-- A thin bar filled to a fraction, with its percentage: memory, disk,
-- quota. The fill turns to the warning color past `warn`.

type Props = {
	value: number,
	width: number?,
	warn: number?,
	label: string?,
	percent: boolean?,
}

return component {
	title = "Meter",
	category = "Data",
	icon = "battery-medium",
	description = "A bar filled to a fraction from 0 to 1, with the percentage beside it; it warns past a threshold.",
	when = "A fraction (0 to 1) as a bar with its percentage: memory, quota, progress of a known size. For work with no known size use `progress` with `busy`; for a narrow upright bar in a tight row use `vertical-bar`.",
	props = {
		["value"] = "0 to 1",
		["width"] = "bar width in pixels",
		["warn"] = "fraction past which it shows the warning color",
		["label"] = "text before the bar",
		["percent"] = "false hides the percentage",
	},
	styles = {
		[".meter-label"] = { size = "small", color = "muted-foreground" },
		[".meter-track"] = { background = "muted-foreground/20", radius = 3 },
		[".meter-fill"] = { background = "accent", radius = 3 },
		[".meter-hot"] = { background = "warning" },
		[".meter-percent"] = { size = "small", color = "muted-foreground" },
	},
	example = { value = 0.62, width = 48, label = "Memory" },

	build = function(props: Props)
		local width = props.width or 40
		local value = math.clamp(props.value, 0, 1)
		return ui.row {
			align = "center",
			gap = "small",
			props.label and ui.text { props.label, class = "meter-label" },
			ui.row {
				class = "meter-track",
				width = width,
				height = 4,
				ui.row {
					class = if value > (props.warn or 0.85) then { "meter-fill", "meter-hot" } else "meter-fill",
					width = value * width,
					height = 4,
				},
			},
			props.percent ~= false and ui.row {
				width = 32,
				justify = "end",
				ui.text { `{math.floor(value * 100 + 0.5)}%`, class = "meter-percent" },
			},
		}
	end,
}
