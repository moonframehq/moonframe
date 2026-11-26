-- Calendar view: switch between 3-day hourly time-blocking, Week hourly grid,
-- and full Month view, with drag-to-reschedule and click-to-inspect.

local nav_buttons = use "nav-buttons"
local pill = use "pill"
local segmented = use "segmented"
local week_calendar = use "week-calendar"

type Props = {
	planner: any,
	mode: string?,
	selected_id: string?,
	over_day: string?,
	tags_map: { [string]: string },
}

local WEEKDAYS = { "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun" }

return component {
	title = "Daybook Calendar",
	category = "Daybook",
	icon = "calendar-days",
	description = "Time-blocked 3-day, Week, and Month calendar with drag-to-reschedule.",
	styles = {
		[".cal-title"] = { size = "title", weight = "bold" },
		[".cal-sub"] = { color = "muted-foreground" },
		[".cal-today-btn"] = { radius = 14, padding = { 5, 12 }, background = "list-hover", color = "muted-foreground" },
		[".cal-today-btn:hover"] = { background = "accent/12", color = "foreground" },
		[".cal-allday"] = { background = "surface-soft", radius = 10, padding = { 6, 10 }, gap = 6 },
		[".cal-allday-label"] = { size = "small", color = "muted-foreground", weight = "medium" },
		[".cal-frame"] = { border = "none", radius = 12 },
	},
	example = { planner = { items = {}, day_details = {}, month = { title = "October 2026", weeks = {} }, today = "2026-10-31" }, tags_map = {} },

	build = function(props: Props)
		local planner = props.planner
		local mode = props.mode or "3days"
		local details: { any } = planner.day_details or {}
		local tags_map = props.tags_map or {}

		local span = if mode == "week" then 7 else 3
		local visible_days: { any } = {}
		local visible_set: { [string]: boolean } = {}
		for i = 1, math.min(span, #details) do
			local d = details[i]
			visible_set[d.id] = true
			table.insert(visible_days, {
				id = d.id,
				label = if d.today then `Today {d.day}` else `{d.weekday} {d.day}`,
				today = d.today,
			})
		end

		local title = planner.month and planner.month.title or "Calendar"
		local subtitle = planner.subtitle or ""
		if mode ~= "month" and #details >= 1 then
			local first = details[1]
			local last = details[math.min(span, #details)]
			title = `{first.short_date} – {last.day}`
			subtitle = string.sub(first.id or "", 1, 4)
		end

		local events: { any } = {}
		local untimed: { any } = {}
		for _, item in planner.items or {} do
			if item.kind ~= "note" and item.date then
				local first_tag = item.tags and item.tags[1]
				if item.start_hour ~= nil then
					table.insert(events, {
						id = item.id,
						day = item.date,
						start = item.start_hour,
						finish = item.end_hour or (item.start_hour + 1),
						title = item.title,
						caption = first_tag,
						time = item.time,
						mark = if item.kind == "event" then "calendar" elseif item.state == "done" then "circle-check" else "circle",
						tooltip = `{item.title}{if item.time then ` · {item.time}` else ""}`,
					})
				else
					-- Untimed tasks show in Month chips and in the All-day strip.
					if mode == "month" then
						table.insert(events, {
							id = item.id,
							day = item.date,
							start = 9,
							finish = 10,
							title = item.title,
							mark = if item.state == "done" then "circle-check" else "circle",
						})
					elseif visible_set[item.date] and item.state ~= "done" then
						table.insert(untimed, item)
					end
				end
			end
		end

		local back_msg: any = if mode == "month" then { month = -1 } elseif mode == "week" then { shift = -7 } else { shift = -3 }
		local next_msg: any = if mode == "month" then { month = 1 } elseif mode == "week" then { shift = 7 } else { shift = 3 }

		local header = ui.row {
			align = "start",
			ui.column {
				gap = "none",
				ui.text { title, class = "cal-title" },
				ui.text { subtitle, class = "cal-sub" },
			},
			ui.spacer(),
			ui.row {
				align = "center",
				gap = "small",
				segmented {
					selected = mode,
					event = "calendar_mode",
					small = true,
					options = {
						{ id = "3days", label = "3 days" },
						{ id = "week", label = "Week" },
						{ id = "month", label = "Month" },
					},
				},
				ui.row {
					class = "cal-today-btn",
					msg = { select = planner.today },
					ui.text { "Today", size = "small", weight = "medium" },
				},
				nav_buttons { back = back_msg, forward = next_msg, to = "provider:planner", small = true },
			},
		}

		local allday_bar: Element? = nil
		if mode ~= "month" and #untimed > 0 then
			local chips: { Element } = { ui.text { "All day:", class = "cal-allday-label" } }
			for _, item in untimed do
				local tag_color = item.tags and item.tags[1] and tags_map[item.tags[1]]
				table.insert(chips, pill {
					label = `{item.title} · {item.when or ""}`,
					color = tag_color,
					tone = if tag_color then nil else "neutral",
					msg = { inspect = item.id },
				})
			end
			allday_bar = ui.row { class = "cal-allday", align = "center", wrap = true, table.unpack(chips) }
		end

		local cal_view: Element
		if mode == "month" then
			cal_view = week_calendar {
				view = "month",
				header = false,
				weekdays = WEEKDAYS,
				weeks = planner.month and planner.month.weeks or {},
				events = events,
				selected = props.selected_id,
				event = "inspect",
				pick_day = "select",
				move = "cal_move",
				over = props.over_day,
				more = 3,
			}
		else
			cal_view = week_calendar {
				view = "week",
				header = false,
				days = visible_days,
				events = events,
				selected = props.selected_id,
				hours = { from = 7, to = 21 },
				hour_height = 68,
				column_width = if mode == "3days" then 166 else 144,
				gutter = 54,
				now = if planner.now_hour then { day = planner.today, hour = planner.now_hour } else nil,
				event = "inspect",
				pick_day = "select",
				move = "cal_move",
				over = props.over_day,
			}
		end

		return ui.column {
			grow = true,
			gap = "normal",
			header,
			allday_bar,
			ui.column { grow = true, class = "cal-frame", cal_view },
		}
	end,
}
