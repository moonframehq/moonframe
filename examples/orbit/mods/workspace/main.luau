-- Seeded demo data, kept by Moonframe's normal state persistence. The
-- board, table, calendar, timeline and detail panel all edit these tasks.
local views = use "orbit-views"
local detail = use "orbit-detail"
local project_card = use "project-card"
local filter_builder = use "filter-builder"
local avatar = use "avatar"
local avatar_stack = use "avatar-stack"
local progress = use "progress"

local GROUPS = {
	{ id = "todo", label = "To do", color = "#9699a8", dot = "#9699a8" },
	{ id = "progress", label = "In progress", color = "#d5a134", dot = "#d5a134" },
	{ id = "review", label = "In review", color = "#9270dc", dot = "#9270dc" },
	{ id = "done", label = "Done", color = "#4ca784", dot = "#4ca784" },
}
local PROJECTS = {
	{ id = "website", name = "Website redesign", icon = "globe", color = "#7b68d8", description = "A clearer story. A faster site. A better first impression." },
	{ id = "mobile", name = "Mobile app", icon = "laptop", color = "#4296bd", description = "Make everyday work feel effortless, wherever it happens." },
	{ id = "system", name = "Design system", icon = "layers", color = "#d58e64", description = "Thoughtful foundations for everything we build next." },
}
local PEOPLE = {
	{ id = "sarah", label = "Sarah Chen", avatar = "Sarah Chen" },
	{ id = "john", label = "John Carter", avatar = "John Carter" },
	{ id = "emma", label = "Emma Wilson", avatar = "Emma Wilson" },
	{ id = "priya", label = "Priya Patel", avatar = "Priya Patel" },
	{ id = "marcus", label = "Marcus Lee", avatar = "Marcus Lee" },
	{ id = "lena", label = "Lena Fischer", avatar = "Lena Fischer" },
	{ id = "ana", label = "Ana Ruiz", avatar = "Ana Ruiz" },
	{ id = "ben", label = "Ben Ito", avatar = "Ben Ito" },
	{ id = "cleo", label = "Cleo Park", avatar = "Cleo Park" },
	{ id = "dev", label = "Dev Shah", avatar = "Dev Shah" },
	{ id = "eli", label = "Eli Moss", avatar = "Eli Moss" },
	{ id = "fay", label = "Fay Lin", avatar = "Fay Lin" },
}
local PRIORITIES = {
	{ id = "urgent", label = "Urgent", icon = "circle-alert" },
	{ id = "high", label = "High", icon = "flag" },
	{ id = "medium", label = "Medium", icon = "flag" },
	{ id = "low", label = "Low", icon = "flag" },
}
local FIELDS: { any } = {
	{ id = "status", label = "Status", icon = "circle-dashed", operators = { "is", "is not" }, values = GROUPS },
	{ id = "assignee", label = "Assignee", icon = "user", operators = { "is", "is not" }, values = PEOPLE },
	{ id = "priority", label = "Priority", icon = "flag", operators = { "is", "is not" }, values = PRIORITIES },
	{ id = "title", label = "Title", icon = "file-text", operators = { "contains", "does not contain" } },
}
local TABS = {
	{ id = "board", label = "Board", icon = "layout-dashboard" },
	{ id = "list", label = "List", icon = "list" },
	{ id = "calendar", label = "Calendar", icon = "calendar-days" },
	{ id = "timeline", label = "Timeline", icon = "chart-gantt" },
}
local MONTHS = { "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December" }
local WEEKDAYS = { "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun" }

