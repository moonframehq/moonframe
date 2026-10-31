-- Recent samples as bars rising from a baseline, newest on the right. Its
-- width is fixed from the first sample, so it never pushes its neighbours
-- while the history fills.

type Props = {
	-- Fractions from 0 to 1, oldest first.
	values: { number },
	samples: number?,
	height: number?,
	hot: number?,
}

return component {
	title = "Sparkline",
	category = "Data",
	icon = "chart-pie",
	description = "Recent samples (0 to 1) as tiny bars, newest on the right, in a fixed width; samples past `hot` turn to the warning color.",
	when = "Recent samples as tiny bars, like CPU over the last minute, in a status bar or header.",
	props = {
		["values"] = "samples from 0 to 1, oldest first",
		["samples"] = "how many bars to show",
		["height"] = "pixels",
		["hot"] = "value past which a bar turns warning",
	},
	example = { values = { 0.1, 0.3, 0.2, 0.5, 0.4, 0.9, 0.6, 0.3, 0.2, 0.4, 0.7, 0.5 } },

	build = function(props: Props)
		local samples = props.samples or 16
		local height = props.height or 12
		local bars = {}
		for index = math.max(1, #props.values - samples + 1), #props.values do
			local sample = math.clamp(props.values[index], 0, 1)
			bars[#bars + 1] = ui.row {
				width = 2,
				height = math.max(1, sample * height),
				style = {
					background = if sample > (props.hot or 0.8) then "warning" else "muted-foreground/60",
					radius = 1,
				},
			}
		end
		return ui.row {
			align = "end",
			justify = "end",
			width = samples * 3 - 1,
			height = height,
			style = { gap = 1 },
			table.unpack(bars),
		}
	end,
}
