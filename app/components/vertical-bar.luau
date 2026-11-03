-- A narrow upright bar filled from the bottom to a fraction, with its
-- percentage beside it: a reading that fits in a single tight row, such as
-- a host's CPU and memory next to its name. The fill warns past `warn`.

type Props = {
	value: number,
	height: number?,
	width: number?,
	warn: number?,
	label: string?,
	percent: boolean?,
	tooltip: string?,
}

return component {
	title = "Vertical Bar",
	category = "Data",
	icon = "chart-column",
	description = "A narrow upright bar filled from the bottom to a fraction (0 to 1), with an optional label and the percentage; it warns past a threshold.",
	when = "A fraction in a row with no room for a horizontal bar: a host's CPU or memory beside its name in a sidebar, a queue's depth in a tab. For a bar with room to spread use `meter`; for a history use `sparkline`.",
	props = {
		["value"] = "0 to 1",
		["height"] = "bar height in pixels (default 12)",
		["width"] = "bar width in pixels (default 3)",
		["warn"] = "fraction past which it shows the warning color (default 0.85)",
		["label"] = "text before the bar",
		["percent"] = "false hides the percentage",
		["tooltip"] = "text under the pointer (default the percentage)",
	},
	styles = {
		[".vertical-bar-label"] = { size = "small", color = "muted-foreground" },
		[".vertical-bar-track"] = { background = "muted-foreground/20", radius = 1 },
		[".vertical-bar-fill"] = { background = "muted-foreground/70", radius = 1 },
		[".vertical-bar-hot"] = { background = "warning" },
		[".vertical-bar-percent"] = { size = "small", color = "muted-foreground" },
	},
	example = { value = 0.5, label = "MEM" },

	build = function(props: Props)
		local height = props.height or 12
		local width = props.width or 3
		local value = math.clamp(props.value, 0, 1)
		local text = `{math.floor(value * 100 + 0.5)}%`
		return ui.row {
			align = "center",
			gap = "small",
			tooltip = props.tooltip or text,
			props.label and ui.text { props.label, class = "vertical-bar-label" },
			ui.column {
				class = "vertical-bar-track",
				width = width,
				height = height,
				justify = "end",
				ui.row {
					class = if value > (props.warn or 0.85) then { "vertical-bar-fill", "vertical-bar-hot" } else "vertical-bar-fill",
					width = width,
					height = math.max(1, value * height),
				},
			},
			props.percent ~= false and ui.row {
				width = 32,
				justify = "end",
				ui.text { text, class = "vertical-bar-percent" },
			},
		}
	end,
}
