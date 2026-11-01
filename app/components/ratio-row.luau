-- A part of a whole as a row: an icon (or a leading element), the label,
-- its count muted and its share bold at the right, and optionally a thin
-- line under it as long as the share, in the part's colour.

type Props = {
	label: string,
	count: string?,
	percent: string?,
	icon: string?,
	leading: any,
	share: number?,
	color: string?,
	width: number?,
	msg: any,
	to: string?,
}

return component {
	title = "Ratio Row",
	category = "Lists",
	icon = "list",
	description = "A label with its count and share at the right, after an icon, with an optional thin bar under it as long as the share.",
	when = "Lists of parts of a whole: where signups came from, top countries, devices, tiers. Stack them in a column; give `share` for the bar under each. For a bar of all the parts at once use `share-bar`; for a data grid use `table`.",
	props = {
		["label"] = "what the part is",
		["count"] = "its count, muted",
		["percent"] = "its share, bold (\"42%\")",
		["icon"] = "icon in a small tile before the label",
		["leading"] = "an element instead of the icon (a `country-code`, a dot)",
		["share"] = "0 to 100: draw a bar under the row this long",
		["color"] = "the bar's colour (default accent)",
		["width"] = "the bar's full width in pixels (default 360)",
		["msg"] = "message a click sends",
		["to"] = "mod to send `msg` to, if not the drawing one",
	},
	styles = {
		[".ratio-row"] = { padding = { 8, 0 }, gap = 8 },
		[".ratio-row-line"] = { gap = 10 },
		[".ratio-row-icon"] = { radius = 6, border = true, border_color = "border", color = "muted-foreground" },
		[".ratio-row-count"] = { color = "muted-foreground" },
		[".ratio-row-percent"] = { weight = "medium" },
		[".ratio-row-track"] = { background = "muted-foreground/12", radius = 3 },
		[".ratio-row-fill"] = { radius = 3 },
	},
	example = { label = "Website signup form", icon = "globe", count = "604", percent = "44%", share = 44, color = "#7c5cff" },

	build = function(props: Props)
		local lead = props.leading
		if not lead and props.icon then
			lead = ui.row {
				class = "ratio-row-icon",
				width = 26,
				height = 26,
				align = "center",
				justify = "center",
				shrink = false,
				ui.icon { name = props.icon :: any, size = "small" },
			}
		end
		local width = props.width or 360
		return ui.column {
			class = "ratio-row",
			msg = props.msg,
			to = props.to,
			ui.row {
				class = "ratio-row-line",
				align = "center",
				lead,
				ui.text { props.label, truncate = true },
				ui.spacer(),
				props.count and ui.text { props.count, class = "ratio-row-count" },
				props.percent and ui.text { props.percent, class = "ratio-row-percent" },
			},
			props.share and ui.row {
				class = "ratio-row-track",
				width = width,
				height = 5,
				style = if props.icon or props.leading then { margin = { left = 36 } } else nil,
				ui.row {
					class = "ratio-row-fill",
					width = width * math.clamp(props.share, 0, 100) / 100,
					height = 5,
					style = { background = props.color or "accent" },
				},
			},
		}
	end,
}
