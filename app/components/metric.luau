-- One figure on a dashboard: a small label, the value large, and a note
-- under it in a tone ("above the floor" in green, "−19.3 from today").

type Props = {
	label: string,
	value: string,
	note: string?,
	tone: string?,
	size: string?,
}

return component {
	title = "Metric",
	category = "Data",
	icon = "hash",
	description = "A label, a value set large (or very large), and a note under it in a tone.",
	when = "Headline figures across the top of a dashboard card: cash today, the lowest week, a total. For a figure in its own tile with a trend use `stat-tile`; for a status bar reading use `stat`; for a fraction use `meter`.",
	props = {
		["label"] = "what it is",
		["value"] = "the figure, as text",
		["note"] = "a line under it",
		["tone"] = "the note's tone: success, danger, warning or muted (default)",
		["size"] = "\"normal\" (default) or \"hero\": the card's main figure",
	},
	styles = {
		[".metric"] = { gap = 2 },
		[".metric-label"] = { color = "muted-foreground" },
		[".metric-value"] = { size = "title", weight = "medium" },
		[".metric-hero"] = { size = "title", weight = "bold" },
		[".metric-note"] = { size = "small", color = "muted-foreground" },
	},
	example = { label = "Lowest week", value = "$26.4M", note = "Week of Jan 4" },

	build = function(props: Props)
		local hero = props.size == "hero"
		return ui.column {
			class = "metric",
			hug = true,
			shrink = false,
			ui.text { props.label, class = "metric-label", wrap = false },
			ui.text {
				props.value,
				wrap = false,
				class = if hero then { "metric-value", "metric-hero" } else "metric-value",
				style = if hero then { margin = { top = 4 } } else nil,
			},
			props.note and ui.text {
				props.note,
				class = "metric-note",
				tone = (if props.tone and props.tone ~= "muted" then props.tone else nil) :: any,
			},
		}
	end,
}
