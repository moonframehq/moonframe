-- A timeline of tasks over days: names on the left, a header of months
-- and days, and a bar per task from its first day to its last, filled as
-- far as it has got, with arrows from each task to those that wait on it,
-- milestones as diamonds, weekends shaded and a line on today. Bars drag
-- sideways to move and stretch by their right edge; the mod keeps the
-- dates. Group headings ("In Progress 3") divide the rows.

local avatar = use "avatar"

type Day = { label: string?, month: string?, weekend: boolean?, today: boolean? }

type Props = {
	days: { Day },
	rows: { any },
	deps: { any }?,
	day_width: number?,
	row_height: number?,
	label_width: number?,
	selected: string?,
	event: string?,
	move: string?,
	resize: string?,
	moving: any,
	height: number?,
	grow: boolean?,
	to: string?,
}

-- The rows and days of the preview and the usage example.
local EXAMPLE_DAYS: { any } = {}
for day = 1, 21 do
	local weekday = (day + 2) % 7
	table.insert(EXAMPLE_DAYS, {
		label = tostring(day + 9),
		month = if day == 1 then "October 2026" else nil,
		weekend = weekday == 5 or weekday == 6,
		today = day == 8,
	})
end
local EXAMPLE_ROWS: { any } = {
	{ heading = "In Progress", count = 2, color = "#f59e0b" },
	{ id = "nav", label = "Finalize navigation", start = 2, finish = 9, progress = 0.6, color = "#8b5cf6", avatar = "Tanjim Islam" },
	{ id = "layouts", label = "Responsive layouts", start = 8, finish = 14, progress = 0.2, color = "#3b82f6", avatar = "Emma Wilson" },
	{ heading = "To Do", count = 2, color = "#94a3b8" },
	{ id = "handoff", label = "Developer handoff", start = 15, finish = 19, color = "#14b8a6", avatar = "Sarah Chen" },
	{ id = "launch", label = "Beta launch", start = 20, milestone = true, color = "#ef4444" },
}

-- Where each task's bar is: its row and its first and last pixel.
type Span = { row: number, left: number, right: number }

-- A task's first and last day after the drag the mod passes as `moving`.
local function span_of(task: any, moving: any): (number, number)
	local start = task.start
	local finish = task.finish or task.start
	if moving and moving.id == task.id then
		local days = moving.days or 0
		if moving.resize then
			finish = math.max(start, finish + days)
		else
			start += days
			finish += days
		end
	end
	return start, finish
end

-- An elbow from the end of one bar to the start of another, a row or more
-- below or above: out to the right, down between the rows, back to the left
-- if the next one starts earlier, then into it.
local function elbow(from: Span, to: Span, height: number): { number }
	local y1 = (from.row - 0.5) * height
	local y2 = (to.row - 0.5) * height
	local out = from.right + 8
	if to.left - 8 >= out then
		return { from.right, y1, out, y1, out, y2, to.left, y2 }
	end
	local between = if to.row > from.row then (to.row - 1) * height else from.row * height - height
	return { from.right, y1, out, y1, out, between, to.left - 10, between, to.left - 10, y2, to.left, y2 }
end

