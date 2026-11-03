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
	when = "Recent samples as tiny bars, like CPU over the last minute, in a status bar or header. For the latest sample alone in a tight row use `vertical-bar`.",
	props = {
		["values"] = "samples from 0 to 1, oldest first",
		["samples"] = "how many bars to show",
		["height"] = "pixels",
		["hot"] = "value past which a bar turns warning",
	},
	styles = {
		[".sparkline"] = { gap = 1 },
		[".sparkline-bar"] = { background = "muted-foreground/60", radius = 1 },
		[".sparkline-hot"] = { background = "warning" },
	},
	example = { values = { 0.1, 0.3, 0.2, 0.5, 0.4, 0.9, 0.6, 0.3, 0.2, 0.4, 0.7, 0.5 } },

	build = function(props: Props)
		local samples = props.samples or 16
		local height = props.height or 12
		local bars = {}
		for index = math.max(1, #props.values - samples + 1), #props.values do
			local sample = math.clamp(props.values[index], 0, 1)
			bars[#bars + 1] = ui.row {
				class = if sample > (props.hot or 0.8) then { "sparkline-bar", "sparkline-hot" } else "sparkline-bar",
				width = 2,
				height = math.max(1, sample * height),
			}
		end
		return ui.row {
			class = "sparkline",
			align = "end",
			justify = "end",
			width = samples * 3 - 1,
			height = height,
			table.unpack(bars),
		}
	end,
}
