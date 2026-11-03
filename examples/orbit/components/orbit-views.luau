-- Four presentations of the same tasks; all actions return to workspace.
local kanban = use "kanban-column"
local card = use "board-card"
local table_view = use "table"
local calendar = use "week-calendar"
local gantt = use "gantt"
local priority = use "priority-icon"
local status = use "status-icon"
local avatar = use "avatar"

return component {
	title = "Orbit project views", category = "Data", icon = "layout-dashboard",
	description = "The task manager's connected board, table, calendar and timeline.",
	when = "Orbit's project workspace.", props = { state = "workspace state", tasks = "filtered tasks", groups = "statuses", dates = "calendar data" },
	example = { state = { view = "board", folded = {} }, tasks = {}, groups = {}, dates = {} },
	build = function(props: any)
		local state, tasks, dates = props.state, props.tasks, props.dates
		if state.view == "calendar" then
			local events = {}
			for _, task in tasks do
				table.insert(events, { id = task.id, day = task.date, title = task.title,
					start = task.hour, finish = task.hour + 1, caption = task.owner, mark = "circle-check" })
			end
			return calendar {
				view = state.calendar_view, title = dates.title, days = dates.days, weeks = dates.weeks,
				weekdays = { "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun" },
				events = events, selected = state.selected, event = "select", switch = "calendar_view", pick_day = "calendar_day",
				move = "calendar_drop", over = state.day_over, previous = "calendar-prev", next = "calendar-next", today = "calendar-today",
				hours = { from = 8, to = 18 }, hour_height = 64, column_width = 170, day_width = 800, grow = true,
			}
		end
		if state.view == "timeline" then
			local rows, visible, deps = {}, {}, {}
			for _, task in tasks do
				visible[task.id] = true
				table.insert(rows, { id = task.id, label = task.title, start = task.start - dates.first + 1,
					finish = task.finish - dates.first + 1, progress = if task.status == "done" then 1 elseif task.status == "progress" then 0.45 else 0,
					color = task.color, avatar = task.owner })
			end
			for _, task in tasks do
				if task.depends and visible[task.depends] then
					table.insert(deps, { from = task.depends, to = task.id })
				end
			end
			return gantt { days = dates.timeline, rows = rows, deps = deps, selected = state.selected,
				moving = state.moving, move = "timeline_move", resize = "timeline_resize", day_width = 32, label_width = 250, grow = true }
		end
		local groups: { any } = {}
		for _, group in props.groups do
			local rows: { any } = {}
			for _, task in tasks do
				if task.status == group.id then
					if state.view == "board" then
						table.insert(rows, card { title = task.title, subtitle = task.subtitle, key = task.id,
							labels = { { label = task.label, color = task.color } }, date = task.due, owner = task.initials,
							selected = task.id == state.selected, msg = { select = task.id },
							carry = { task = task.id }, accept = { card_drop = task.id },
						})
					else
						table.insert(rows, { id = task.id, title = ui.row { gap = "small", align = "center",
							status { status = task.status, color = group.color }, ui.text { task.title, truncate = true } },
							priority = priority { priority = task.priority, label = true },
							owner = ui.row { gap = "small", align = "center", avatar { name = task.owner, size = "small" }, ui.text { task.owner, size = "small" } },
							due = task.due,
						})
					end
				end
			end
			if state.view == "board" then
				local target = state.board_over
				table.insert(groups, kanban { title = group.label, color = group.color, count = #rows, width = 250,
					over = target and target.group == group.id,
					marker = if target and target.group == group.id then target.index else nil,
					accept = { column_drop = group.id }, add = { add = group.id }, add_label = "Add task", table.unpack(rows) })
			else
				table.insert(groups, { id = group.id, title = group.label, color = group.color, rows = rows,
					collapsed = state.folded[group.id] == true, add = { add = group.id } })
			end
		end
		if state.view == "board" then
			return ui.column { grow = true, scroll = "y", ui.row { align = "start", gap = "normal", scroll = "x", shrink = false, table.unpack(groups) } }
		end
		return table_view { id = "project-tasks", groups = groups, grow = true, selected = state.selected, sortable = false,
			reorder = "table_drop", marker = state.table_over, columns = {
				{ key = "title", title = "Task" }, { key = "priority", title = "Priority", width = 100 },
				{ key = "owner", title = "Assignee", width = 160 }, { key = "due", title = "Due date", width = 90 },
			} }
	end,
}
