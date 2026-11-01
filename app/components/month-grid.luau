-- A month as a grid of weeks, Monday first: today in the danger colour, the
-- chosen day raised, days of the months around dimmed, and a dot per thing
-- on that day (up to three). The mod passes the weeks (a provider knows the
-- calendar) and gets { select = id }.

type Day = { id: string, day: string, today: boolean?, outside: boolean?, count: number? }

type Props = {
	weeks: { { Day } },
	selected: string?,
	weekdays: { string }?,
	event: string?,
	to: string?,
}

local WEEKDAYS: { string } = { "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun" }

return component {
	title = "Month Grid",
	category = "Navigation",
	icon = "calendar",
	description = "A month of days in weeks, Monday first; today in red, the chosen day raised, other months dimmed, dots for what is on.",
	when = "A calendar view or a date picker's body: choosing a day in a month. For a strip of days over a list use `day-strip`; for events in order use `timeline`.",
	props = {
		["weeks"] = "list of weeks, each 7 of { id, day (\"31\"), today?, outside? (another month), count? (dots) }",
		["selected"] = "id of the chosen day",
		["weekdays"] = "the heading's names, Monday first (default Mon … Sun)",
		["event"] = "key of the message a click sends, default \"select\": { select = id }",
		["to"] = "mod to send it to, if not the drawing one",
	},
	styles = {
		[".month-grid"] = { gap = 4 },
		[".month-grid-week"] = { gap = 4 },
		[".month-grid-weekday"] = { size = "small", color = "muted-foreground", weight = "medium" },
		[".month-grid-day"] = { radius = 10, padding = { 8, 0 }, gap = 4, border = "all", border_color = "#00000000" },
		[".month-grid-day:hover"] = { background = "list-hover" },
		[".month-grid-day:selected"] = { background = "card", border_color = "border", shadow = "0 1px 3px #00000014" },
		[".month-grid-number"] = { weight = "medium" },
		[".month-grid-outside"] = { opacity = 0.35 },
		[".month-grid-today"] = { color = "danger" },
		[".month-grid-dots"] = { gap = 3 },
		[".month-grid-dot"] = { background = "accent", radius = 2 },
	},
	example = {
		selected = "2026-10-14",
		weeks = {
			{
				{ id = "2026-09-28", day = "28", today = false, outside = true, count = 0 },
				{ id = "2026-09-29", day = "29", today = false, outside = true, count = 0 },
				{ id = "2026-09-30", day = "30", today = false, outside = true, count = 0 },
				{ id = "2026-10-01", day = "1", today = false, outside = false, count = 1 },
				{ id = "2026-10-02", day = "2", today = false, outside = false, count = 0 },
				{ id = "2026-10-03", day = "3", today = false, outside = false, count = 2 },
				{ id = "2026-10-04", day = "4", today = false, outside = false, count = 0 },
			},
			{
				{ id = "2026-10-12", day = "12", today = false, outside = false, count = 0 },
				{ id = "2026-10-13", day = "13", today = true, outside = false, count = 3 },
				{ id = "2026-10-14", day = "14", today = false, outside = false, count = 1 },
				{ id = "2026-10-15", day = "15", today = false, outside = false, count = 0 },
				{ id = "2026-10-16", day = "16", today = false, outside = false, count = 0 },
				{ id = "2026-10-17", day = "17", today = false, outside = false, count = 0 },
				{ id = "2026-10-18", day = "18", today = false, outside = false, count = 1 },
			},
		},
	},

	build = function(props: Props)
		local key = props.event or "select"
		local heading = {}
		local names: { string } = props.weekdays or WEEKDAYS
		for _, name in names do
			table.insert(heading, ui.row { grow = true, justify = "center", ui.text { name, class = "month-grid-weekday" } })
		end
		local rows = { ui.row { class = "month-grid-week", table.unpack(heading) } }
		for _, week in props.weeks do
			local cells = {}
			for _, day in week do
				local class = { "month-grid-day" }
				if day.outside then
					table.insert(class, "month-grid-outside")
				end
				local dots = {}
				for _ = 1, math.min(day.count or 0, 3) do
					table.insert(dots, ui.row { class = "month-grid-dot", width = 4, height = 4 })
				end
				table.insert(cells, ui.column {
					class = class,
					grow = true,
					align = "center",
					selected = day.id == props.selected,
					msg = { [key] = day.id },
					to = props.to,
					ui.text {
						day.day,
						class = if day.today then { "month-grid-number", "month-grid-today" } else "month-grid-number",
					},
					ui.row { class = "month-grid-dots", height = 4, table.unpack(dots) },
				})
			end
			table.insert(rows, ui.row { class = "month-grid-week", table.unpack(cells) })
		end
		return ui.column { class = "month-grid", table.unpack(rows) }
	end,
}