-- Civil dates as whole days since 1970 (Howard Hinnant's algorithms).
local function days_from(year: number, month: number, day: number): number
	local y = year - (if month <= 2 then 1 else 0)
	local era = y // 400
	local yoe = y - era * 400
	local doy = (153 * (if month > 2 then month - 3 else month + 9) + 2) // 5 + day - 1
	return era * 146097 + yoe * 365 + yoe // 4 - yoe // 100 + doy - 719468
end
local function civil(days: number): (number, number, number)
	local z = days + 719468
	local era = z // 146097
	local doe = z - era * 146097
	local yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
	local doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
	local mp = (5 * doy + 2) // 153
	local day = doy - (153 * mp + 2) // 5 + 1
	local month = if mp < 10 then mp + 3 else mp - 9
	return yoe + era * 400 + (if month <= 2 then 1 else 0), month, day
end
local function date_id(days: number): string
	local y, m, d = civil(days)
	return string.format("%04d-%02d-%02d", y, m, d)
end
local function parse_date(id: string): number
	local y, m, d = string.match(id, "^(%d+)%-(%d+)%-(%d+)$")
	return days_from(tonumber(y) :: number, tonumber(m) :: number, tonumber(d) :: number)
end
local function month_step(days: number, delta: number): number
	local y, m = civil(days)
	m += delta
	return days_from(y + (m - 1) // 12, (m - 1) % 12 + 1, 1)
end
local TODAY = days_from(2026, 11, 2) -- Deliberately fixed, labelled sample dates.

local function dates_for(at: number, mode: string): any
	local y, m = civil(at)
	local first = days_from(y, m, 1)
	local start = first - (first + 3) % 7
	local weeks, days, timeline = {}, {}, {}
	for w = 0, 5 do
		local week = {}
		for i = 0, 6 do
			local day = start + w * 7 + i
			local _, month, number = civil(day)
			table.insert(week, { id = date_id(day), day = tostring(number), today = day == TODAY, outside = month ~= m })
		end
		table.insert(weeks, week)
	end
	local monday = at - (at + 3) % 7
	for i = 0, (if mode == "day" then 0 else 6) do
		local day = if mode == "day" then at else monday + i
		local _, _, n = civil(day)
		table.insert(days, { id = date_id(day), label = `{WEEKDAYS[(day + 3) % 7 + 1]} {n}`, today = day == TODAY })
	end
	for i = 0, 34 do
		local day = first + i
		local _, month, n = civil(day)
		table.insert(timeline, { label = tostring(n), month = if i == 0 or n == 1 then MONTHS[month] else nil,
			weekend = (day + 3) % 7 >= 5, today = day == TODAY })
	end
	return { title = `{MONTHS[m]} {y}`, first = first, weeks = weeks, days = days, timeline = timeline }
end

local function find(list: { any }, id: string): any
	for _, item in list do if item.id == id then return item end end
	return nil
end

local function task_of(id: string, title: string, project: string, status: string, person: string, start: number, finish: number, label: string): any
	return { id = id, title = title, project = project, status = status, assignee = person,
		start = TODAY + start, finish = TODAY + finish, hour = 9 + finish % 7, label = label,
		priority = if finish % 3 == 0 then "high" else "medium", subtitle = "November launch · Sprint 08",
		description = `## Outcome\n{title}, with a clear handoff to the team.\n\n- Review the current experience\n- Build and test the proposed changes\n- Share a short walkthrough\n\nKeep accessibility and mobile layouts in mind.`,
		subtasks = { { title = "Review the brief", done = true }, { title = "Share a first pass", done = false }, { title = "Get team sign-off", done = false } },
		files = { { name = "design-brief.md" } },
		comments = { { author = "Emma Wilson", age = "Sample comment", body = "The direction looks good. Let's keep the mobile experience in mind.", reactions = { { emoji = "👍", others = 2, mine = false } } } },
	}
end

local function initial(): any
	local tasks = {
		task_of("WEB-101", "Audit the current experience", "website", "done", "sarah", -6, -2, "Research"),
		task_of("WEB-102", "Map the new information architecture", "website", "review", "lena", -4, 1, "Strategy"),
		task_of("WEB-103", "Create homepage wireframes", "website", "review", "sarah", -2, 3, "Design"),
		task_of("WEB-104", "Finalize navigation", "website", "progress", "john", 0, 4, "Frontend"),
		task_of("WEB-105", "Design responsive layouts", "website", "progress", "emma", 1, 7, "Design"),
		task_of("WEB-106", "Build the hero component", "website", "progress", "marcus", 2, 9, "Frontend"),
		task_of("WEB-107", "Write the homepage story", "website", "todo", "priya", 3, 8, "Content"),
		task_of("WEB-108", "Prepare developer handoff", "website", "todo", "sarah", 8, 12, "Design"),
		task_of("WEB-109", "Test keyboard navigation", "website", "todo", "john", 11, 15, "QA"),
		task_of("WEB-110", "Ship the new website", "website", "todo", "lena", 15, 17, "Launch"),
		task_of("APP-201", "Prototype onboarding", "mobile", "progress", "emma", 1, 10, "Design"),
		task_of("APP-202", "Offline sync", "mobile", "todo", "marcus", 5, 16, "Engineering"),
		task_of("APP-203", "Triage beta feedback", "mobile", "review", "sarah", 0, 6, "Research"),
		task_of("DS-301", "Refresh color tokens", "system", "done", "sarah", -5, -1, "Foundations"),
		task_of("DS-302", "Document the new components", "system", "progress", "priya", 0, 12, "Content"),
	}
	for id, dep in { ["WEB-103"] = "WEB-101", ["WEB-104"] = "WEB-102", ["WEB-106"] = "WEB-103", ["WEB-108"] = "WEB-105", ["WEB-110"] = "WEB-109" } do
		find(tasks, id :: string).depends = dep
	end
	return { version = 1, tasks = tasks, page = "website", project = "website", view = "board", selected = nil,
		query = "", folded = {}, conditions = {}, match = "all", next = 401, filter_next = 1,
		calendar_at = TODAY, calendar_view = "month", due_month = TODAY, draft = "", caret = 0, mention_at = 1,
		picker_query = "", emoji_query = "", starred = { website = true }, preview = true }
end

local function shown(state: any): { any }
	local tasks = {}
	for _, task in state.tasks do
		local scope = if state.page == "mine" then task.assignee == "sarah" else task.project == state.project
		local all, any = true, #state.conditions == 0
		for _, condition in state.conditions do
			local value = task[condition.field] or ""
			local hit = string.find(string.lower(value), string.lower(condition.value), 1, true) ~= nil
			local yes = condition.value == "" or (if condition.operator == "is" then value == condition.value
				elseif condition.operator == "is not" then value ~= condition.value
				elseif condition.operator == "contains" then hit else not hit)
			all, any = all and yes, any or yes
		end
		if scope and string.find(string.lower(task.title), string.lower(state.query), 1, true)
			and (if state.match == "all" then all else any) then table.insert(tasks, task) end
	end
	return tasks
end

local function decorated(task: any): any
	local copy = table.clone(task)
	local person = find(PEOPLE, task.assignee) or PEOPLE[1]
	local _, month, day = civil(task.finish)
	copy.owner = person.label
	copy.initials = string.sub(person.label, 1, 1) .. (string.match(person.label, " (%a)") or "")
	copy.date = date_id(task.finish)
	copy.due = `{string.sub(MONTHS[month], 1, 3)} {day}`
	copy.color = if task.label == "Design" then "#9270dc" elseif task.label == "Frontend" then "#479b9b" elseif task.label == "Content" then "#d59655" else "#6f89b8"
	return copy
end

local function close_picker(state: any)
	state.picker, state.highlighted, state.picker_query = nil, nil, ""
end
local function select_task(state: any, id: string?)
	state.selected, state.draft, state.caret = id, "", 0
	state.reaction_picker, state.mention_closed, state.mention_at = nil, false, 1
	close_picker(state)
	local task = id and find(state.tasks, id)
	if task then state.due_month = task.finish end
end
local function move_task(state: any, id: string, group: string, target: string?, after: boolean)
	if id == target then return end
	local source = find(state.tasks, id)
	if not source then return end
	table.remove(state.tasks, table.find(state.tasks, source) :: number)
	source.status = group
	local at = #state.tasks + 1
	for index, task in state.tasks do
		if target == task.id then at = index + (if after then 1 else 0); break end
		if not target and task.project == source.project and task.status == group then at = index + 1 end
	end
	table.insert(state.tasks, at, source)
end

return mod {
	keys = { escape = "close-task" } :: { [string]: any },
	init = function(previous: any): any
		local state = if previous and previous.version == 1 then previous else initial()
		state.board_over, state.table_over, state.day_over, state.moving = nil, nil, nil, nil
		state.filtering, state.filter_open, state.reaction_picker = false, nil, nil
		close_picker(state)
		return state
	end,
	update = function(state: any, msg: any)
		local task = state.selected and find(state.tasks, state.selected)
		if msg == "close-task" then select_task(state, nil)
		elseif msg == "close-picker" then close_picker(state)
		elseif msg == "filter" then state.filtering = not state.filtering
		elseif msg == "close-filter" or msg == "done" then state.filtering, state.filter_open = false, nil
		elseif msg == "clear" then state.conditions = {}
		elseif msg == "add" then
			table.insert(state.conditions, { id = `c{state.filter_next}`, field = "status", operator = "is", value = "" })
			state.filter_next += 1
		elseif msg == "new-task" or (type(msg) == "table" and msg.add) then
			local id = `TASK-{state.next}`
			state.next += 1
			table.insert(state.tasks, task_of(id, "Untitled task", state.project, if type(msg) == "table" then msg.add else "todo", "sarah", 0, 4, "Design"))
			state.page, state.query, state.conditions = state.project, "", {}
			select_task(state, id)
		elseif msg == "post-comment" or (type(msg) == "table" and msg.msg == "post-comment") then
			if task and string.find(state.draft, "%S") then
				table.insert(task.comments, { author = "Sarah Chen", age = "Just now", body = state.draft, reactions = {} })
				state.draft, state.caret = "", 0
			end
		elseif msg == "close-mentions" then state.mention_closed = true
		elseif msg == "close-reactions" then state.reaction_picker = nil
		elseif msg == "calendar-today" then state.calendar_at = TODAY
		elseif msg == "due-today" then state.due_month = TODAY
		elseif msg == "due-prev" or msg == "due-next" then state.due_month = month_step(state.due_month, if msg == "due-prev" then -1 else 1)
		elseif msg == "calendar-prev" or msg == "calendar-next" then
			local delta = if msg == "calendar-prev" then -1 else 1
			state.calendar_at = if state.view == "timeline" or state.calendar_view == "month" then month_step(state.calendar_at, delta)
				else state.calendar_at + delta * (if state.calendar_view == "week" then 7 else 1)
		elseif type(msg) ~= "table" then return
		elseif msg.page then
			state.page = msg.page
			if find(PROJECTS, msg.page) then state.project = msg.page end
			state.query, state.conditions = "", {}
			select_task(state, nil)
		elseif msg.view then
			state.view = msg.view
			if state.page == "home" then state.page = state.project end
			state.board_over, state.table_over, state.day_over, state.moving = nil, nil, nil, nil
		elseif msg.select then select_task(state, msg.select)
		elseif msg.star then state.starred[msg.star] = not state.starred[msg.star]
		elseif msg.fold then state.folded[msg.fold] = not state.folded[msg.fold]
		elseif msg.picker then
			local open = state.picker ~= msg.picker
			close_picker(state)
			if open then state.picker = msg.picker end
		elseif msg.highlight then state.highlighted = msg.highlight
		elseif msg.msg == "search" then state.query = msg.value
		elseif msg.msg == "picker-search" then state.picker_query, state.highlighted = msg.value, nil
		elseif msg.msg == "emoji-search" then state.emoji_query = msg.value
		elseif task and (msg.status or msg.priority or msg.assignee) then
			for _, field in { "status", "priority", "assignee" } do if msg[field] then task[field] = msg[field] end end
			close_picker(state)
		elseif task and msg.due then task.finish = parse_date(msg.due); task.start = math.min(task.start, task.finish); close_picker(state)
		elseif task and msg.msg == "title" then task.title = msg.value
		elseif task and msg.msg == "description" then task.description = msg.value
		elseif msg.description_view then state.preview = msg.description_view == "preview"
		elseif task and msg.subtask then task.subtasks[msg.subtask].done = not task.subtasks[msg.subtask].done
		elseif task and msg.remove_file then table.remove(task.files, msg.remove_file)
		elseif msg.browse then
			return state, { { to = "app:pick", msg = { kind = "open", multiple = true, prompt = "Attach locally", reply = { attach = msg.browse } } } }
		elseif msg.attach and msg.paths then
			local owner = find(state.tasks, msg.attach)
			if owner then for _, path in msg.paths do table.insert(owner.files, { name = string.match(path, "([^/\\]+)$") or path, path = path }) end end
		elseif msg.msg == "comment" then state.draft, state.caret, state.mention_at, state.mention_closed = msg.value, msg.caret, 1, false
		elseif msg.mention == "up" then state.mention_at = math.max(1, state.mention_at - 1)
		elseif msg.mention == "down" then state.mention_at += 1
		elseif msg.mention == "choose" then return state, { { to = "app:edit", msg = msg.edit } }
		elseif msg.reaction_picker then state.reaction_picker, state.emoji_query = msg.reaction_picker, ""
		elseif msg.open then state.filter_open = if msg.open == "" or msg.open == state.filter_open then nil else msg.open
		elseif msg.match then state.match = msg.match
		elseif msg.remove then
			for i, condition in state.conditions do if condition.id == msg.remove then table.remove(state.conditions, i); break end end
		elseif msg.set then
			local condition = find(state.conditions, msg.set.id)
			if condition then
				local value = msg.set.value or msg.value or ""
				condition[msg.set.key] = value
				if msg.set.key == "field" then condition.operator, condition.value = find(FIELDS, value).operators[1], "" end
			end
			state.filter_open = nil
		elseif msg.calendar_view then state.calendar_view = msg.calendar_view
		elseif msg.calendar_day then state.calendar_at, state.calendar_view = parse_date(msg.calendar_day), "day"
		elseif msg.calendar_drop then
			if msg.phase == "over" then state.day_over = msg.calendar_drop
			elseif msg.phase == "leave" then if state.day_over == msg.calendar_drop then state.day_over = nil end
			elseif msg.phase == "drop" then
				local moved = find(state.tasks, msg.carry.event)
				if moved then
					local finish = parse_date(msg.calendar_drop)
					moved.start, moved.finish = moved.start + finish - moved.finish, finish
					if state.calendar_view ~= "month" then moved.hour = math.clamp(math.floor((8 + msg.y * 10) * 2) / 2, 8, 17) end
				end
				state.day_over = nil
			end
		elseif msg.timeline_move or msg.timeline_resize then
			local id, delta = msg.timeline_move or msg.timeline_resize, msg.value or 0
			if msg.phase == "up" then
				local moved = find(state.tasks, id)
				if moved then
					if delta == 0 then select_task(state, id)
					elseif msg.timeline_resize then moved.finish = math.max(moved.start, moved.finish + delta)
					else moved.start, moved.finish = moved.start + delta, moved.finish + delta end
				end
				state.moving = nil
			else state.moving = { id = id, days = delta, resize = msg.timeline_resize ~= nil } end
		elseif msg.card_drop or msg.column_drop or msg.table_drop then
			local target = msg.card_drop or (if type(msg.table_drop) == "string" then msg.table_drop else nil)
			local row = target and find(state.tasks, target)
			local group = if row then row.status else msg.column_drop or msg.group
			local after = target ~= nil and (msg.y or 0) > 0.5
			if msg.phase == "drop" then
				move_task(state, msg.carry.task or msg.carry.row, group, target, after)
				state.board_over, state.table_over = nil, nil
			elseif msg.phase == "over" then
				if msg.table_drop then state.table_over = if target then { id = target, after = after } else { group = group }
				else
					local index, count = 1, 0
					for _, item in shown(state) do
						if item.status == group then count += 1; if item.id == target then index = count + (if after then 1 else 0) end end
					end
					state.board_over = { group = group, card = target, index = if target then index else count + 1 }
				end
			elseif msg.phase == "leave" then
				local over = if msg.table_drop then state.table_over else state.board_over
				if over and (if target then (over.id or over.card) == target else over.group == group and not (over.id or over.card)) then
					if msg.table_drop then state.table_over = nil else state.board_over = nil end
				end
			end
		elseif task then
			-- Each comment's reaction key includes its index; the picker uses
			-- the comment stored when it opened.
			local index, value = state.reaction_picker, msg.emoji
			for key, v in msg do
				if type(key) == "string" and string.match(key, "^react%-%d+$") then index, value = tonumber(string.match(key, "%d+")), v end
			end
			local entry = index and task.comments[index]
			if entry and value then
				local found = false
				for _, reaction in entry.reactions do
					if reaction.emoji == value then reaction.mine = if msg.emoji then true else not reaction.mine; found = true end
				end
				if not found then table.insert(entry.reactions, { emoji = value, others = 0, mine = true }) end
				state.reaction_picker = nil
			end
		end
	end,

	view = function(state: any)
		local project = find(PROJECTS, state.project) or PROJECTS[1]
		local task = state.selected and find(state.tasks, state.selected)
		local list, mentions, team = {}, {}, {}
		for _, item in shown(state) do table.insert(list, decorated(item)) end
		for _, person in PEOPLE do table.insert(mentions, { name = person.label, detail = "Orbit team" }) end
		for i = 1, 4 do table.insert(team, { name = PEOPLE[i].label }) end
		local nav = {}
		local function nav_row(label: string, icon: string, id: string, count: number?)
			return ui.row { class = "orbit-nav", align = "center", selected = state.page == id, msg = { page = id },
				ui.icon { name = icon :: any, size = "small" }, ui.text { label, size = "small" }, ui.spacer(), count and ui.text { tostring(count), size = "small", tone = "muted" } }
		end
		local cards, total, done = {}, #state.tasks, 0
		for _, item in state.tasks do if item.status == "done" then done += 1 end end
		for _, p in PROJECTS do
			local n, completed = 0, 0
			for _, item in state.tasks do if item.project == p.id then n += 1; if item.status == "done" then completed += 1 end end end
			table.insert(nav, nav_row(p.name, p.icon, p.id, n))
			table.insert(cards, project_card { name = p.name, icon = p.icon, color = p.color, description = p.description,
				status = "In progress", tone = "accent", progress = completed / math.max(1, n), tasks = `{completed}/{n}`, due = "November 2026",
				people = team, starred = state.starred[p.id], star = { star = p.id }, msg = { page = p.id }, width = 300 })
		end
		local tabs = {}
		for _, tab in TABS do table.insert(tabs, ui.row { class = "orbit-tab", selected = state.view == tab.id, align = "center", msg = { view = tab.id },
			ui.icon { name = tab.icon :: any, size = "small" }, ui.text { tab.label, size = "small" } }) end
		local body
		if state.page == "home" then
			local function stat(label: string, value: number, note: string)
				return ui.column { class = "orbit-stat", grow = true, ui.text { label, tone = "muted", size = "small" },
					ui.text { tostring(value), style = { size = "hero", weight = "semibold" } }, ui.text { note, tone = "muted", size = "small" } }
			end
			body = ui.column { grow = true, scroll = "y", ui.column { class = "orbit-body", shrink = false,
				ui.row { gap = "normal", stat("Projects", #PROJECTS, "One shared workspace"), stat("Open tasks", total - done, "Across your projects"), stat("Completed", done, "Every step counts") },
				ui.row { align = "center", ui.text { "Your projects", style = { size = "large", weight = "semibold" } }, ui.spacer(), ui.text { "Click a project to jump in", tone = "muted", size = "small" } },
				ui.row { gap = "normal", wrap = true, align = "start", table.unpack(cards) },
				ui.column { class = "orbit-stat", ui.text { "A workspace you can play with", style = { weight = "semibold" } },
					ui.text { "Move a card, change its due date, or leave a comment. Board, List, Calendar and Timeline all use the same tasks.", tone = "muted" },
					progress { value = done / math.max(total, 1) },
					ui.text { "Seeded November 2026 data. Your edits are saved locally between launches.", class = "orbit-label" },
				},
			} }
		else
			body = ui.column { grow = true, gap = "none",
				ui.row { class = "orbit-tabs", table.unpack(tabs) },
				ui.row { class = "orbit-toolbar", align = "center",
					ui.input { id = "search", value = state.query, placeholder = "Search tasks…", change = "search", width = 190, class = "orbit-search" },
					ui.popover { open = state.filtering, dismiss = "close-filter", width = 520, side = "bottom", align = "start",
						content = filter_builder { fields = FIELDS, conditions = state.conditions, match = state.match, open = state.filter_open },
						ui.button { if #state.conditions > 0 then `Filter · {#state.conditions}` else "Filter", icon = "funnel", small = true, variant = "ghost", msg = "filter" },
					},
					ui.spacer(), ui.text { `{#list} tasks`, tone = "muted", size = "small" },
					if state.view == "timeline" then ui.row { gap = "small",
						ui.button { icon = "chevron-left", tooltip = "Previous month", small = true, variant = "ghost", msg = "calendar-prev" },
						ui.text { dates_for(state.calendar_at, "month").title, size = "small" },
						ui.button { icon = "chevron-right", tooltip = "Next month", small = true, variant = "ghost", msg = "calendar-next" },
					} else avatar_stack { people = team, size = "small" },
				},
				ui.column { class = "orbit-body", grow = true,
					views { state = state, tasks = list, groups = GROUPS, dates = dates_for(state.calendar_at, state.calendar_view) },
					ui.text { if state.view == "board" then "Drag cards to reorder or change status · Esc cancels · Click a card to edit"
						elseif state.view == "timeline" then "Drag bars to reschedule · Drag their right edge to resize · Arrows show dependencies"
						elseif state.view == "calendar" then "Drag tasks to reschedule their due date · Click a task to edit"
						else "Drag rows between groups · Click a group to fold it · Click a task to edit", class = "orbit-hint" },
				},
			}
		end
		return ui.column { grow = true, gap = "none",
			ui.row { class = "orbit-chrome", drag = true, align = "center",
				ui.text { "Orbit", style = { weight = "semibold" } }, ui.text { "/", tone = "muted" },
				ui.text { "Design workspace", tone = "muted", size = "small" }, ui.spacer(),
				ui.badge { "Interactive demo", tone = "muted" },
				ui.button { icon = "sun", tooltip = "Toggle light / dark", small = true, variant = "ghost", command = "toggle-theme" },
				avatar { name = "Sarah Chen", size = "small" },
			},
			ui.row { grow = true, gap = "none",
				ui.column { class = "orbit-sidebar", width = 220, shrink = false,
					ui.row { align = "center", gap = "small", ui.icon { name = "circle-dot", class = "orbit-logo" }, ui.text { "Orbit Studio", class = "orbit-brand" } },
					ui.text { "WORKSPACE", class = "orbit-section-label" },
					nav_row("Overview", "house", "home", nil), nav_row("My tasks", "circle-check", "mine", nil),
					ui.text { "PROJECTS", class = "orbit-section-label" }, ui.column { gap = "none", table.unpack(nav) },
					ui.spacer(),
					ui.column { class = "orbit-hint", ui.text { "Made with Moonframe", size = "small" }, ui.text { "Sample data · November 2026", size = "small", tone = "muted" } },
					ui.row { align = "center", gap = "small", avatar { name = "Sarah Chen", size = "small" }, ui.column { gap = "none", ui.text { "Sarah Chen", size = "small" }, ui.text { "Product design", class = "orbit-label" } } },
				},
				ui.column { class = "orbit-work", grow = true,
					ui.column { class = "orbit-heading", shrink = false,
						ui.row { align = "center", gap = "small", ui.text { if state.page == "home" then "Good morning, Sarah" elseif state.page == "mine" then "My tasks" else project.name, class = "orbit-title" }, ui.spacer(),
							ui.button { "New task", icon = "plus", small = true, variant = "primary", msg = "new-task" } },
						ui.text { if state.page == "home" then "A little clarity for the work ahead." elseif state.page == "mine" then "Your work, across every project." else project.description, class = "orbit-subtitle" },
					}, body,
				},
				task and ui.column { width = 400, shrink = false, detail { task = decorated(task), state = state,
					choices = { status = GROUPS, people = PEOPLE, priority = PRIORITIES, mentions = mentions }, dates = dates_for(state.due_month, "month") } },
			},
		}
	end,
}
