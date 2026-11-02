-- A time of day: the hour and the minutes each between up and down arrows,
-- AM and PM in 12-hour form, and preset times as chips under it. Every
-- control sends the whole new time, { time = { hour, minute } }, so the mod
-- only keeps what it gets.

type Props = {
	hour: number?,
	minute: number?,
	step: number?,
	clock: number?,
	presets: { any }?,
	event: string?,
	to: string?,
}

-- Hours and minutes as a 24-hour time, wrapped into one day.
local function at(hour: number, minute: number): { hour: number, minute: number }
	local total = (hour * 60 + minute) % (24 * 60)
	return { hour = total // 60, minute = total % 60 }
end

-- "9:00 AM" or "21:00".
local function label(hour: number, minute: number, twelve: boolean): string
	if not twelve then
		return string.format("%d:%02d", hour, minute)
	end
	local shown = hour % 12
	return string.format("%d:%02d %s", if shown == 0 then 12 else shown, minute, if hour < 12 then "AM" else "PM")
end

return component {
	title = "Time Picker",
	category = "Forms",
	icon = "clock",
	description = "A time of day: hour and minutes with up and down arrows, AM and PM, and preset times as chips; sends { time = { hour, minute } }.",
	when = "Choosing when: Send Later, Snooze or a reminder's \"Choose a Time…\", beside a `month-grid` for the day. For a list of preset moments alone use `menu` or `dropdown`; for a number that is not a time use `stepper`; for a day use `month-grid` or `day-strip`.",
	props = {
		["hour"] = "the hour, 0 to 23 (default 9)",
		["minute"] = "the minutes, 0 to 59 (default 0)",
		["step"] = "minutes the minute arrows move by (default 15)",
		["clock"] = "12 (default: AM and PM) or 24",
		["presets"] = "chips under it: { hour, minute, label? } (the label defaults to the time)",
		["event"] = "key of the message every control sends, default \"time\": { time = { hour, minute } }",
		["to"] = "mod to send it to, if not the drawing one",
	},
	styles = {
		[".time-picker"] = { gap = 10 },
		[".time-picker-face"] = { gap = 6 },
		[".time-picker-unit"] = { background = "muted", border = "all", border_color = "border", radius = 9, padding = { 2, 4 }, gap = 0 },
		[".time-picker-arrow"] = { radius = 6, padding = { 1, 8 }, color = "muted-foreground" },
		[".time-picker-arrow:hover"] = { background = "list-hover", color = "foreground" },
		[".time-picker-value"] = { size = "large", weight = "semibold", font = "mono" },
		[".time-picker-colon"] = { size = "large", weight = "semibold", color = "muted-foreground" },
		[".time-picker-period"] = { background = "muted", border = "all", border_color = "border", radius = 9, padding = 2, gap = 2 },
		[".time-picker-half"] = { padding = { 4, 10 }, radius = 7, color = "muted-foreground", size = "small", weight = "medium" },
		[".time-picker-half:hover"] = { color = "foreground" },
		[".time-picker-half:selected"] = { background = "background", color = "foreground", shadow = "0 1 2 #00000024" },
		[".time-picker-presets"] = { gap = 6 },
		[".time-picker-preset"] = { border = "all", border_color = "border", radius = 999, padding = { 3, 10 }, size = "small" },
		[".time-picker-preset:hover"] = { background = "list-hover" },
		[".time-picker-preset:selected"] = { background = "accent/12", border_color = "accent", color = "accent" },
	},
	example = {
		hour = 9,
		minute = 0,
		presets = {
			{ hour = 9, minute = 0, label = "Morning" },
			{ hour = 13, minute = 0, label = "Afternoon" },
			{ hour = 18, minute = 0, label = "Evening" },
		},
	},
	usage = [[local time_picker = use "time-picker"
local month_grid = use "month-grid"

ui.row { gap = "large", align = "start",
	month_grid { weeks = weeks, selected = state.day },
	time_picker { hour = state.hour, minute = state.minute, step = 15 },
}
-- update: msg.time is the new time: state.hour = msg.time.hour,
-- state.minute = msg.time.minute.]],

	build = function(props: Props)
		local hour = math.clamp(props.hour or 9, 0, 23)
		local minute = math.clamp(props.minute or 0, 0, 59)
		local step = math.max(1, props.step or 15)
		local twelve = props.clock ~= 24
		local event = props.event or "time"
		local function send(h: number, m: number)
			return { [event] = at(h, m) }
		end

		-- One unit: an arrow up, its value, an arrow down.
		local function unit(text: string, up: any, down: any, name: string)
			return ui.column {
				class = "time-picker-unit",
				align = "center",
				shrink = false,
				ui.row {
					class = "time-picker-arrow",
					msg = up,
					to = props.to,
					tooltip = `Later {name}`,
					ui.icon { name = "chevron-up", size = "small" },
				},
				ui.text { text, class = "time-picker-value" },
				ui.row {
					class = "time-picker-arrow",
					msg = down,
					to = props.to,
					tooltip = `Earlier {name}`,
					ui.icon { name = "chevron-down", size = "small" },
				},
			}
		end

		-- The minute arrows land on the step: from 9:07 up goes to 9:15.
		local below = minute - (minute % step)
		local down_minute = if below == minute then minute - step else below
		local shown_hour = if twelve then (if hour % 12 == 0 then 12 else hour % 12) else hour

		local face = ui.row {
			class = "time-picker-face",
			align = "center",
			unit(
				if twelve then tostring(shown_hour) else string.format("%02d", hour),
				send(hour + 1, minute),
				send(hour - 1, minute),
				"hour"
			),
			ui.text { ":", class = "time-picker-colon" },
			unit(string.format("%02d", minute), send(hour, below + step), send(hour, down_minute), "minutes"),
			twelve and ui.row {
				class = "time-picker-period",
				align = "center",
				shrink = false,
				ui.row {
					class = "time-picker-half",
					selected = hour < 12,
					msg = if hour < 12 then nil else send(hour - 12, minute),
					to = props.to,
					ui.text "AM",
				},
				ui.row {
					class = "time-picker-half",
					selected = hour >= 12,
					msg = if hour >= 12 then nil else send(hour + 12, minute),
					to = props.to,
					ui.text "PM",
				},
			},
		}

		local chips = {}
		local presets: { any } = props.presets or {}
		for _, preset in presets do
			local h = preset.hour or 0
			local m = preset.minute or 0
			chips[#chips + 1] = ui.row {
				class = "time-picker-preset",
				hug = true,
				selected = h == hour and m == minute,
				msg = send(h, m),
				to = props.to,
				tooltip = if preset.label then label(h, m, twelve) else nil,
				ui.text { preset.label or label(h, m, twelve) },
			}
		end

		return ui.column {
			class = "time-picker",
			face,
			#chips > 0 and ui.row { class = "time-picker-presets", wrap = true, table.unpack(chips) },
		}
	end,
}
