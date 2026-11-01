-- One line of a breakdown: a label in capitals, a rounded track filled to
-- a fraction with a gradient, and the count at the end. Stack a few for
-- leads by status, tickets by priority, a funnel.

type Props = {
	label: string,
	value: string?,
	fraction: number,
	color: string?,
	end_color: string?,
	width: number?,
	label_width: number?,
	height: number?,
	tooltip: string?,
	msg: any?,
	to: string?,
}

return component {
	title = "Level Row",
	category = "Lists",
	icon = "chart-column",
	description = "A capitalised label, a rounded track filled to a fraction with a gradient, and its count.",
	when = "Rows of a breakdown where each part is read on its own scale: leads by status, a funnel's stages, tickets by priority. For parts of one whole with shares use `ratio-row` or `share-bar`; for one fraction with a percentage use `meter`; for a quota use `usage-bar`.",
	props = {
		["label"] = "what it counts, shown in capitals",
		["value"] = "the count at the end, as text",
		["fraction"] = "0 to 1: how far the fill goes",
		["color"] = "the fill's colour at its start (default accent)",
		["end_color"] = "the fill's colour at its end, for a gradient (default `color`)",
		["width"] = "the track's width in pixels (default 200)",
		["label_width"] = "the label column's width in pixels (default 120)",
		["height"] = "the track's height in pixels (default 20)",
		["tooltip"] = "text under the pointer",
		["msg"] = "message a click sends",
		["to"] = "mod to send `msg` to, if not the drawing one",
	},
	styles = {
		[".level-row"] = { gap = 16, padding = { 10, 0 } },
		[".level-row-label"] = { font = "mono", color = "muted-foreground", case = "upper" },
		[".level-row-track"] = { background = "foreground/7", border = true, border_color = "foreground/6", radius = 10 },
		[".level-row-fill"] = { radius = 10 },
		[".level-row-value"] = { font = "mono", color = "muted-foreground" },
	},
	example = { label = "New leads", value = "134", fraction = 0.8, color = "#5fdcff", end_color = "#00a8ff", width = 220 },

	build = function(props: Props)
		local width = props.width or 200
		local height = props.height or 20
		local fraction = math.clamp(props.fraction, 0, 1)
		local from = props.color or "accent"
		local fill = math.floor(width * fraction + 0.5)
		return ui.row {
			class = "level-row",
			align = "center",
			tooltip = props.tooltip,
			msg = props.msg,
			to = props.to,
			ui.row { width = props.label_width or 120, shrink = false, ui.text { props.label, class = "level-row-label", truncate = true } },
			ui.row {
				class = "level-row-track",
				width = width,
				height = height,
				shrink = false,
				fill > 0 and ui.row {
					class = "level-row-fill",
					width = fill,
					style = { gradient = `90deg, {from}, {props.end_color or from}` },
				},
			},
			ui.spacer(),
			props.value and ui.text { props.value, class = "level-row-value" },
		}
	end,
}
