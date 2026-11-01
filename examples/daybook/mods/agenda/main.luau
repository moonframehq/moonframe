-- The day card: the view the sidebar picked. Today shows a strip of days,
-- a line saying what is on, and that day's tasks and events; Upcoming the
-- next two weeks; Calendar a month; Notes, Tasks and Events everything of
-- their kind. Typing in the top bar shows what matches instead. A check
-- marks a task done; a right-click cycles its priority.

local day_strip = use "day-strip"
local month_grid = use "month-grid"
local nav_buttons = use "nav-buttons"
local task_row = use "task-row"
local pill = use "pill"

type State = { view: string, query: string }

-- Order within a day: done first (as they were), then in progress,
-- important, open; events by time after them.
local RANK = { done = 1, progress = 2, important = 3, open = 4 }

local VIEW_TITLES = {
	upcoming = "Upcoming",
	calendar = "Calendar",
	notes = "Notes",
	tasks = "Tasks",
	events = "Events",
}

local function by_day(a: any, b: any): boolean
	if a.kind ~= b.kind then
		return a.kind == "task"
	end
	if a.kind == "task" and a.state ~= b.state then
		return (RANK[a.state] or 9) < (RANK[b.state] or 9)
	end
	return (a.time or "99") < (b.time or "99")
end

