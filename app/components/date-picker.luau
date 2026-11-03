-- A day chosen on a month calendar: the month's name with ‹ › arrows and
-- an optional Today button over a `month-grid`, and presets (Today,
-- Tomorrow, No date) beside it or under it. Drawn inline; with `toggle` it
-- is a quiet pill with the date that opens it in a popover. The mod passes
-- the weeks (it knows the calendar) and gets { select = id } or
-- { preset = id }.

local icon_button = use "icon-button"
local month_grid = use "month-grid"

type Preset = { id: string, label: string, detail: string? }

type Props = {
	title: string?,
	weeks: { any },
	selected: string?,
	weekdays: { string }?,
	event: string?,
	previous: any,
	next: any,
	today: any,
	presets: { Preset }?,
	preset: string?,
	presets_side: string?,
	open: boolean?,
	toggle: any,
	dismiss: any,
	value: string?,
	placeholder: string?,
	overdue: boolean?,
	width: number?,
	align: string?,
	to: string?,
}

-- One week of the preview, `first` the Monday's day of November 2026 (0
-- or less in October); the 3rd is today.
local function week(first: number): { any }
	local days = {}
	for offset = 0, 6 do
		local day, month = first + offset, 11
		if day < 1 then
			day, month = day + 31, 10
		elseif day > 30 then
			day, month = day - 30, 12
		end
		table.insert(days, {
			id = string.format("2026-%02d-%02d", month, day),
			day = tostring(day),
			today = month == 11 and day == 3,
			outside = month ~= 11,
		})
	end
	return days
end

-- The preview's presets, typed loosely: one has no detail.
local EXAMPLE_PRESETS: { any } = {
	{ id = "today", label = "Today", detail = "Tue" },
	{ id = "tomorrow", label = "Tomorrow", detail = "Wed" },
	{ id = "next-week", label = "Next week", detail = "Nov 9" },
	{ id = "none", label = "No date" },
}

