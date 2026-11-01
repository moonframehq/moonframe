-- A calendar leaf: the month on a band over the day's number, for a dated
-- item in a list ("NOV / 2"); the band takes a tone when it is close.

type Props = {
	month: string,
	day: string,
	tone: string?,
	size: string?,
}

return component {
	title = "Date Tile",
	category = "Data",
	icon = "calendar",
	description = "A small calendar leaf: the month on a coloured band over the day's number.",
	when = "Beside dated items in a list (deadlines, calls, distributions, events) where the date leads. For a date inside a line of text use plain text; for picking a day use `day-strip` or `month-grid`.",
	props = {
		["month"] = "the month, short (\"Nov\"); shown in capitals",
		["day"] = "the day's number",
		["tone"] = "the band: neutral (default, dark), danger or accent",
		["size"] = "\"normal\" (default, 48 px) or \"small\" (38 px)",
	},
	styles = {
		[".date-tile"] = { radius = 10, border = true, border_color = "border", background = "background", gap = 0, shadow = "0 1px 2px #0000000f" },
		[".date-tile-band"] = { background = "foreground", color = "background", padding = { 1, 0 } },
		[".date-tile-danger"] = { background = "danger", color = "#ffffff" },
		[".date-tile-accent"] = { background = "accent", color = "#ffffff" },
		[".date-tile-month"] = { size = "small", weight = "bold", case = "upper" },
		[".date-tile-day"] = { size = "large", weight = "medium" },
	},
	example = { month = "Nov", day = "9", tone = "danger" },

	build = function(props: Props)
		local small = props.size == "small"
		local band = { "date-tile-band" }
		if props.tone and props.tone ~= "neutral" then
			table.insert(band, `date-tile-{props.tone}`)
		end
		return ui.column {
			class = "date-tile",
			width = if small then 38 else 48,
			shrink = false,
			self_align = "start",
			style = { radius = 10 },
			ui.row {
				class = band,
				justify = "center",
				style = { radius = 9 },
				ui.text { props.month, class = "date-tile-month" },
			},
			ui.row {
				justify = "center",
				height = if small then 26 else 34,
				align = "center",
				ui.text { props.day, class = "date-tile-day" },
			},
		}
	end,
}
