-- Devices: click a header to sort by it (again to reverse), click a row to
-- select it, tick rows (or all of them from the header); the switches
-- toggle zebra stripes and the bordered card look. Under it, tasks grouped
-- by status: fold a group from its heading, drag a task to another place
-- or group (onto a heading for its end).
local table_view = use "table"
local switch = use "switch"

local DEVICES = {
	{ id = "studio", name = "mac-studio", os = "macOS 26.1", cpu = 42, memory = 72, seen = 0 },
	{ id = "build", name = "build-01", os = "Ubuntu 24.04", cpu = 31, memory = 48, seen = 60 },
	{ id = "air", name = "macbook-air", os = "macOS 26.0", cpu = 8, memory = 61, seen = 300 },
	{ id = "pi", name = "pi-kitchen", os = "Debian 12", cpu = 3, memory = 22, seen = 7200 },
	{ id = "devbox", name = "devbox", os = "Fedora 41", cpu = 67, memory = 83, seen = 20 },
}

local function ago(seconds: number): string
	if seconds < 30 then
		return "now"
	elseif seconds < 3600 then
		return `{math.floor(seconds / 60)}m`
	end
	return `{math.floor(seconds / 3600)}h`
end

-- Tasks by status; each task is { id, title, project, due }.
local STATUSES = {
	{ id = "todo", title = "To Do", color = "#94a3b8" },
	{ id = "progress", title = "In Progress", color = "#f59e0b" },
	{ id = "done", title = "Done", color = "#22c55e" },
}

local function tasks()
	return {
		todo = {
			{ id = "web121", title = "Review responsive layouts", project = "Website", due = "Oct 15" },
			{ id = "web124", title = "Prepare developer handoff", project = "Website", due = "Oct 21" },
		},
		progress = {
			{ id = "web112", title = "Finalize navigation", project = "Website", due = "Today" },
			{ id = "mob7", title = "Push notification service", project = "Mobile App", due = "Tue" },
		},
		done = { { id = "mkt3", title = "Launch webinar deck", project = "Marketing", due = "Oct 2" } },
	}
end

-- Moves the task `id` to `group`, before the task `before` (or to its end).
local function move(board: { [string]: { any } }, id: string, group: string, before: string?)
	if id == before then
		return
	end
	local task
	for _, list in board do
		for index, item in list do
			if item.id == id then
				task = table.remove(list, index)
				break
			end
		end
	end
	if not task then
		return
	end
	local list = board[group]
	local at = #list + 1
	for index, item in list do
		if item.id == before then
			at = index
		end
	end
	table.insert(list, at, task)
end

type State = {
	tasks: any,
	folded: any,
	marker: any,
	sort: string,
	descending: boolean,
	selected: string,
	zebra: boolean,
	bordered: boolean?,
	checks: { [string]: boolean? }?,
}

return mod {
	init = function(previous: State?): State
		local state: State = previous
			or { sort = "cpu", descending = true, selected = "studio", zebra = true, tasks = tasks(), folded = {}, marker = nil }
		local first: { [string]: boolean? } = { build = true }
		state.checks = state.checks or first
		state.tasks = state.tasks or tasks()
		state.folded = state.folded or {}
		return state
	end,

	update = function(state: State, msg)
		local checks: { [string]: boolean? } = state.checks or {}
		state.checks = checks
		if msg == "toggle-zebra" then
			state.zebra = not state.zebra
		elseif msg == "toggle-bordered" then
			state.bordered = not state.bordered
		elseif type(msg) == "table" and msg.check then
			checks[msg.check] = if checks[msg.check] then nil else true
		elseif type(msg) == "table" and msg.check_all then
			-- All on unless every device already is, then all off.
			local every = true
			for _, device in DEVICES do
				every = every and checks[device.id] == true
			end
			for _, device in DEVICES do
				checks[device.id] = if every then nil else true
			end
		elseif type(msg) == "table" and msg.sort then
			if state.sort == msg.sort then
				state.descending = not state.descending
			else
				state.sort = msg.sort
				state.descending = msg.sort ~= "name" and msg.sort ~= "os"
			end
		elseif type(msg) == "table" and msg.select then
			state.selected = msg.select
		elseif type(msg) == "table" and msg.fold then
			state.folded[msg.fold] = not state.folded[msg.fold] or nil
		elseif type(msg) == "table" and msg.move ~= nil then
			-- Over a task: the gap above or below it; over a heading: the end.
			local target = if msg.move == true then nil else msg.move
			local after = target ~= nil and msg.y > 0.5
			if msg.phase == "over" then
				state.marker = if target then { id = target, after = after } else { group = msg.group }
			elseif msg.phase == "leave" then
				local marker = state.marker
				if marker and (if target then marker.id == target else marker.group == msg.group) then
					state.marker = nil
				end
			elseif msg.phase == "drop" then
				state.marker = nil
				local before = target
				if target and after then
					-- After it is before the one that follows it.
					local list = state.tasks[msg.group]
					for index, item in list do
						if item.id == target then
							before = if list[index + 1] then list[index + 1].id else nil
						end
					end
				end
				if msg.carry.row ~= target then
					move(state.tasks, msg.carry.row, msg.group, before)
				end
			end
		end
	end,

	view = function(state: State)
		local sorted = table.clone(DEVICES)
		table.sort(sorted, function(a, b)
			local x, y = a[state.sort], b[state.sort]
			if state.descending then
				return x > y
			end
			return x < y
		end)
		local rows = {}
		for index, device in sorted do
			rows[index] = {
				id = device.id,
				name = device.name,
				os = device.os,
				cpu = `{device.cpu}%`,
				memory = `{device.memory}%`,
				seen = ago(device.seen),
			}
		end
		return ui.column {
			gap = "normal",
			ui.row {
				gap = "large",
				switch { on = state.zebra, label = "Zebra stripes", small = true, msg = "toggle-zebra" },
				switch { on = state.bordered, label = "Bordered", small = true, msg = "toggle-bordered" },
			},
			table_view {
				sort = state.sort,
				descending = state.descending,
				selected = state.selected,
				zebra = state.zebra,
				bordered = state.bordered,
				checkable = true,
				checks = state.checks,
				columns = {
					{ key = "name", title = "Device" },
					{ key = "os", title = "OS", width = 110, muted = true },
					{ key = "cpu", title = "CPU", width = 56, align = "right", mono = true },
					{ key = "memory", title = "Memory", width = 72, align = "right", mono = true },
					{ key = "seen", title = "Seen", width = 56, align = "right", muted = true },
				},
				rows = rows,
			},
			ui.text { "Tasks", weight = "semibold" },
			table_view {
				id = "tasks",
				columns = {
					{ key = "title", title = "Task" },
					{ key = "project", title = "Project", width = 110, muted = true },
					{ key = "due", title = "Due", width = 70, align = "right", muted = true },
				},
				groups = (function()
					local groups = {}
					for _, status in STATUSES do
						table.insert(groups, {
							id = status.id,
							title = status.title,
							color = status.color,
							rows = state.tasks[status.id],
							collapsed = state.folded[status.id],
							add = { add = status.id },
						})
					end
					return groups
				end)(),
				add_label = "Add task",
				reorder = "move",
				marker = state.marker,
				sortable = false,
			},
		}
	end,
}
