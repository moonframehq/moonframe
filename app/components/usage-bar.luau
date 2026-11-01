-- A quota block, as in a usage sheet: "Weekly 97% left" and when it resets
-- on one line, a bar of what is left with a tick at where steady use would
-- be by now (the pace), and a note under it. The bar turns warning, then
-- danger, as little is left.

type Props = {
	title: string?,
	left: number,
	pace: number?,
	resets: string?,
	note: string?,
	width: number?,
	warn: number?,
	danger: number?,
}

return component {
	title = "Usage Bar",
	category = "Data",
	icon = "battery-medium",
	description = "A quota limit: title and \"97% left\", reset time, a bar of what is left with a pace marker, and a note; its tone follows what is left.",
	when = "A rate limit or quota with a reset (an AI plan's weekly or 5-hour window, an API budget), in a usage sheet or settings page. For a plain fraction use `meter`; for task progress use `progress`; for a single big number use `stat-tile`.",
	props = {
		["title"] = "the window's name, e.g. \"Weekly\"",
		["left"] = "fraction left, 0 to 1",
		["pace"] = "0 to 1: where a tick marks steady use by now (as a fraction of the bar)",
		["resets"] = "e.g. \"Resets in 4d 6h\"",
		["note"] = "muted line under the bar, e.g. \"36% in reserve · Lasts until reset\"",
		["width"] = "bar width in pixels (default 300)",
		["warn"] = "left fraction under which it turns warning (default 0.25)",
		["danger"] = "left fraction under which it turns danger (default 0.1)",
	},
	styles = {
		[".usage-bar"] = { gap = 6 },
		[".usage-bar-title"] = { weight = "semibold" },
		[".usage-bar-resets"] = { size = "small", color = "muted-foreground" },
		[".usage-bar-note"] = { size = "small", color = "muted-foreground" },
		[".usage-bar-track"] = { background = "muted-foreground/20", radius = 999 },
		[".usage-bar-fill"] = { background = "accent", radius = 999 },
		[".usage-bar-fill-warning"] = { background = "warning" },
		[".usage-bar-fill-danger"] = { background = "danger" },
		[".usage-bar-pace"] = { background = "foreground", radius = 1 },
	},
	example = {
		title = "Weekly",
		left = 0.97,
		pace = 0.61,
		resets = "Resets in 4d 6h",
		note = "36% in reserve · Lasts until reset",
		width = 320,
	},

	build = function(props: Props)
		local width = props.width or 300
		local left = math.clamp(props.left, 0, 1)
		local fill = { "usage-bar-fill" }
		if left < (props.danger or 0.1) then
			table.insert(fill, "usage-bar-fill-danger")
		elseif left < (props.warn or 0.25) then
			table.insert(fill, "usage-bar-fill-warning")
		end
		local percent = math.floor(left * 100 + 0.5)

		-- The pace tick floats over the track, taller than it.
		local height = 6
		local pace = props.pace and math.clamp(props.pace, 0, 1)
		local bar = ui.row {
			class = "usage-bar-track",
			width = width,
			height = height,
			shrink = false,
			ui.row { class = fill, width = left * width, height = height },
			pace and ui.row {
				class = "usage-bar-pace",
				width = 2,
				height = height + 8,
				float = { left = math.floor(pace * width) - 1, top = -4 },
			},
		}

		return ui.column {
			class = "usage-bar",
			width = width,
			ui.row {
				align = "center",
				gap = "small",
				ui.text { `{props.title or "Usage"} {percent}% left`, class = "usage-bar-title" },
				ui.spacer(),
				props.resets and ui.text { props.resets, class = "usage-bar-resets" },
			},
			ui.row { height = 14, align = "center", bar },
			props.note and ui.text { props.note, class = "usage-bar-note" },
		}
	end,
}
