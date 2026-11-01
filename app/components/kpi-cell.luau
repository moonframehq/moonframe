-- One cell of a row of key figures: a coloured icon tile and a title with a
-- ⋯ menu, the figure large with its change in a tinted pill (green when
-- good, red when bad), and a caption under a dashed rule.

local pill = use "pill"

type Props = {
	title: string,
	value: string,
	icon: string?,
	tint: string?,
	delta: string?,
	good: boolean?,
	caption: string?,
	menu: any,
	to: string?,
}

return component {
	title = "KPI Cell",
	category = "Data",
	icon = "chart-column",
	description = "A key figure in a dashboard's top row: an icon tile and title with a menu, the value large with its change in a pill, and a caption.",
	when = "A row of three to five headline figures across the top of a dashboard (readers, signups, churn), set side by side in a bordered strip. For a figure inside a card's text use `metric`; for a standalone tile with a sparkline use `stat-tile`.",
	props = {
		["title"] = "what it counts",
		["value"] = "the figure, as text",
		["icon"] = "icon on the tile",
		["tint"] = "the tile's colour (default accent)",
		["delta"] = "its change, as text (\"+8.2%\")",
		["good"] = "whether the change is good: green when true (default), red when false",
		["caption"] = "a line under the figure",
		["menu"] = "message the ⋯ button sends; no button without it",
		["to"] = "mod to send `menu` to, if not the drawing one",
	},
	styles = {
		[".kpi-cell"] = { padding = { 16, 18 }, gap = 14 },
		[".kpi-cell-head"] = { gap = 10 },
		[".kpi-cell-tile"] = { radius = 7, color = "#ffffff" },
		[".kpi-cell-title"] = { weight = "medium" },
		[".kpi-cell-value"] = { size = "title", weight = "medium" },
		[".kpi-cell-rule"] = { color = "border" },
		[".kpi-cell-caption"] = { color = "muted-foreground" },
	},
	example = {
		title = "Total readers",
		icon = "user",
		tint = "#7c5cff",
		value = "24,816",
		delta = "+6.4%",
		good = true,
		caption = "All active subscribers",
		menu = "more",
	},

	build = function(props: Props)
		local good = props.good ~= false
		return ui.column {
			class = "kpi-cell",
			grow = true,
			ui.row {
				class = "kpi-cell-head",
				align = "center",
				ui.row {
					class = "kpi-cell-tile",
					width = 24,
					height = 24,
					align = "center",
					justify = "center",
					shrink = false,
					style = { background = props.tint or "accent" },
					ui.icon { name = (props.icon or "hash") :: any, size = "small" },
				},
				ui.text { props.title, class = "kpi-cell-title", truncate = true },
				ui.spacer(),
				props.menu ~= nil and ui.button { icon = "ellipsis", variant = "ghost", small = true, msg = props.menu, to = props.to },
			},
			ui.row {
				align = "center",
				ui.text { props.value, class = "kpi-cell-value" },
				ui.spacer(),
				props.delta and pill {
					label = props.delta,
					icon = "arrow-up",
					tone = if good then "success" else "danger",
				},
			},
			props.caption and ui.line { points = { 0, 0 }, dashed = true, stroke = 1, height = 2, class = "kpi-cell-rule" },
			props.caption and ui.row { justify = "center", ui.text { props.caption, class = "kpi-cell-caption", truncate = true } },
		}
	end,
}
