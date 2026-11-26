-- The main center card: dispatches across Today, Upcoming (Board & List),
-- Calendar (3-day, Week & Month), Notes grid, Rich Note editor, Tasks (grouped
-- List & Board), Events, Habits, Folder filter, Tag filter, and Search.

local day_strip = use "day-strip"
local daybook_calendar = use "daybook-calendar"
local daybook_habits = use "daybook-habits"
local daybook_note = use "daybook-note"
local daybook_upcoming = use "daybook-upcoming"
local nav_buttons = use "nav-buttons"
local pill = use "pill"
local segmented = use "segmented"
local task_row = use "task-row"

type State = {
	view: string,
	note_id: string?,
	folder: string?,
	tag: string?,
	query: string,
	selected_id: string?,
	upcoming_mode: string,
	calendar_mode: string,
	tasks_mode: string,
	over_day: string?,
	-- Note editor drafts (transient)
	note_draft_id: string?,
	note_title: string,
	note_body: string,
	note_preview: boolean,
	note_tag_menu: boolean,
	note_new_task: string,
	-- Inline creators
	new_habit: string,
	new_event: string,
	new_scoped_task: string,
	-- Latest snapshot from provider/planner (transient)
	planner: any,
	open_newest_note: boolean,
}

local RANK: { [string]: number } = { done = 1, progress = 2, important = 3, open = 4 }
local COVER_CYCLE = { "sunset", "ocean", "forest", "aurora", "amber", "plum", "none" }
local EMOJI_CYCLE = { "📝", "⭐", "🧳", "🚀", "💡", "📋", "🌿", "🎯", "✨" }
local COVER_GRADS: { [string]: string } = {
	sunset = "135deg, #ff7e5f, #e5484d",
	ocean = "135deg, #3b82f6, #06b6d4",
	forest = "135deg, #2f9e5b, #10b981",
	aurora = "135deg, #6366f1, #ec4899",
	amber = "135deg, #f59e0b, #ef4444",
	plum = "135deg, #8b5cf6, #3b6fd8",
}

local function by_day(a: any, b: any): boolean
	if a.kind ~= b.kind then
		return a.kind == "task"
	end
	if a.kind == "task" and a.state ~= b.state then
		return (RANK[a.state] or 9) < (RANK[b.state] or 9)
	end
	local ha: number = a.start_hour or 99
	local hb: number = b.start_hour or 99
	if ha ~= hb then
		return ha < hb
	end
	local ta = a.title or ""
	local tb = b.title or ""
	if ta ~= tb then
		return ta < tb
	end
	return (a.id or "") < (b.id or "")
end

local function find_item(planner: any, id: string?): any
	if not planner or not id then
		return nil
	end
	for _, item in planner.items or {} do
		if item.id == id then
			return item
		end
	end
	return nil
end