return mod {
	region = "main",
	title = "Agenda",

	styles = {
		[".agenda"] = { padding = { 28, 32, 20, 32 }, gap = 18 },
		[".agenda-big"] = { size = "title", weight = "bold" },
		[".agenda-subtitle"] = { color = "muted-foreground" },
		[".agenda-today"] = { radius = 14, padding = { 5, 12 }, background = "list-hover", color = "muted-foreground" },
		[".agenda-today:hover"] = { background = "accent/12" },
		[".agenda-summary"] = { radius = 14, padding = { 10, 14 }, gap = 6, border = true, border_color = "border", background = "list-hover" },
		[".agenda-section"] = { size = "small", weight = "medium", color = "muted-foreground", padding = { 10, 8, 2, 8 } },
		[".agenda-empty"] = { color = "muted-foreground", padding = { 4, 8 } },
		[".agenda-note"] = { padding = { 8, 8 }, radius = 10, gap = 12 },
		[".agenda-note:hover"] = { background = "list-hover" },
	},

	init = function(previous: State?): State
		return previous or { view = "today", query = "" }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.view then
			state.view = msg.view
		elseif msg.query ~= nil then
			state.query = msg.query
		elseif msg.select then
			-- A day from the strip or the month.
			return state, { { to = "provider:planner", msg = { select = msg.select } } }
		elseif msg.toggle or msg.cycle then
			return state, { { to = "provider:planner", msg = msg } }
		end
	end,

	view = function(state: State, app)
		local planner = app.planner
		if not planner then
			return ui.column { class = "agenda", ui.text { "Opening your planner…", tone = "muted" } }
		end
		local tags = {}
		for _, tag in planner.tags do
			tags[tag.name] = tag.color
		end

		local function row(item: any)
			local pills = {}
			for _, name in item.tags do
				table.insert(pills, { label = name, color = tags[name] })
			end
			if item.kind == "note" then
				return ui.row {
					class = "agenda-note",
					align = "center",
					ui.text { item.emoji or "📝" },
					ui.text { item.title, weight = "medium" },
				}
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

		local function heading(title: string, subtitle: string?, actions: any?)
			return ui.row {
				align = "start",
				ui.column {
					gap = "none",
					ui.text { title, class = "agenda-big" },
					subtitle and ui.text { subtitle, class = "agenda-subtitle" },
				},
				ui.spacer(),
				actions,
			}
		end

		local function list(items: { any }, empty: string)
			if #items == 0 then
				return ui.text { empty, class = "agenda-empty" }
			end
			local rows = {}
			for _, item in items do
				table.insert(rows, row(item))
			end
			return ui.column { gap = "none", shrink = false, table.unpack(rows) }
		end

		local body = {}
		local query = string.lower(state.query)
		if query ~= "" then
			local found = {}
			for _, item in planner.items do
				if string.find(string.lower(item.title), query, 1, true) then
					table.insert(found, item)
				end
			end
			table.insert(body, heading("Search", `{#found} found for “{state.query}”; Enter adds it as a task`))
			table.insert(body, list(found, "Nothing matches"))
		elseif state.view == "today" then
			local day, events, habits, important = {}, 0, 0, 0
			for _, item in planner.items do
				if item.date == planner.selected and item.kind ~= "note" then
					table.insert(day, item)
					if item.kind == "event" then
						events += 1
					elseif item.habit then
						habits += 1
					elseif item.state == "important" or item.state == "progress" then
						important += 1
					end
				end
			end
			table.sort(day, by_day)
			local function plural(n: number, one: string, many: string)
				return `{n} {if n == 1 then one else many}`
			end
			table.insert(body, heading(planner.title, planner.subtitle, ui.row {
				gap = "small",
				align = "center",
				ui.row {
					class = "agenda-today",
					msg = { select = planner.today },
					ui.text { "Today", size = "small", weight = "medium" },
				},
				nav_buttons { back = { shift = -1 }, forward = { shift = 1 }, to = "provider:planner", small = true },
			}))
			table.insert(body, day_strip { days = planner.days, selected = planner.selected })
			table.insert(body, ui.row {
				class = "agenda-summary",
				align = "center",
				wrap = true,
				ui.text { `{planner.greeting}, you have`, weight = "medium" },
				pill { label = plural(events, "event", "events"), icon = "calendar", tone = "danger", size = "normal" },
				pill { label = plural(habits, "daily habit", "daily habits"), icon = "activity", size = "normal" },
				ui.text { "and", weight = "medium" },
				pill { label = `{important} important`, icon = "triangle-alert", tone = "warning", size = "normal" },
			})
			table.insert(body, ui.column { grow = true, scroll = true, list(day, "Nothing on this day. Type above to add a task.") })
		elseif state.view == "calendar" then
			local day = {}
			for _, item in planner.items do
				if item.date == planner.selected and item.kind ~= "note" then
					table.insert(day, item)
				end
			end
			table.sort(day, by_day)
			table.insert(body, heading(planner.month.title, planner.subtitle, nav_buttons {
				back = { month = -1 },
				forward = { month = 1 },
				to = "provider:planner",
				small = true,
			}))
			table.insert(body, month_grid { weeks = planner.month.weeks, selected = planner.selected })
			table.insert(body, ui.column { grow = true, scroll = true, list(day, "Nothing on this day") })
		elseif state.view == "upcoming" then
			table.insert(body, heading("Upcoming", "The next two weeks"))
			local sections = {}
			for _, day in planner.days do
				local items = {}
				for _, item in planner.items do
					if item.date == day.id and item.kind ~= "note" and item.state ~= "done" then
						table.insert(items, item)
					end
				end
				if #items > 0 then
					table.sort(items, by_day)
					table.insert(sections, ui.text {
						if day.today then "Today" else `{day.weekday} {day.day}`,
						class = "agenda-section",
					})
					table.insert(sections, list(items, ""))
				end
			end
			table.insert(body, ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				if #sections == 0 then ui.text { "Nothing coming up", class = "agenda-empty" } else nil,
				table.unpack(sections),
			})
		else
			local kind = if state.view == "notes" then "note" elseif state.view == "events" then "event" else "task"
			local open, done = {}, {}
			for _, item in planner.items do
				if item.kind == kind then
					table.insert(if item.state == "done" then done else open, item)
				end
			end
			table.sort(open, function(a, b)
				return (a.date or "~") < (b.date or "~")
			end)
			table.insert(body, heading(VIEW_TITLES[state.view] or "Tasks", `{#open} {if kind == "note" then "notes" else "to come"}`))
			table.insert(body, ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				list(open, "None yet"),
				#done > 0 and ui.text { "Done", class = "agenda-section" },
				#done > 0 and list(done, ""),
			})
		end

		return ui.column { class = "agenda", grow = true, table.unpack(body) }
	end,
}
