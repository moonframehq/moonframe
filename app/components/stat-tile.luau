-- A dashboard tile: a label, a big number, and how it moved ("+12%" in
-- green or red with an arrow), with an optional note and anything else
-- (a sparkline, a meter) under it.

type Props = {
	label: string,
	value: string,
	unit: string?,
	icon: string?,
	delta: string?,
	trend: string?,
	good: string?,
	detail: string?,
	msg: any,
	to: string?,
	selected: boolean?,
	width: number?,
	[number]: any,
}

return component {
	title = "Stat Tile",
	category = "Data",
	icon = "layout-dashboard",
	description = "A dashboard tile: label, big value with unit, a delta with an up or down arrow in good or bad colour, a note, and optional content.",
	when = "A headline number on a dashboard or overview: open PRs, tokens today, uptime, failing checks; put several in a wrapping row. For a status bar reading use `stat`; for a fraction use `meter`; for a quota use `usage-bar`; for a series use `bar-chart`.",
	props = {
		["label"] = "what it counts",
		["value"] = "the number, as text",
		["unit"] = "small text after the value (\"ms\", \"%\", \"tokens\")",
		["icon"] = "an icon beside the label",
		["delta"] = "change, e.g. \"+12%\" or \"−3\"",
		["trend"] = "up | down | flat (default from the delta's sign)",
		["good"] = "up (default) | down: which direction is good, for the delta's colour",
		["detail"] = "muted line under the value, e.g. \"vs last week\"",
		["msg"] = "message sent on a click",
		["to"] = "the mod to send `msg` to",
		["selected"] = "highlight it (a chosen filter)",
		["width"] = "pixels (default: as wide as its parent gives)",
		["[children]"] = "anything under the value: a sparkline, a meter",
	},
	styles = {
		[".stat-tile"] = {
			gap = 6,
			padding = { 12, 14 },
			radius = 10,
			border = "all",
			border_color = "border",
			background = "background",
		},
		[".stat-tile:hover"] = { border_color = "accent/40" },
		[".stat-tile:selected"] = { border_color = "accent", background = "accent/6" },
		[".stat-tile-label"] = { size = "small", color = "muted-foreground", weight = "medium" },
		[".stat-tile-icon"] = { size = "small", color = "muted-foreground" },
		[".stat-tile-value"] = { size = "title", weight = "bold" },
		[".stat-tile-unit"] = { size = "small", color = "muted-foreground" },
		[".stat-tile-delta"] = { gap = 2, radius = 999, padding = { 1, 6 } },
		[".stat-tile-delta-text"] = { size = "small", weight = "medium" },
		[".stat-tile-good"] = { background = "success/12", color = "success" },
		[".stat-tile-bad"] = { background = "danger/12", color = "danger" },
		[".stat-tile-flat"] = { background = "muted-foreground/12", color = "muted-foreground" },
		[".stat-tile-detail"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		label = "Tokens today",
		icon = "activity",
		value = "1.84M",
		delta = "+12%",
		detail = "vs. yesterday · 3 agents",
		width = 240,
	},

	build = function(props: Props)
		local trend = props.trend
		if trend == nil and props.delta then
			local first = string.sub(props.delta, 1, 1)
			trend = if first == "+" then "up" elseif first == "-" or string.sub(props.delta, 1, 3) == "−" then "down" else "flat"
		end
		local mood = "flat"
		if trend == "up" or trend == "down" then
			mood = if trend == (props.good or "up") then "good" else "bad"
		end
		local delta = props.delta
			and ui.row {
				class = { "stat-tile-delta", `stat-tile-{mood}` },
				align = "center",
				trend ~= "flat" and ui.icon { name = if trend == "up" then "arrow-up" else "arrow-down", size = "small" },
				ui.text { props.delta, class = "stat-tile-delta-text" },
			}

		local children = {}
		for _, child in ipairs(props) do
			children[#children + 1] = child
		end

		return ui.column {
			class = "stat-tile",
			shrink = false,
			width = props.width,
			msg = props.msg,
			to = props.to,
			selected = props.selected,
			ui.row {
				align = "center",
				gap = "small",
				props.icon and ui.icon { name = props.icon :: any, class = "stat-tile-icon" },
				ui.text { props.label, class = "stat-tile-label", truncate = true },
			},
			ui.row {
				align = "end",
				gap = "small",
				ui.text { props.value, class = "stat-tile-value" },
				props.unit and ui.text { props.unit, class = "stat-tile-unit" },
				ui.spacer(),
				delta,
			},
			props.detail and ui.text { props.detail, class = "stat-tile-detail" },
			table.unpack(children),
		}
	end,
}
