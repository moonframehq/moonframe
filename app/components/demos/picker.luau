-- A task's properties, each a picker: click a value to open its list, type
-- to filter it, move with ↑ ↓ and pick with Enter or a click. Labels and
-- "Blocked by" take several; typing a label that does not exist offers to
-- create it.
local picker = use "picker"

local STATUSES: { any } = {
	{ id = "backlog", label = "Backlog", dot = "#8b8d98" },
	{ id = "todo", label = "Todo", dot = "#b9bbc6" },
	{ id = "progress", label = "In Progress", dot = "#f5a524" },
	{ id = "review", label = "In Review", dot = "#3e63dd" },
	{ id = "done", label = "Done", dot = "#30a46c" },
	{ id = "cancelled", label = "Cancelled", dot = "#e5484d", disabled = true },
}

local PRIORITIES: { any } = {
	{ id = "none", label = "No priority", icon = "ellipsis" },
	{ id = "urgent", label = "Urgent", icon = "triangle-alert" },
	{ id = "high", label = "High", icon = "arrow-up" },
	{ id = "medium", label = "Medium", icon = "minus" },
	{ id = "low", label = "Low", icon = "arrow-down" },
}

local PEOPLE: { any } = {
	{ id = "sarah", label = "Sarah Chen", avatar = "Sarah Chen", detail = "Design" },
	{ id = "marcus", label = "Marcus Webb", avatar = "Marcus Webb", detail = "Backend" },
	{ id = "priya", label = "Priya Raman", avatar = "Priya Raman", detail = "Frontend" },
	{ id = "theo", label = "Theo Lind", avatar = "Theo Lind", detail = "QA" },
}

local PROJECTS: { any } = {
	{ id = "mobile", label = "Mobile app v2", icon = "layers" },
	{ id = "billing", label = "Billing revamp", icon = "layers" },
	{ id = "onboarding", label = "Onboarding", icon = "layers" },
}

local TASKS: { any } = {
	{ id = "ENG-112", label = "Rate limit the public API", detail = "ENG-112", icon = "circle-dashed" },
	{ id = "ENG-118", label = "Migrate sessions to Redis", detail = "ENG-118", icon = "circle-dot" },
	{ id = "ENG-121", label = "Audit log export", detail = "ENG-121", icon = "circle-dashed" },
}

-- Colours a created label takes, in turn.
local COLOURS = { "#e5484d", "#8e4ec6", "#3e63dd", "#12a594", "#f76b15", "#d6409f" }

type State = {
	open: string?,
	query: string,
	highlighted: string?,
	status: string,
	priority: string,
	assignee: string?,
	project: string?,
	labels: { string },
	blocked: { string },
	label_options: { any },
}

-- Fields that take several values.
local MULTIPLE = { labels = true, blocked = true }

local function close(state: State)
	state.open = nil
	state.query = ""
	state.highlighted = nil
end

local function toggle(list: { string }, id: string)
	local at = table.find(list, id)
	if at then
		table.remove(list, at)
	else
		table.insert(list, id)
	end
end

return mod {
	init = function(previous: State?): State
		return previous
			or {
				open = "labels",
				query = "",
				status = "progress",
				priority = "high",
				assignee = "sarah",
				project = nil,
				labels = { "bug", "design" },
				blocked = {},
				label_options = {
					{ id = "bug", label = "Bug", color = "#e5484d" },
					{ id = "feature", label = "Feature", color = "#8e4ec6" },
					{ id = "design", label = "Design", color = "#3e63dd" },
					{ id = "docs", label = "Docs", color = "#12a594" },
				},
			}
	end,

	update = function(state: State, msg)
		if msg == "close" then
			close(state)
			return
		elseif type(msg) ~= "table" then
			return
		end
		if msg.open then
			local same = state.open == msg.open
			close(state)
			state.open = if same then nil else msg.open
		elseif msg.msg == "search" then
			state.query = msg.value or ""
		elseif msg.highlight then
			state.highlighted = msg.highlight
		elseif msg.msg == "create" then
			local id = string.lower(msg.value)
			table.insert(state.label_options, {
				id = id,
				label = msg.value,
				color = COLOURS[#state.label_options % #COLOURS + 1],
			})
			table.insert(state.labels, id)
			state.query = ""
		elseif msg.pick and state.open then
			local field = state.open :: string
			if MULTIPLE[field] then
				-- Several: a pick toggles one and the list stays open.
				toggle((state :: any)[field], msg.pick)
			else
				(state :: any)[field] = msg.pick
				close(state)
			end
		end
	end,

	view = function(state: State)
		local function field(name: string, label: string, props: { [string]: any })
			props.open = state.open == name
			props.toggle = { open = name }
			props.dismiss = "close"
			props.query = state.query
			props.search = "search"
			props.highlighted = state.highlighted
			props.selected = (state :: any)[name]
			props.multiple = MULTIPLE[name]
			return ui.row {
				align = "center",
				gap = "small",
				ui.row { width = 90, shrink = false, ui.text { label, tone = "muted" } },
				picker(props),
			}
		end
		return ui.column {
			gap = "small",
			width = 360,
			field("status", "Status", { options = STATUSES }),
			field("priority", "Priority", { options = PRIORITIES }),
			field("assignee", "Assignee", { options = PEOPLE, icon = "user", placeholder = "Assign" }),
			field("labels", "Labels", {
				options = state.label_options,
				icon = "tag",
				placeholder = "Add labels",
				create = "create",
			}),
			field("project", "Project", { options = PROJECTS, icon = "layers", placeholder = "Add to project" }),
			field("blocked", "Blocked by", {
				options = TASKS,
				icon = "ban",
				placeholder = "None",
				width = 300,
				empty = "No task matches",
			}),
			ui.row { height = 220 },
		}
	end,
}
