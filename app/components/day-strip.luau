-- A row of days, each its date over its weekday: today in the danger
-- colour, the chosen one raised in a card. The mod passes the days (views
-- have no clock: a provider knows the date) and gets { select = id }.

type Day = { id: string, day: string, weekday: string, today: boolean?, dot: boolean? }

type Props = {
	days: { Day },
	selected: string?,
	event: string?,
	to: string?,
}

return component {
	title = "Day Strip",
	category = "Navigation",
	icon = "calendar",
	description = "Days in a row, date over weekday; today in red, the chosen day raised, a dot under days with something on.",
	when = "Picking a day in a planner, an agenda or a log, over the list for that day. For a whole month use a calendar grid; for steps of a flow use `pager`; for a few named views use `segmented`.",
	props = {
		["days"] = "list of { id, day (\"31\"), weekday (\"Sat\"), today?, dot? }",
		["selected"] = "id of the chosen day",
		["event"] = "key of the message a click sends, default \"select\": { select = id }",
		["to"] = "mod to send it to, if not the drawing one",
	},
	styles = {
		[".day-strip"] = { gap = 2 },
		[".day-strip-day"] = { padding = { 8, 0 }, radius = 12, gap = 2, border = "all", border_color = "#00000000" },
		[".day-strip-day:hover"] = { background = "list-hover" },
		[".day-strip-day:selected"] = {
			background = "card",
			border_color = "border",
			shadow = "0 1px 3px #00000014",
		},
		[".day-strip-number"] = { size = "large", weight = "medium", color = "muted-foreground" },
		[".day-strip-weekday"] = { size = "small", color = "muted-foreground" },
		[".day-strip-today"] = { color = "danger" },
		[".day-strip-dot"] = { background = "accent", radius = 2 },
	},
	example = {
		selected = "2026-10-31",
		days = {
			{ id = "2026-10-31", day = "31", weekday = "Sat", today = true, dot = false },
			{ id = "2026-11-01", day = "1", weekday = "Sun", today = false, dot = true },
			{ id = "2026-11-02", day = "2", weekday = "Mon", today = false, dot = false },
			{ id = "2026-11-03", day = "3", weekday = "Tue", today = false, dot = false },
			{ id = "2026-11-04", day = "4", weekday = "Wed", today = false, dot = true },
			{ id = "2026-11-05", day = "5", weekday = "Thu", today = false, dot = false },
		},
	},

	build = function(props: Props)
		local key = props.event or "select"
		local cells = {}
		for _, day in props.days do
			local number = { "day-strip-number" }
			local weekday = { "day-strip-weekday" }
			if day.today then
				table.insert(number, "day-strip-today")
				table.insert(weekday, "day-strip-today")
			end
			table.insert(cells, ui.column {
				class = "day-strip-day",
				width = 52,
				align = "center",
				shrink = false,
				selected = day.id == props.selected,
				msg = { [key] = day.id },
				to = props.to,
				ui.text { day.day, class = number },
				ui.text { day.weekday, class = weekday },
				ui.row { width = 4, height = 4, class = if day.dot then "day-strip-dot" else nil },
			})
		end
		return ui.row { class = "day-strip", scroll = "x", table.unpack(cells) }
	end,
}
