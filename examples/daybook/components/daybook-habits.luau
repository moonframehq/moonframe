-- Habits view: daily habit tracker with streak badges, goal progress bars,
-- quick-increment buttons, and a clickable 7-day check-in matrix.

local pill = use "pill"
local progress = use "progress"
local task_check = use "task-check"

type Props = {
	planner: any,
	tags_map: { [string]: string },
	selected_id: string?,
	new_habit: string,
}

return component {
	title = "Daybook Habits",
	category = "Daybook",
	icon = "zap",
	description = "Daily habits with streak badges, progress tracking, and 7-day check-in grid.",
	styles = {
		[".hab-title"] = { size = "title", weight = "bold" },
		[".hab-sub"] = { color = "muted-foreground" },
		[".hab-summary"] = { radius = 14, padding = { 10, 14 }, gap = 8, border = true, border_color = "border", background = "list-hover" },
		[".hab-add"] = { background = "surface-soft", radius = 12, padding = { 6, 12 }, gap = 10 },
		[".hab-bare"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		[".hab-card"] = { background = "surface-soft", radius = 14, padding = { 14, 16 }, gap = 10, border = "all", border_color = "#00000000" },
		[".hab-card:hover"] = { border_color = "border" },
		[".hab-card:selected"] = { background = "card", border_color = "foreground/70", shadow = "0 2px 8px #00000012" },
		[".hab-name"] = { weight = "semibold", size = "normal" },
		[".hab-done"] = { color = "muted-foreground" },
		[".hab-days"] = { gap = 6 },
		[".hab-day-col"] = { gap = 4 },
		[".hab-day-label"] = { size = "small", color = "muted-foreground" },
		[".hab-day-today"] = { size = "small", color = "danger", weight = "medium" },
		[".hab-dot"] = { radius = 12, border = "all", border_color = "border", background = "background" },
		[".hab-dot:hover"] = { border_color = "foreground/50" },
		[".hab-dot-done"] = { background = "success", border_color = "success", color = "#ffffff" },
		[".hab-step"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 2, 8 }, size = "small", weight = "medium" },
		[".hab-step:hover"] = { background = "list-hover" },
		[".hab-empty"] = { color = "muted-foreground", padding = { 8, 4 } },
	},
	example = { planner = { items = {}, week_history = {} }, tags_map = {}, new_habit = "" },

	build = function(props: Props)
		local planner = props.planner
		local tags_map = props.tags_map or {}
		local history: { any } = planner.week_history or {}

		local habits: { any } = {}
		local completed_today = 0
		local best_streak = 0
		for _, item in planner.items or {} do
			if item.habit then
				table.insert(habits, item)
				if item.done then
					completed_today += 1
				end
				if (item.streak or 0) > best_streak then
					best_streak = item.streak or 0
				end
			end
		end

		local cards: { Element } = {}
		for _, habit: any in habits do
			local done_set: { [string]: boolean } = {}
			for _, d in habit.done_dates or {} do
				done_set[d] = true
			end

			local meta: { Element } = {}
			local streak = habit.streak or 0
			table.insert(meta, pill {
				label = if streak == 1 then "1 day streak" else `{streak} in a row`,
				icon = "zap",
				tone = if streak >= 3 then "danger" else "neutral",
			})
			if habit.goal and habit.goal > 1 then
				local prog = habit.progress or 0
				table.insert(meta, pill { label = `Daily · {prog}/{habit.goal}`, tone = "neutral" })
				local step_val = if habit.goal >= 50 then 10 elseif habit.goal >= 10 then 5 else 1
				table.insert(meta, ui.row {
					class = "hab-step",
					msg = { habit_step = habit.id, delta = step_val },
					tooltip = `Add +{step_val} toward daily goal`,
					ui.text { `+{step_val}`, size = "small", weight = "medium" },
				})
			end
			if habit.time then
				table.insert(meta, pill { label = habit.time, icon = "clock", tone = "neutral" })
			end
			for _, t in habit.tags or {} do
				table.insert(meta, pill { label = t, color = tags_map[t] })
			end

			local day_cells: { Element } = {}
			for _, d in history do
				local checked = done_set[d.id] == true
				table.insert(day_cells, ui.column {
					class = "hab-day-col",
					align = "center",
					ui.text { string.sub(d.weekday or "", 1, 2), class = if d.today then "hab-day-today" else "hab-day-label" },
					ui.row {
						class = if checked then { "hab-dot", "hab-dot-done" } else "hab-dot",
						width = 24,
						height = 24,
						align = "center",
						justify = "center",
						tooltip = `{d.weekday} {d.day}`,
						msg = { habit_checkin = habit.id, date = d.id },
						checked and ui.icon { name = "check", size = "small" },
					},
				})
			end

			local bar: Element? = nil
			if habit.goal and habit.goal > 1 then
				local ratio = math.min(1, (habit.progress or 0) / habit.goal)
				bar = progress { value = ratio }
			end

			table.insert(cards, ui.column {
				class = "hab-card",
				selected = props.selected_id == habit.id,
				msg = { inspect = habit.id },
				ui.row {
					align = "center",
					gap = "normal",
					task_check { state = habit.state, msg = { toggle = habit.id } },
					ui.column {
						grow = true,
						gap = "small",
						ui.text {
							habit.title,
							class = if habit.done then { "hab-name", "hab-done" } else "hab-name",
							strike = habit.done,
						},
						ui.row { align = "center", gap = "small", wrap = true, table.unpack(meta) },
					},
					ui.row { class = "hab-days", align = "center", shrink = false, table.unpack(day_cells) },
				},
				bar,
			})
		end

		return ui.column {
			grow = true,
			gap = "normal",
			ui.column {
				gap = "none",
				ui.text { "Habits", class = "hab-title" },
				ui.text { `{completed_today} of {#habits} completed today`, class = "hab-sub" },
			},
			ui.row {
				class = "hab-summary",
				align = "center",
				wrap = true,
				pill { label = `{#habits} daily habits`, icon = "zap", tone = "neutral", size = "normal" },
				pill { label = `{completed_today} done today`, icon = "circle-check", tone = "success", size = "normal" },
				pill { label = `Best streak: {best_streak}d`, icon = "sparkles", tone = "warning", size = "normal" },
			},
			ui.row {
				class = "hab-add",
				align = "center",
				ui.icon { name = "plus", size = "small", tone = "muted" },
				ui.input {
					id = "new-habit",
					value = props.new_habit,
					placeholder = "Add a daily habit (e.g. Meditate 10 min)…",
					change = "habit-typing",
					submit = "habit-add",
					grow = true,
					class = "hab-bare",
				},
			},
			ui.column {
				grow = true,
				scroll = true,
				gap = "small",
				if #cards == 0 then ui.text { "No daily habits yet. Add one above to start building streaks.", class = "hab-empty" } else nil,
				table.unpack(cards),
			},
		}
	end,
}