local function cycle_in(list: { string }, current: string?): string
	for i, value in list do
		if value == current then
			return list[(i % #list) + 1]
		end
	end
	return list[1]
end

local function load_note_drafts(state: State, note: any)
	if note and state.note_draft_id ~= note.id then
		state.note_draft_id = note.id
		state.note_title = note.title or ""
		state.note_body = note.body or ""
		state.note_preview = false
		state.note_tag_menu = false
		state.note_new_task = ""
	end
end

local function calendar_hours(planner: any, mode: string): (number, number)
	local from_h, to_h = 7, 21
	if not planner or mode == "month" then
		return from_h, to_h
	end
	local details: { any } = planner.day_details or {}
	local span = if mode == "week" then 7 else 3
	local visible: { [string]: boolean } = {}
	for i = 1, math.min(span, #details) do
		visible[details[i].id] = true
	end
	for _, item in planner.items or {} do
		if item.kind ~= "note" and item.date and visible[item.date] and item.start_hour ~= nil then
			local start_h: number = item.start_hour
			local end_h: number = item.end_hour or math.min(24, start_h + 1)
			from_h = math.min(from_h, math.max(0, math.floor(start_h)))
			to_h = math.max(to_h, math.min(24, math.ceil(end_h)))
		end
	end
	return from_h, to_h
end

local function format_hour_from_y(y: number?, from_h: number, to_h: number): string
	local frac = if type(y) == "number" then math.max(0, math.min(1, y)) else 0.25
	local span = math.max(1, to_h - from_h)
	local hours = from_h + frac * span
	local h = math.floor(hours)
	local m = if (hours - h) >= 0.5 then 30 else 0
	local max_h = math.min(23, math.max(from_h, to_h - 1))
	return string.format("%02d:%02d", math.min(max_h, math.max(from_h, h)), m)
end

return mod {
	region = "main",
	title = "Agenda",
	subscribe = { "provider/planner" },

	styles = {
		[".agenda"] = { padding = { 26, 30, 18, 30 }, gap = 16 },
		[".agenda-big"] = { size = "title", weight = "bold" },
		[".agenda-subtitle"] = { color = "muted-foreground" },
		[".agenda-today"] = { radius = 14, padding = { 5, 12 }, background = "list-hover", color = "muted-foreground" },
		[".agenda-today:hover"] = { background = "accent/12", color = "foreground" },
		[".agenda-summary"] = { radius = 14, padding = { 10, 14 }, gap = 6, border = true, border_color = "border", background = "list-hover" },
		[".agenda-section"] = { size = "small", weight = "semibold", color = "muted-foreground", padding = { 12, 8, 4, 8 } },
		[".agenda-empty"] = { color = "muted-foreground", padding = { 4, 8 } },
		[".agenda-note-card"] = { background = "surface-soft", radius = 14, border = "all", border_color = "border/60", clip = true },
		[".agenda-note-card:hover"] = { border_color = "foreground/30", shadow = "0 4px 12px -4px #00000018" },
		[".agenda-note-body"] = { padding = { 10, 12 }, gap = 6 },
		[".agenda-note-grid"] = { gap = 12 },
		[".agenda-add-box"] = { background = "surface-soft", radius = 12, padding = { 6, 12 }, gap = 10 },
		[".agenda-bare"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		[".agenda-board"] = { gap = 12, padding = { 4, 2 } },
		[".agenda-board-col"] = { background = "surface-soft", radius = 14, padding = { 12, 10 }, gap = 8 },
		[".agenda-board-card"] = { background = "card", radius = 10, padding = { 8, 10 }, border = "all", border_color = "border/60" },
		[".agenda-board-card:hover"] = { border_color = "foreground/30" },
		[".agenda-board-card:selected"] = { border_color = "foreground/75" },
		[".agenda-btn"] = { background = "surface-soft", border = true, border_color = "border", radius = 10, padding = { 5, 12 }, gap = 6 },
		[".agenda-btn:hover"] = { background = "list-hover" },
	},

	init = function(previous: State?): State
		return {
			view = previous and previous.view or "today",
			note_id = previous and previous.note_id or nil,
			folder = previous and previous.folder or nil,
			tag = previous and previous.tag or nil,
			query = "",
			selected_id = nil,
			upcoming_mode = previous and previous.upcoming_mode or "board",
			calendar_mode = previous and previous.calendar_mode or "3days",
			tasks_mode = previous and previous.tasks_mode or "list",
			over_day = nil,
			note_draft_id = nil,
			note_title = "",
			note_body = "",
			note_preview = false,
			note_tag_menu = false,
			note_new_task = "",
			new_habit = "",
			new_event = "",
			new_scoped_task = "",
			planner = nil,
			open_newest_note = false,
		}
	end,

	update = function(state: State, msg)
		if msg == "new-note" then
			state.open_newest_note = true
			return state, {
				{ to = "provider:planner", msg = { add = "Untitled note", kind = "note", cover = "sunset" } },
			}
		elseif msg == "note-toggle-preview" then
			state.note_preview = not state.note_preview
			return
		elseif msg == "note-open-tags" then
			state.note_tag_menu = not state.note_tag_menu
			return
		elseif msg == "note-close-tags" then
			state.note_tag_menu = false
			return
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.topic == "provider/planner" then
			state.planner = msg.value
			if state.open_newest_note and state.planner and state.planner.items then
				state.open_newest_note = false
				local newest: any = nil
				for _, item in state.planner.items do
					if item.kind == "note" then
						newest = item
					end
				end
				if newest then
					state.view = "note"
					state.note_id = newest.id
					state.folder = nil
					state.tag = nil
					load_note_drafts(state, newest)
					return state, { { to = "sidebar", msg = { sync = { view = "note", note_id = newest.id } } } }
				end
			end
			if state.view == "note" and state.note_id then
				local note = find_item(state.planner, state.note_id)
				if note then
					load_note_drafts(state, note)
				end
			end
			return
		end

		if msg.route then
			state.view = msg.route.view or "today"
			state.note_id = msg.route.note_id
			state.folder = msg.route.folder
			state.tag = msg.route.tag
			if state.view == "note" then
				load_note_drafts(state, find_item(state.planner, state.note_id))
			end
			return
		elseif msg.view then
			state.view = msg.view
			state.note_id = nil
			state.folder = nil
			state.tag = nil
			return
		elseif msg.open_note then
			state.view = "note"
			state.note_id = msg.open_note
			state.folder = nil
			state.tag = nil
			load_note_drafts(state, find_item(state.planner, msg.open_note))
			return state, { { to = "sidebar", msg = { sync = { view = "note", note_id = msg.open_note } } } }
		elseif msg.selected_id ~= nil then
			state.selected_id = if msg.selected_id == "" then nil else msg.selected_id
			return
		elseif msg.inspect then
			state.selected_id = msg.inspect
			return state, {
				{ to = "shell", msg = "show-right" },
				{ to = "inbox", msg = { inspect = msg.inspect } },
			}
		elseif msg.query ~= nil then
			state.query = msg.query
			return
		elseif msg.upcoming_mode then
			state.upcoming_mode = msg.upcoming_mode
			return
		elseif msg.calendar_mode then
			state.calendar_mode = msg.calendar_mode
			return
		elseif msg.tasks_mode then
			state.tasks_mode = msg.tasks_mode
			return
		elseif msg.cal_move then
			local day_id = msg.cal_move
			if msg.phase == "over" then
				state.over_day = day_id
			elseif msg.phase == "leave" then
				if state.over_day == day_id then
					state.over_day = nil
				end
			elseif msg.phase == "drop" then
				state.over_day = nil
				local event_id = msg.carry and msg.carry.event
				if event_id then
					local patch: any = { update = event_id, date = day_id }
					if state.calendar_mode ~= "month" and type(msg.y) == "number" then
						local from_h, to_h = calendar_hours(state.planner, state.calendar_mode)
						patch.time = format_hour_from_y(msg.y, from_h, to_h)
					end
					return state, { { to = "provider:planner", msg = patch } }
				end
			end
			return
		elseif msg.select then
			return state, { { to = "provider:planner", msg = { select = msg.select } } }
		elseif msg.toggle or msg.cycle or msg.habit_checkin or msg.habit_step then
			return state, { { to = "provider:planner", msg = msg } }
		elseif msg.note_cycle_cover then
			local note = find_item(state.planner, msg.note_cycle_cover)
			local current = if note and note.cover then note.cover else "none"
			local next_cover = cycle_in(COVER_CYCLE, current)
			return state, { { to = "provider:planner", msg = { update = msg.note_cycle_cover, cover = next_cover } } }
		elseif msg.note_cycle_emoji then
			local note = find_item(state.planner, msg.note_cycle_emoji)
			local next_emoji = cycle_in(EMOJI_CYCLE, note and note.emoji)
			return state, { { to = "provider:planner", msg = { update = msg.note_cycle_emoji, emoji = next_emoji } } }
		elseif msg.note_toggle_fav then
			local note = find_item(state.planner, msg.note_toggle_fav)
			local fav = not (note and note.favorite == true)
			return state, { { to = "provider:planner", msg = { update = msg.note_toggle_fav, favorite = fav } } }
		elseif msg.note_cycle_folder then
			local note = find_item(state.planner, msg.note_cycle_folder)
			local folders: { string } = (state.planner and state.planner.folders) or {}
			local list = { "" }
			for _, f in folders do
				table.insert(list, f)
			end
			local next_folder = cycle_in(list, (note and note.folder) or "")
			return state, { { to = "provider:planner", msg = { update = msg.note_cycle_folder, folder = next_folder } } }
		elseif msg.note_toggle_tag and state.note_id then
			state.note_tag_menu = false
			return state, { { to = "provider:planner", msg = { update = state.note_id, toggle_tag = msg.note_toggle_tag } } }
		elseif msg.note_delete then
			state.view = "notes"
			state.note_id = nil
			state.note_draft_id = nil
			return state, {
				{ to = "provider:planner", msg = { remove = msg.note_delete } },
				{ to = "sidebar", msg = { sync = { view = "notes" } } },
			}
		elseif msg.msg == "note-title-typing" then
			state.note_title = msg.value
		elseif msg.msg == "note-title-save" and state.note_id then
			state.note_title = msg.value
			if string.match(msg.value, "%S") then
				return state, { { to = "provider:planner", msg = { update = state.note_id, title = msg.value } } }
			end
		elseif msg.msg == "note-body-typing" then
			state.note_body = msg.value
		elseif msg.msg == "note-body-save" and state.note_id then
			state.note_body = msg.value
			return state, { { to = "provider:planner", msg = { update = state.note_id, body = msg.value } } }
		elseif msg.msg == "note-task-typing" then
			state.note_new_task = msg.value
		elseif msg.msg == "note-task-add" and state.note_id then
			local title = msg.value
			state.note_new_task = ""
			if string.match(title, "%S") then
				local note = find_item(state.planner, state.note_id)
				return state, { {
					to = "provider:planner",
					msg = { add = title, note_id = state.note_id, folder = note and note.folder },
				} }
			end
		elseif msg.msg == "habit-typing" then
			state.new_habit = msg.value
		elseif msg.msg == "habit-add" then
			local title = msg.value
			state.new_habit = ""
			if string.match(title, "%S") then
				return state, { { to = "provider:planner", msg = { add = title, habit = true, tags = { "Health" } } } }
			end
		elseif msg.msg == "event-typing" then
			state.new_event = msg.value
		elseif msg.msg == "event-add" then
			local title = msg.value
			state.new_event = ""
			if string.match(title, "%S") then
				return state, { { to = "provider:planner", msg = { add = title, kind = "event", time = "14:00" } } }
			end
		elseif msg.msg == "scoped-typing" then
			state.new_scoped_task = msg.value
		elseif msg.msg == "scoped-add" then
			local title = msg.value
			state.new_scoped_task = ""
			if string.match(title, "%S") then
				local payload: any = { add = title }
				if state.view == "folder" and state.folder then
					payload.folder = state.folder
				elseif state.view == "tag" and state.tag then
					payload.tags = { state.tag }
				end
				return state, { { to = "provider:planner", msg = payload } }
			end
		end
	end,

	view = function(state: State, app)
		local planner = app.planner
		if not planner then
			return ui.column { class = "agenda", ui.text { "Opening your planner…", tone = "muted" } }
		end
		local tags_map: { [string]: string } = {}
		for _, tag in planner.tags or {} do
			tags_map[tag.name] = tag.color
		end
		local notes_map: { [string]: any } = {}
		for _, item in planner.items or {} do
			if item.kind == "note" then
				notes_map[item.id] = item
			end
		end

		local function row(item: any): Element
			if item.kind == "note" then
				return ui.row {
					class = "task-row",
					align = "center",
					msg = { open_note = item.id },
					ui.text { item.emoji or "📝" },
					ui.text { item.title, weight = "medium", truncate = true },
					ui.spacer(),
					item.folder and pill { label = item.folder, tone = "neutral" },
				}
			end
			local pills = {}
			if item.days_left and item.state ~= "done" then
				table.insert(pills, { label = item.days_left, tone = "neutral" })
			end
			if item.habit and (item.streak or 0) > 0 then
				table.insert(pills, { label = `{item.streak}d`, icon = "zap", tone = "danger" })
			end
			if item.note_id and notes_map[item.note_id] then
				local n = notes_map[item.note_id]
				table.insert(pills, { label = `{n.emoji or "📝"} {n.title}`, tone = "neutral" })
			end
			for _, name in item.tags or {} do
				table.insert(pills, { label = name, color = tags_map[name] })
			end
			return task_row {
				title = item.title,
				state = item.state,
				event = item.kind == "event",
				date = item.when,
				time = item.time,
				due = item.due,
				icon = if item.habit then "zap" else nil,
				tags = pills,
				selected = state.selected_id == item.id,
				toggle = { toggle = item.id },
				msg = { inspect = item.id },
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

		local function list(items: { any }, empty: string): Element
			if #items == 0 then
				return ui.text { empty, class = "agenda-empty" }
			end
			local rows: { Element } = {}
			for _, item in items do
				table.insert(rows, row(item))
			end
			return ui.column { gap = "none", shrink = false, table.unpack(rows) }
		end

		local query = string.lower(state.query)
		if query ~= "" then
			local found = {}
			for _, item in planner.items or {} do
				local hay = string.lower(`{item.title or ""} {item.body or ""} {item.folder or ""}`)
				if string.find(hay, query, 1, true) then
					table.insert(found, item)
				end
			end
			return ui.column {
				class = "agenda",
				grow = true,
				heading("Search", `{#found} found for “{state.query}”; Enter adds it as a task`),
				ui.column { grow = true, scroll = true, list(found, "Nothing matches") },
			}
		end

		if state.view == "today" then
			local day_items, events, habits, important, open_tasks = {}, 0, 0, 0, 0
			for _, item in planner.items or {} do
				if item.date == planner.selected and item.kind ~= "note" then
					table.insert(day_items, item)
					if item.kind == "event" then
						events += 1
					elseif item.habit then
						habits += 1
					elseif item.state == "important" or item.state == "progress" then
						important += 1
					elseif item.state == "open" then
						open_tasks += 1
					end
				end
			end
			table.sort(day_items, by_day)
			local function plural(n: number, one: string, many: string)
				return `{n} {if n == 1 then one else many}`
			end
			local summary = if events + habits + important + open_tasks == 0
				then ui.row {
					class = "agenda-summary",
					align = "center",
					ui.text { `{planner.greeting}, nothing planned.`, weight = "medium" },
				}
				else ui.row {
					class = "agenda-summary",
					align = "center",
					wrap = true,
					ui.text { `{planner.greeting}, you have`, weight = "medium" },
					pill { label = plural(events, "event", "events"), icon = "calendar", tone = "danger", size = "normal", msg = { view = "events" }, to = "sidebar" },
					pill { label = plural(habits, "daily habit", "daily habits"), icon = "zap", size = "normal", msg = { view = "habits" }, to = "sidebar" },
					pill { label = `{important} important`, icon = "triangle-alert", tone = "warning", size = "normal", msg = { view = "tasks" }, to = "sidebar" },
					open_tasks > 0 and ui.text { "and", weight = "medium" },
					open_tasks > 0 and pill { label = plural(open_tasks, "task", "tasks"), icon = "square-check", size = "normal", msg = { view = "tasks" }, to = "sidebar" },
				}
			return ui.column {
				class = "agenda",
				grow = true,
				heading(planner.title, planner.subtitle, ui.row {
					gap = "small",
					align = "center",
					ui.row {
						class = "agenda-today",
						msg = { select = planner.today },
						ui.text { "Today", size = "small", weight = "medium" },
					},
					nav_buttons { back = { shift = -1 }, forward = { shift = 1 }, to = "provider:planner", small = true },
				}),
				day_strip { days = planner.days, selected = planner.selected },
				summary,
				ui.column { grow = true, scroll = true, list(day_items, "Nothing on this day. Type above to add a task.") },
			}
		end

		if state.view == "upcoming" then
			return ui.column {
				class = "agenda",
				grow = true,
				daybook_upcoming {
					planner = planner,
					mode = state.upcoming_mode,
					selected_id = state.selected_id,
					tags_map = tags_map,
					notes_map = notes_map,
				},
			}
		end

		if state.view == "calendar" then
			return ui.column {
				class = "agenda",
				grow = true,
				daybook_calendar {
					planner = planner,
					mode = state.calendar_mode,
					selected_id = state.selected_id,
					over_day = state.over_day,
					tags_map = tags_map,
				},
			}
		end

		if state.view == "habits" then
			return ui.column {
				class = "agenda",
				grow = true,
				daybook_habits {
					planner = planner,
					tags_map = tags_map,
					selected_id = state.selected_id,
					new_habit = state.new_habit,
				},
			}
		end

		if state.view == "note" then
			local note = find_item(planner, state.note_id)
			if note then
				local title_draft = if state.note_draft_id == note.id then state.note_title else (note.title or "")
				local body_draft = if state.note_draft_id == note.id then state.note_body else (note.body or "")
				return ui.column {
					class = "agenda",
					grow = true,
					daybook_note {
						note = note,
						planner = planner,
						tags_map = tags_map,
						selected_id = state.selected_id,
						title_draft = title_draft,
						body_draft = body_draft,
						preview = state.note_preview,
						tag_menu = state.note_tag_menu,
						new_task = state.note_new_task,
					},
				}
			end
		end

		if state.view == "notes" then
			local notes = {}
			for _, item in planner.items or {} do
				if item.kind == "note" then
					table.insert(notes, item)
				end
			end
			local rows: { Element } = {}
			for i = 1, #notes, 2 do
				local pair: { Element } = {}
				for j = i, math.min(i + 1, #notes) do
					local n = notes[j]
					local grad: string? = if n.cover and n.cover ~= "none" then COVER_GRADS[n.cover] else nil
					local tag_pills: { Element } = {}
					if n.folder then
						table.insert(tag_pills, pill { label = n.folder, icon = "folder", tone = "neutral" })
					end
					for _, t in n.tags or {} do
						table.insert(tag_pills, pill { label = t, color = tags_map[t] })
					end
					local preview_text = string.gsub(n.body or "", "#+%s*", "")
					table.insert(pair, ui.column {
						class = "agenda-note-card",
						grow = true,
						width = 1,
						gap = "none",
						msg = { open_note = n.id },
						grad and ui.row { height = 56, style = { gradient = grad } },
						ui.column {
							class = "agenda-note-body",
							ui.row {
								align = "center",
								gap = "small",
								ui.text { n.emoji or "📝" },
								ui.text { n.title, weight = "semibold", truncate = true },
								ui.spacer(),
								n.favorite and ui.icon { name = "star-fill", size = "small", tone = "warning" },
							},
							preview_text ~= "" and ui.text { preview_text, size = "small", tone = "muted", lines = 2 },
							#tag_pills > 0 and ui.row { gap = "small", wrap = true, table.unpack(tag_pills) },
						},
					})
				end
				if #pair == 1 then
					table.insert(pair, ui.column { grow = true, width = 1 })
				end
				table.insert(rows, ui.row { class = "agenda-note-grid", table.unpack(pair) })
			end
			return ui.column {
				class = "agenda",
				grow = true,
				heading("Notes", `{#notes} notes`, ui.row {
					class = "agenda-btn",
					align = "center",
					msg = "new-note",
					ui.icon { name = "plus", size = "small" },
					ui.text { "New note", size = "small", weight = "medium" },
				}),
				ui.column {
					grow = true,
					scroll = true,
					gap = "normal",
					if #rows == 0 then ui.text { "No notes yet. Click New note to create one.", class = "agenda-empty" } else nil,
					table.unpack(rows),
				},
			}
		end

		if state.view == "tasks" then
			local inbox_t, overdue_t, today_t, upcoming_t, done_t = {}, {}, {}, {}, {}
			for _, item in planner.items or {} do
				if item.kind == "task" then
					if item.state == "done" then
						table.insert(done_t, item)
					elseif item.overdue then
						table.insert(overdue_t, item)
					elseif not item.date then
						table.insert(inbox_t, item)
					elseif item.date == planner.today then
						table.insert(today_t, item)
					else
						table.insert(upcoming_t, item)
					end
				end
			end
			local open_count = #inbox_t + #overdue_t + #today_t + #upcoming_t
			local switcher = segmented {
				selected = state.tasks_mode,
				event = "tasks_mode",
				small = true,
				options = {
					{ id = "list", icon = "list", tooltip = "Grouped list" },
					{ id = "board", icon = "columns-2", tooltip = "Status board" },
				},
			}
			local body_el: Element
			if state.tasks_mode == "board" then
				local groups = {
					{ title = "Inbox", items = inbox_t },
					{ title = "Overdue", items = overdue_t },
					{ title = "Today", items = today_t },
					{ title = "Upcoming", items = upcoming_t },
					{ title = "Done", items = done_t },
				}
				local cols: { Element } = {}
				for _, g in groups do
					local cards: { Element } = {}
					for _, it in g.items do
						table.insert(cards, ui.column {
							class = "agenda-board-card",
							selected = state.selected_id == it.id,
							row(it),
						})
					end
					table.insert(cols, ui.column {
						class = "agenda-board-col",
						width = 250,
						shrink = false,
						ui.row {
							align = "center",
							ui.text { g.title, weight = "semibold" },
							ui.spacer(),
							pill { label = tostring(#g.items), tone = "neutral" },
						},
						ui.column { grow = true, scroll = true, gap = "small", table.unpack(cards) },
					})
				end
				body_el = ui.row { class = "agenda-board", grow = true, scroll = "x", align = "stretch", table.unpack(cols) }
			else
				local sections: { Element } = {}
				local function add_sec(label: string, items: { any })
					if #items == 0 then
						return
					end
					table.insert(sections, ui.text { `{label}  ·  {#items}`, class = "agenda-section" })
					table.insert(sections, list(items, ""))
				end
				add_sec("Inbox", inbox_t)
				add_sec("Overdue", overdue_t)
				add_sec("Today", today_t)
				add_sec("Upcoming", upcoming_t)
				add_sec("Done", done_t)
				body_el = ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					if #sections == 0 then ui.text { "No tasks yet", class = "agenda-empty" } else nil,
					table.unpack(sections),
				}
			end
			return ui.column {
				class = "agenda",
				grow = true,
				heading("Tasks", `{open_count} open · {#done_t} done`, switcher),
				body_el,
			}
		end

		if state.view == "events" then
			local upcoming_ev = {}
			for _, item in planner.items or {} do
				if item.kind == "event" then
					table.insert(upcoming_ev, item)
				end
			end
			table.sort(upcoming_ev, function(a: any, b: any)
				local da = a.date or "~"
				local db = b.date or "~"
				if da ~= db then
					return da < db
				end
				local ha: number = a.start_hour or 99
				local hb: number = b.start_hour or 99
				if ha ~= hb then
					return ha < hb
				end
				local ta = a.title or ""
				local tb = b.title or ""
				if ta ~= tb then
					return ta < tb
				end
				return (a.id or "") < (b.id or "")
			end)
			return ui.column {
				class = "agenda",
				grow = true,
				heading("Events", `{#upcoming_ev} scheduled`),
				ui.row {
					class = "agenda-add-box",
					align = "center",
					ui.icon { name = "calendar", size = "small", tone = "danger" },
					ui.input {
						id = "new-event",
						value = state.new_event,
						placeholder = "Add an event for the selected day (14:00)…",
						change = "event-typing",
						submit = "event-add",
						grow = true,
						class = "agenda-bare",
					},
				},
				ui.column { grow = true, scroll = true, list(upcoming_ev, "No events yet") },
			}
		end

		-- Folder or Tag filtered views
		if state.view == "folder" or state.view == "tag" then
			local is_folder = state.view == "folder"
			local filter_name = (if is_folder then state.folder else state.tag) or ""
			local matched_notes, matched_open, matched_done = {}, {}, {}
			for _, item in planner.items or {} do
				local hit = false
				if is_folder then
					hit = item.folder == filter_name
				else
					for _, t in item.tags or {} do
						if t == filter_name then
							hit = true
							break
						end
					end
				end
				if hit then
					if item.kind == "note" then
						table.insert(matched_notes, item)
					elseif item.state == "done" then
						table.insert(matched_done, item)
					else
						table.insert(matched_open, item)
					end
				end
			end
			local total = #matched_notes + #matched_open + #matched_done
			local badge: Element? = if is_folder
				then pill { label = "Folder", icon = "folder", tone = "neutral", size = "normal" }
				else pill { label = filter_name, color = tags_map[filter_name], size = "normal" }
			return ui.column {
				class = "agenda",
				grow = true,
				heading(filter_name, `{total} items`, badge),
				ui.row {
					class = "agenda-add-box",
					align = "center",
					ui.icon { name = "plus", size = "small", tone = "muted" },
					ui.input {
						id = `scoped-add-{filter_name}`,
						value = state.new_scoped_task,
						placeholder = `Add a task to {filter_name}…`,
						change = "scoped-typing",
						submit = "scoped-add",
						grow = true,
						class = "agenda-bare",
					},
				},
				ui.column {
					grow = true,
					scroll = true,
					gap = "none",
					#matched_open > 0 and ui.text { "Tasks & Events", class = "agenda-section" },
					#matched_open > 0 and list(matched_open, ""),
					#matched_notes > 0 and ui.text { "Notes", class = "agenda-section" },
					#matched_notes > 0 and list(matched_notes, ""),
					#matched_done > 0 and ui.text { "Done", class = "agenda-section" },
					#matched_done > 0 and list(matched_done, ""),
					total == 0 and ui.text { `Nothing in {filter_name} yet.`, class = "agenda-empty" },
				},
			}
		end

		return ui.column { class = "agenda", grow = true, ui.text { "Select a view from the sidebar", tone = "muted" } }
	end,
}
