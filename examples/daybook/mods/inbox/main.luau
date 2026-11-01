-- The inbox card: a field to add a task with no date, then what is
-- overdue, what has no date yet (tasks and notes), and what is on today.
-- All, Tasks or Notes filters it.

local icon_button = use "icon-button"
local dropdown = use "dropdown"
local task_check = use "task-check"
local task_row = use "task-row"

type State = { filter: string, picking: boolean, adding: string }

local FILTERS = { all = "All", tasks = "Tasks", notes = "Notes" }

return mod {
	region = "right",
	title = "Inbox",

	styles = {
		[".inbox"] = { padding = { 28, 24, 16, 24 }, gap = 10 },
		[".inbox-title"] = { size = "title", weight = "bold" },
		[".inbox-new"] = { padding = { 4, 8 }, gap = 10 },
		[".inbox-section"] = { size = "small", weight = "medium", color = "muted-foreground", padding = { 12, 8, 2, 8 } },
		[".inbox-note"] = { padding = { 8, 8 }, radius = 10, gap = 14 },
		[".inbox-note:hover"] = { background = "list-hover" },
		[".bare-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 2, 0 } },
	},

	init = function(previous: State?): State
		return previous or { filter = "all", picking = false, adding = "" }
	end,

	update = function(state: State, msg)
		if msg == "pick-filter" then
			state.picking = not state.picking
			return
		elseif msg == "close-filter" then
			state.picking = false
			return
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.pick then
			state.filter = msg.pick
			state.picking = false
		elseif msg.msg == "typing" then
			state.adding = msg.value
		elseif msg.msg == "add" then
			state.adding = ""
			if string.match(msg.value, "%S") then
				return state, { { to = "provider:planner", msg = { add = msg.value, inbox = true } } }
			end
		elseif msg.toggle or msg.cycle then
			return state, { { to = "provider:planner", msg = msg } }
		end
	end,

	view = function(state: State, app)
		local planner = app.planner
		if not planner then
			return ui.column { class = "inbox" }
		end
		local tags = {}
		for _, tag in planner.tags do
			tags[tag.name] = tag.color
		end
		local show_tasks = state.filter ~= "notes"
		local show_notes = state.filter ~= "tasks"

		local function row(item: any)
			if item.kind == "note" then
				return ui.row {
					class = "inbox-note",
					align = "center",
					ui.text { item.emoji or "📝" },
					ui.text { item.title, weight = "medium", truncate = true },
				}
			end
			local pills = {}
			for _, name in item.tags do
				table.insert(pills, { label = name, color = tags[name] })
			end
			return task_row {
				title = item.title,
				state = item.state,
				event = item.kind == "event",
				date = item.when,
				time = item.time,
				due = item.due,
				icon = if item.habit then "activity" else nil,
				tags = pills,
				toggle = { toggle = item.id },
				context = if item.kind == "task" then { cycle = item.id } else nil,
			}
		end

		local overdue, undated, today = {}, {}, {}
		for _, item in planner.items do
			if item.kind == "note" then
				if show_notes then
					table.insert(undated, item)
				end
			elseif show_tasks or item.kind == "event" then
				if item.overdue then
					table.insert(overdue, item)
				elseif not item.date and item.state ~= "done" then
					table.insert(undated, item)
				elseif item.date == planner.today and item.state ~= "done" then
					table.insert(today, item)
				end
			end
		end
		table.sort(today, function(a, b)
			return (a.time or "99") < (b.time or "99")
		end)

		local sections = {}
		local function section(title: string, items: { any })
			if #items == 0 then
				return
			end
			table.insert(sections, ui.text { title, class = "inbox-section" })
			for _, item in items do
				table.insert(sections, row(item))
			end
		end
		section("Overdue", overdue)
		section("Inbox", undated)
		section("Today", today)

		local options = {}
		for _, id in { "all", "tasks", "notes" } do
			table.insert(options, { id = id, label = FILTERS[id] })
		end

		return ui.column {
			class = "inbox",
			grow = true,
			ui.row {
				align = "center",
				gap = "small",
				ui.text { "Inbox", class = "inbox-title" },
				ui.spacer(),
				dropdown {
					options = options,
					selected = state.filter,
					open = state.picking,
					toggle = "pick-filter",
					dismiss = "close-filter",
					align = "end",
				},
				icon_button { icon = "panel-right", tooltip = "Hide the inbox", msg = "toggle-right", to = "shell" },
			},
			ui.row {
				class = "inbox-new",
				align = "center",
				task_check {},
				ui.input {
					id = "new-task",
					value = state.adding,
					placeholder = "New task",
					change = "typing",
					submit = "add",
					grow = true,
					class = "bare-input",
				},
			},
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column { gap = "none", shrink = false, table.unpack(sections) },
			},
		}
	end,
}