return component {
	title = "Gantt",
	category = "Data",
	icon = "chart-gantt",
	description = "Tasks as bars over days: names on the left, months and days on top, progress inside each bar, dependency arrows, milestones, weekends shaded and a line on today; bars drag to move and stretch.",
	when = "A project's schedule or timeline: when tasks run, how far they are, what waits on what. For events placed in hours use `week-calendar`; for a dated list of events use `timeline`; for a board by status use `kanban-column`.",
	props = {
		["days"] = "the columns, oldest first: { label (\"12\"), month? (shown from this day on, \"October 2026\"), weekend?, today? }; a column can stand for a week too",
		["rows"] = "a task { id, label, start, finish (column indexes from 1, the last included), progress? (0 to 1), color?, avatar? (a name), done?, milestone? (a diamond on `start`) }, or a heading { heading, count?, color? }",
		["deps"] = "arrows from a task to one that waits on it: { from = id, to = id }",
		["day_width"] = "pixels per column (default 32)",
		["row_height"] = "pixels per row (default 36)",
		["label_width"] = "pixels for the names (default 220)",
		["selected"] = "id of the chosen task",
		["event"] = "key of the message a task's name sends, default \"select\": { select = id }",
		["move"] = "key of the message a bar's drag sends, default \"move\": { move = id, phase, value }, `value` the whole columns moved; on `up` with `value = 0` it was a click",
		["resize"] = "key of the message a drag of a bar's right edge sends, default \"resize\": the same, for the last day",
		["moving"] = "the drag in progress, drawn without changing the dates: { id, days, resize? }; apply it on `up`",
		["height"] = "pixels: the rows scroll within it",
		["grow"] = "fill the parent and scroll the rows",
		["to"] = "mod to send them to, if not the drawing one",
	},
	styles = {
		[".gantt"] = { border = true, border_color = "border", radius = 10, clip = true, background = "background" },
		[".gantt-names"] = { border = "right", border_color = "border", background = "background" },
		[".gantt-head"] = { border = "bottom", border_color = "border", background = "secondary" },
		[".gantt-month"] = { size = "small", weight = "semibold", padding = { 0, 8 } },
		[".gantt-day"] = { size = "small", color = "muted-foreground" },
		[".gantt-day-weekend"] = { color = "muted-foreground/60" },
		[".gantt-day-today"] = { color = "accent-foreground", background = "accent", radius = 999, weight = "semibold" },
		[".gantt-weekend"] = { background = "muted-foreground/5" },
		[".gantt-today"] = { background = "accent" },
		[".gantt-row"] = { border = "bottom", border_color = "border/60" },
		[".gantt-group"] = { background = "secondary/70" },
		[".gantt-name"] = { padding = { 0, 12 }, gap = 8 },
		[".gantt-name:hover"] = { background = "list-hover" },
		[".gantt-name:selected"] = { background = "accent/10" },
		[".gantt-heading"] = { weight = "semibold", size = "small" },
		[".gantt-count"] = { size = "small", color = "muted-foreground" },
		[".gantt-bar"] = { radius = 6, clip = true, padding = { 0, 6 }, gap = 6 },
		[".gantt-bar:selected"] = { border = true, border_color = "foreground/60" },
		[".gantt-bar-text"] = { size = "small", weight = "medium" },
		[".gantt-bar-after"] = { size = "small", color = "muted-foreground" },
		[".gantt-grip"] = { radius = 2 },
		[".gantt-milestone"] = { size = "large" },
	},
	example = {
		days = EXAMPLE_DAYS,
		rows = EXAMPLE_ROWS,
		deps = { { from = "nav", to = "layouts" }, { from = "layouts", to = "handoff" }, { from = "handoff", to = "launch" } },
		day_width = 28,
		selected = "nav",
		height = 260,
	},
	usage = [[local gantt = use "gantt"

gantt {
	days = state.days, -- { { label = "12", month = "October 2026", weekend = false, today = true }, ... }
	rows = {
		{ heading = "In Progress", count = 1 },
		{ id = "nav", label = "Finalize navigation", start = 2, finish = 9, progress = 0.6, color = "#8b5cf6", avatar = "Tanjim Islam" },
	},
	deps = { { from = "nav", to = "handoff" } },
	selected = state.selected, moving = state.moving, grow = true,
}
-- update: msg.select picks a task. msg.move and msg.resize carry the task's
-- id, `phase` and `value` (whole days): keep { id, days = value, resize }
-- as state.moving while it moves, add it to the dates on "up" and clear it.]],

	build = function(props: Props)
		local dw = props.day_width or 32
		local rh = props.row_height or 36
		local names_width = props.label_width or 220
		local days = props.days
		local rows = props.rows
		local width = #days * dw
		local body_height = #rows * rh
		local event = props.event or "select"
		local move = props.move or "move"
		local resize = props.resize or "resize"
		local to = props.to
		-- A drag's `value` is the whole columns moved since the press.
		local columns = { min = -100000, max = 100000, value = 0, speed = 1 / dw, step = 1 }

		-- The header: months over the days they start, then the days.
		local months, numbers = {}, {}
		for index, day in days do
			local left = (index - 1) * dw
			if day.month or index == 1 then
				table.insert(months, ui.row {
					float = { top = 0, left = left },
					height = 24,
					align = "center",
					ui.text { day.month or "", class = "gantt-month", truncate = true },
				})
			end
			local class = { "gantt-day" }
			if day.weekend then
				table.insert(class, "gantt-day-weekend")
			end
			table.insert(numbers, ui.row {
				width = dw,
				height = 24,
				align = "center",
				justify = "center",
				shrink = false,
				ui.row {
					class = if day.today then "gantt-day-today" else nil,
					width = 24,
					height = 20,
					align = "center",
					justify = "center",
					ui.text { day.label or "", class = class },
				},
			})
		end

		-- Behind the rows: weekends and today.
		local backdrop = {}
		for index, day in days do
			local left = (index - 1) * dw
			if day.weekend then
				table.insert(backdrop, ui.row { class = "gantt-weekend", float = { top = 0, left = left }, width = dw, height = body_height })
			end
			if day.today then
				table.insert(backdrop, ui.row { class = "gantt-today", float = { top = 0, left = left + dw / 2 - 1 }, width = 2, height = body_height })
			end
		end

		-- The rows: a name on the left, and on the right a bar or a diamond.
		local names, lanes = {}, {}
		local spans: { [string]: Span } = {}
		for index, row in rows do
			if row.heading then
				table.insert(names, ui.row {
					class = { "gantt-row", "gantt-group", "gantt-name" },
					height = rh,
					align = "center",
					shrink = false,
					row.color and ui.row { width = 8, height = 8, style = { background = row.color, radius = 4 } },
					ui.text { row.heading, class = "gantt-heading" },
					row.count and ui.text { tostring(row.count), class = "gantt-count" },
				})
				table.insert(lanes, ui.row { class = { "gantt-row", "gantt-group" }, width = width, height = rh, shrink = false })
				continue
			end
			local color = row.color or "accent"
			local start, finish = span_of(row, props.moving)
			local left = (start - 1) * dw
			local selected = props.selected == row.id
			table.insert(names, ui.row {
				class = { "gantt-row", "gantt-name" },
				height = rh,
				align = "center",
				shrink = false,
				selected = selected,
				msg = { [event] = row.id },
				to = to,
				if row.milestone
					then ui.icon { name = "diamond", size = "small", style = { color = color } }
					else ui.row { width = 8, height = 8, style = { background = color, radius = 4 } },
				ui.text { row.label, truncate = true, tone = if row.done then "muted" else nil },
			})
			local mark
			if row.milestone then
				spans[row.id] = { row = index, left = left + dw / 2 - 8, right = left + dw / 2 + 8 }
				mark = ui.row {
					float = { top = rh / 2 - 8, left = left + dw / 2 - 8 },
					width = 16,
					height = 16,
					align = "center",
					justify = "center",
					pointer = { [move] = row.id },
					range = columns,
					cursor = "grab",
					to = to,
					tooltip = row.label,
					ui.icon { name = "diamond", class = "gantt-milestone", style = { color = color } },
				}
			else
				local bar_width = math.max((finish - start + 1) * dw - 4, 8)
				spans[row.id] = { row = index, left = left + 2, right = left + 2 + bar_width }
				local inside = bar_width >= 96
				local progress = math.clamp(row.progress or (if row.done then 1 else 0), 0, 1)
				mark = ui.row {
					float = { top = 6, left = left + 2 },
					width = bar_width,
					height = rh - 12,
					ui.row {
						class = "gantt-bar",
						selected = selected,
						width = bar_width,
						height = rh - 12,
						align = "center",
						style = { background = `{color}/22` },
						pointer = { [move] = row.id },
						range = columns,
						cursor = "grab",
						to = to,
						tooltip = row.label,
						progress > 0 and ui.row {
							float = { top = 0, left = 0 },
							width = bar_width * progress,
							height = rh - 12,
							style = { background = `{color}/45` },
						},
						inside and row.avatar and avatar { name = row.avatar, size = "small", shape = "circle" },
						inside and ui.text { row.label, class = "gantt-bar-text", truncate = true },
					},
					ui.row {
						float = { top = 0, right = 0 },
						width = 8,
						height = rh - 12,
						align = "center",
						justify = "center",
						pointer = { [resize] = row.id },
						range = columns,
						cursor = "ew-resize",
						to = to,
						ui.row { class = "gantt-grip", width = 2, height = 12, style = { background = `{color}/70` } },
					},
				}
				if not inside then
					mark = ui.row {
						float = { top = 0, left = 0 },
						width = width,
						height = rh,
						mark,
						ui.row {
							float = { top = 0, left = left + bar_width + 10 },
							height = rh,
							align = "center",
							ui.text { row.label, class = "gantt-bar-after", truncate = true },
						},
					}
				end
			end
			table.insert(lanes, ui.row { class = "gantt-row", width = width, height = rh, shrink = false, mark })
		end

		-- The arrows, over the backdrop and under the bars.
		local arrows: { any } = {}
		local deps: { any } = props.deps or {}
		for _, dep in deps do
			local from, there = spans[dep.from], spans[dep.to]
			if from and there then
				table.insert(arrows, {
					points = elbow(from, there, rh),
					arrow = "end",
					radius = 5,
					stroke = 1.25,
					color = "muted-foreground/70",
				})
			end
		end

		local scrolls = props.grow or props.height ~= nil
		return ui.row {
			class = "gantt",
			gap = "none",
			grow = props.grow,
			height = props.height,
			align = "start",
			scroll = if scrolls then "y" else nil,
			ui.column {
				class = "gantt-names",
				width = names_width,
				shrink = false,
				gap = "none",
				ui.row { class = "gantt-head", height = 49, shrink = false },
				table.unpack(names),
			},
			ui.column {
				grow = true,
				scroll = "x",
				gap = "none",
				ui.column {
					width = width,
					shrink = false,
					gap = "none",
					ui.column {
						class = "gantt-head",
						width = width,
						height = 49,
						shrink = false,
						gap = "none",
						ui.row { height = 24, width = width, gap = "none", table.unpack(months) },
						ui.row { height = 24, width = width, gap = "none", table.unpack(numbers) },
					},
					ui.column {
						width = width,
						height = body_height,
						shrink = false,
						gap = "none",
						ui.row { float = { top = 0, left = 0 }, width = width, height = body_height, table.unpack(backdrop) },
						#arrows > 0 and ui.row {
							float = { top = 0, left = 0 },
							width = width,
							height = body_height,
							ui.paths { paths = arrows, width = width, height = body_height },
						},
						table.unpack(lanes),
					},
				},
			},
		}
	end,
}
