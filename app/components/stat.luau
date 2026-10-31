-- An icon, a label, and a value: one reading in a status bar. The value
-- sits in a box of fixed width, so a changing number never pushes its
-- neighbours.

type Props = {
	-- A name from the bundled icons (`IconName`).
	icon: string?,
	label: string?,
	value: string,
	-- Pixels for the value; wide enough for its longest reading.
	width: number?,
	tone: any,
}

return component {
	title = "Stat",
	category = "Status bar",
	icon = "cpu",
	description = "An icon, an optional label, and a value in a fixed-width box, for status bars and toolbars.",
	when = "One labelled value with an icon in a status bar or toolbar, kept to a fixed width so it does not jitter.",
	props = {
		["icon"] = "icon name",
		["label"] = "short label",
		["value"] = "the value text",
		["width"] = "value width in pixels",
		["tone"] = "text tone",
	},
	example = { icon = "cpu", label = "CPU", value = "12%", width = 32 },

	build = function(props: Props)
		return ui.row {
			align = "center",
			gap = "small",
			props.icon and ui.icon { name = props.icon :: any, style = { size = "small", color = "muted-foreground" } },
			props.label and ui.text { props.label, size = "small", tone = "muted", weight = "medium" },
			ui.row {
				width = props.width,
				justify = "end",
				ui.text { props.value, size = "small", tone = props.tone },
			},
		}
	end,
}
