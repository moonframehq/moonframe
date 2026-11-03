-- A project's timeline: drag a bar to move its task, drag its right edge to
-- stretch it, click a name or a bar to pick it. The bars move by whole days;
-- the dates change when the drag ends.
local gantt = use "gantt"

-- October 12 to November 15, 2026; Monday the 12th is the first column.
local function days()
	local list = {}
	for index = 1, 35 do
		local date = 11 + index
		local month = if date > 31 then "November 2026" else nil
		local label = if date > 31 then date - 31 else date
		table.insert(list, {
			label = tostring(label),
			month = if index == 1 then "October 2026" elseif date == 32 then month else nil,
			weekend = index % 7 == 6 or index % 7 == 0,
			today = index == 9,
		})
	end
	return list
end

local TASKS: { any } = {
	{ id = "brief", status = "done", label = "Write the brief", start = 1, finish = 4, color = "#22c55e", avatar = "Lena Fischer" },
	{ id = "wire", status = "progress", label = "Homepage wireframes", start = 3, finish = 10, progress = 0.7, color = "#8b5cf6", avatar = "Tanjim Islam" },
	{ id = "nav", status = "progress", label = "Finalize navigation", start = 8, finish = 14, progress = 0.35, color = "#3b82f6", avatar = "Sarah Chen" },
	{ id = "layouts", status = "todo", label = "Responsive layouts", start = 15, finish = 21, color = "#14b8a6", avatar = "Emma Wilson" },
	{ id = "handoff", status = "todo", label = "Developer handoff", start = 22, finish = 25, color = "#f59e0b", avatar = "John Carter" },
	{ id = "launch", status = "todo", label = "Beta launch", start = 28, milestone = true, color = "#ef4444" },
}

local GROUPS = {
	{ id = "progress", heading = "In Progress", color = "#f59e0b" },
	{ id = "todo", heading = "To Do", color = "#94a3b8" },
	{ id = "done", heading = "Done", color = "#22c55e" },
}

return mod {
	init = function(previous): any
		local state: any = previous or { selected = "nav" }
		if not state.tasks then
			state.tasks = {}
			for _, task in TASKS do
				state.tasks[task.id] = table.clone(task)
			end
		end
		return state
	end,

	update = function(state: any, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.selected = msg.select
			return
		end
		local id = msg.move or msg.resize
		if not id then
			return
		end
		local value = msg.value or 0
		if msg.phase == "up" then
			state.moving = nil
			local task = state.tasks[id]
			if value == 0 then
				state.selected = id
			elseif msg.resize then
				task.finish = math.max(task.start, task.finish + value)
			else
				task.start += value
				if task.finish then
					task.finish += value
				end
			end
		else
			state.moving = { id = id, days = value, resize = msg.resize ~= nil }
		end
	end,

	view = function(state: any)
		local rows: { any } = {}
		for _, group in GROUPS do
			local members = {}
			for _, task in TASKS do
				if task.status == group.id then
					table.insert(members, state.tasks[task.id])
				end
			end
			table.insert(rows, { heading = group.heading, color = group.color, count = #members })
			for _, task in members do
				table.insert(rows, task)
			end
		end
		return gantt {
			days = days(),
			rows = rows,
			deps = {
				{ from = "brief", to = "wire" },
				{ from = "wire", to = "layouts" },
				{ from = "nav", to = "handoff" },
				{ from = "handoff", to = "launch" },
			},
			selected = state.selected,
			moving = state.moving,
			day_width = 30,
			height = 360,
		}
	end,
}
