-- Filtering a list of tasks: the Filter button opens the builder in a
-- popover. Change a condition's field, operator or value, type into a
-- title condition, add and remove conditions, switch between all and any;
-- the list under it follows. Done or a click outside closes it.
local filter_builder = use "filter-builder"

local STATUS = {
	{ id = "todo", label = "Todo", dot = "#b9bbc6" },
	{ id = "progress", label = "In Progress", dot = "#f5a524" },
	{ id = "done", label = "Done", dot = "#30a46c" },
}

local PEOPLE = {
	{ id = "sarah", label = "Sarah Chen", avatar = "Sarah Chen" },
	{ id = "marcus", label = "Marcus Webb", avatar = "Marcus Webb" },
	{ id = "priya", label = "Priya Raman", avatar = "Priya Raman" },
}

local FIELDS: { any } = {
	{ id = "status", label = "Status", icon = "circle-dashed", operators = { "is", "is not" }, values = STATUS },
	{ id = "assignee", label = "Assignee", icon = "user", operators = { "is", "is not" }, values = PEOPLE },
	{ id = "title", label = "Title", icon = "file-text", operators = { "contains", "does not contain" } },
}

local TASKS = {
	{ title = "Fix login redirect loop", status = "progress", assignee = "sarah" },
	{ title = "Design empty states", status = "todo", assignee = "sarah" },
	{ title = "Rate limit the public API", status = "todo", assignee = "marcus" },
	{ title = "Login with passkeys", status = "done", assignee = "priya" },
	{ title = "Audit log export", status = "progress", assignee = "marcus" },
}

type Condition = { id: string, field: string, operator: string, value: string }
type State = { filtering: boolean, open: string?, match: string, conditions: { Condition }, next: number }

local function field_of(id: string): any
	for _, field in FIELDS do
		if field.id == id then
			return field
		end
	end
	return FIELDS[1]
end

-- Whether a task meets one condition; one without a value holds for all.
local function holds(task: any, condition: Condition): boolean
	if condition.value == "" then
		return true
	end
	local value = task[condition.field]
	if condition.operator == "is" then
		return value == condition.value
	elseif condition.operator == "is not" then
		return value ~= condition.value
	end
	local found = string.find(string.lower(value), string.lower(condition.value), 1, true) ~= nil
	return if condition.operator == "contains" then found else not found
end

local function shown(state: State): { any }
	local list = {}
	for _, task in TASKS do
		local all, any = true, #state.conditions == 0
		for _, condition in state.conditions do
			local ok = holds(task, condition)
			all = all and ok
			any = any or ok
		end
		if (state.match == "all" and all) or (state.match == "any" and any) then
			table.insert(list, task)
		end
	end
	return list
end

local function label_of(options: { any }, id: string): string
	for _, option in options do
		if option.id == id then
			return option.label
		end
	end
	return id
end

return mod {
	init = function(previous: State?): State
		return previous
			or {
				filtering = true,
				match = "all",
				next = 3,
				conditions = {
					{ id = "c1", field = "status", operator = "is not", value = "done" },
					{ id = "c2", field = "title", operator = "contains", value = "" },
				},
			}
	end,

	update = function(state: State, msg)
		if msg == "filter" then
			state.filtering = not state.filtering
		elseif msg == "close-filter" or msg == "done" then
			state.filtering = false
			state.open = nil
		elseif msg == "add" then
			table.insert(state.conditions, { id = `c{state.next}`, field = "status", operator = "is", value = "" })
			state.next += 1
		elseif msg == "clear" then
			state.conditions = {}
		elseif type(msg) ~= "table" then
			return
		elseif msg.open then
			state.open = if msg.open == "" or msg.open == state.open then nil else msg.open
		elseif msg.match then
			state.match = msg.match
		elseif msg.remove then
			for index, condition in state.conditions do
				if condition.id == msg.remove then
					table.remove(state.conditions, index)
				end
			end
		elseif msg.set then
			for _, condition in state.conditions do
				if condition.id == msg.set.id then
					local value = msg.set.value or msg.value or ""
					if msg.set.key == "field" and value ~= condition.field then
						-- A new field starts from its first operator, no value.
						condition.field = value
						condition.operator = field_of(value).operators[1]
						condition.value = ""
					elseif msg.set.key == "operator" then
						condition.operator = value
					elseif msg.set.key == "value" then
						condition.value = value
					end
				end
			end
			-- A pick closes its dropdown; typing leaves nothing open anyway.
			state.open = nil
		end
	end,

	view = function(state: State)
		local rows = {}
		for _, task in shown(state) do
			table.insert(rows, ui.row {
				gap = "small",
				align = "center",
				ui.text { task.title, truncate = true },
				ui.spacer(),
				ui.text { label_of(STATUS, task.status), tone = "muted", size = "small" },
				ui.text { label_of(PEOPLE, task.assignee), tone = "muted", size = "small" },
			})
		end
		return ui.column {
			gap = "normal",
			width = 520,
			ui.row {
				align = "center",
				gap = "small",
				ui.popover {
					open = state.filtering,
					dismiss = "close-filter",
					side = "bottom",
					align = "start",
					gap = 4,
					width = 520,
					content = filter_builder {
						match = state.match,
						conditions = state.conditions,
						fields = FIELDS,
						open = state.open,
					},
					ui.button { "Filter", icon = "funnel", small = true, msg = "filter" },
				},
				ui.text { `{#state.conditions} conditions`, tone = "muted", size = "small" },
			},
			ui.column { gap = "small", table.unpack(rows) },
			ui.row { height = 160 },
		}
	end,
}