return component {
	title = "Date Picker",
	category = "Forms",
	icon = "calendar-days",
	description = "A month calendar with its name, ‹ › arrows and Today over a `month-grid`, and presets beside it; inline, or a date pill that opens it in a popover.",
	when = "Setting a date on a record: a task's due date, a start date, a reminder, with presets for the usual ones. For the grid alone (a calendar view) use `month-grid`; for a time of day use `time-picker`; for a strip of days over a list use `day-strip`.",
	props = {
		["title"] = "the month shown, over the grid (\"November 2026\")",
		["weeks"] = "the month's weeks, passed to `month-grid`: each 7 of { id, day, today?, outside?, count? }",
		["selected"] = "id of the chosen day",
		["weekdays"] = "the grid's heading names, Monday first (default Mon … Sun)",
		["event"] = "key of the message a day sends, default \"select\": { select = id }",
		["previous"] = "message the ‹ arrow sends (show the month before)",
		["next"] = "message the › arrow sends (show the month after)",
		["today"] = "message of a Today button in the header (show this month); none when unset",
		["presets"] = "list of { id, label, detail? (muted, on the right: \"Mon\", \"Nov 9\") }: Today, Tomorrow, Next week, No date",
		["preset"] = "key of the message a preset sends, default \"preset\": { preset = id }",
		["presets_side"] = "\"left\" (default: a column beside the calendar) or \"bottom\" (a row of chips under it)",
		["toggle"] = "message the trigger pill sends; with it the calendar is a popover under a pill, else it is drawn inline",
		["open"] = "true while the popover shows",
		["dismiss"] = "message sent on a click outside or Escape",
		["value"] = "the pill's text, the date as the mod writes it (\"Nov 14\")",
		["placeholder"] = "the pill's text while there is no `value` (default \"Set date\"), muted",
		["overdue"] = "draws the pill in the danger colour (a due date that has passed)",
		["width"] = "popover width in pixels (default 300, or 450 with presets on the left)",
		["align"] = "\"start\" (default) or \"end\": which edge of the pill the popover lines up with",
		["to"] = "mod to send messages to, if not the drawing one",
	},
	styles = {
		[".date-picker"] = { gap = 6, padding = 8 },
		[".date-picker-header"] = { gap = 2, padding = { 0, 0, 4, 6 } },
		[".date-picker-title"] = { weight = "semibold" },
		[".date-picker-presets"] = { gap = 1, padding = { 8, 6 }, border = "right", border_color = "border" },
		[".date-picker-preset"] = { padding = { 5, 8 }, radius = 6, gap = 8 },
		[".date-picker-preset:hover"] = { background = "list-hover" },
		[".date-picker-detail"] = { color = "muted-foreground", size = "small" },
		[".date-picker-chips"] = { gap = 6, padding = { 4, 2, 0, 2 } },
		[".date-picker-chip"] = { border = "all", border_color = "border", radius = 999, padding = { 3, 10 }, size = "small" },
		[".date-picker-chip:hover"] = { background = "list-hover" },
		[".date-picker-trigger"] = { padding = { 3, 6 }, gap = 6, radius = 6 },
		[".date-picker-trigger:hover"] = { background = "list-hover" },
		[".date-picker-trigger:selected"] = { background = "list-hover" },
		[".date-picker-icon"] = { color = "muted-foreground" },
		[".date-picker-placeholder"] = { color = "muted-foreground" },
		[".date-picker-overdue"] = { color = "danger" },
		[".date-picker-popover"] = { padding = 0, radius = 10 },
	},
	example = {
		title = "November 2026",
		selected = "2026-11-14",
		previous = "previous",
		next = "next",
		today = "today",
		presets = EXAMPLE_PRESETS,
		weeks = { week(-5), week(2), week(9), week(16), week(23), week(30) },
	},
	usage = [[local date_picker = use "date-picker"

date_picker {
	toggle = "toggle-due", open = state.due_open, dismiss = "close-due",
	value = label(task.due), overdue = task.due < today,
	title = month_title(state.month), weeks = weeks(state.month), selected = task.due,
	previous = "previous-month", next = "next-month", today = "this-month",
	presets = { { id = "today", label = "Today" }, { id = "none", label = "No date" } },
}
-- update: msg.select sets task.due (a day's id) and closes it; msg.preset
-- sets it from the preset ("none" clears it); "previous-month" and
-- "next-month" move state.month.]],

	build = function(props: Props)
		local presets: { Preset } = props.presets or {}
		local key = props.preset or "preset"
		local bottom = props.presets_side == "bottom"

		local calendar = ui.column {
			gap = "small",
			grow = true,
			ui.row {
				class = "date-picker-header",
				align = "center",
				ui.text { props.title or "", class = "date-picker-title" },
				ui.spacer(),
				props.today ~= nil and ui.button { "Today", small = true, variant = "ghost", msg = props.today, to = props.to },
				props.previous ~= nil
					and icon_button { icon = "chevron-left", tooltip = "Previous month", size = "small", msg = props.previous, to = props.to },
				props.next ~= nil
					and icon_button { icon = "chevron-right", tooltip = "Next month", size = "small", msg = props.next, to = props.to },
			},
			month_grid {
				weeks = props.weeks,
				selected = props.selected,
				weekdays = props.weekdays,
				event = props.event,
				to = props.to,
			},
		}

		local body
		if #presets == 0 then
			body = ui.column { class = "date-picker", calendar }
		elseif bottom then
			local chips = {}
			for _, preset in presets do
				table.insert(chips, ui.row {
					class = "date-picker-chip",
					hug = true,
					msg = { [key] = preset.id },
					to = props.to,
					ui.text { preset.label },
				})
			end
			body = ui.column {
				class = "date-picker",
				calendar,
				ui.row { class = "date-picker-chips", wrap = true, table.unpack(chips) },
			}
		else
			local rows = {}
			for _, preset in presets do
				table.insert(rows, ui.row {
					class = "date-picker-preset",
					align = "center",
					msg = { [key] = preset.id },
					to = props.to,
					ui.text { preset.label, truncate = true },
					ui.spacer(),
					preset.detail and ui.text { preset.detail, class = "date-picker-detail" },
				})
			end
			body = ui.row {
				gap = "none",
				align = "stretch",
				ui.column { class = "date-picker-presets", width = 150, shrink = false, table.unpack(rows) },
				ui.column { class = "date-picker", grow = true, calendar },
			}
		end

		if props.toggle == nil then
			return body
		end

		local class = { "date-picker-trigger" }
		if props.overdue then
			table.insert(class, "date-picker-overdue")
		end
		local trigger = ui.row {
			class = class,
			align = "center",
			hug = true,
			selected = props.open == true,
			msg = props.toggle,
			to = props.to,
			ui.icon { name = "calendar", class = if props.overdue then nil else "date-picker-icon" },
			if props.value
				then ui.text { props.value, truncate = true }
				else ui.text { props.placeholder or "Set date", class = "date-picker-placeholder" },
		}
		local side_presets = #presets > 0 and not bottom
		return ui.popover {
			class = "date-picker-popover",
			open = props.open == true,
			dismiss = props.dismiss,
			to = props.to,
			side = "bottom",
			align = (props.align or "start") :: any,
			gap = 4,
			width = props.width or (if side_presets then 450 else 300),
			keys = { escape = props.dismiss },
			content = body,
			trigger,
		}
	end,
}
