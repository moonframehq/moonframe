-- The right card: switches cleanly between Inbox (Overdue / Inbox / Today),
-- the Task/Event Inspector (when an item is clicked), and the planner-aware
-- AI Assistant ("Ask Daybook") with live answers and one-click actions.

local daybook_assistant = use "daybook-assistant"
local daybook_inspector = use "daybook-inspector"
local dropdown = use "dropdown"
local icon_button = use "icon-button"
local task_check = use "task-check"
local task_row = use "task-row"

type AssistantAction = { label: string, icon: string?, msg: any }
type AssistantTurn = { from: string, text: string, actions: { AssistantAction }? }

type State = {
	mode: string,
	-- Inbox state
	filter: string,
	picking: boolean,
	adding: string,
	-- Inspector state
	selected_id: string?,
	insp_draft_id: string?,
	insp_title: string,
	insp_body: string,
	insp_subtask: string,
	insp_tag_menu: boolean,
	-- Assistant state
	asst_messages: { AssistantTurn },
	asst_draft: string,
	-- Latest snapshot from provider/planner (transient)
	planner: any,
}

local FILTERS: { [string]: string } = { all = "All", tasks = "Tasks", notes = "Notes" }
local TIMES = { "09:00", "11:00", "14:00", "16:00", "18:00", "" }

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

local function load_insp_drafts(state: State, item: any)
	if item and state.insp_draft_id ~= item.id then
		state.insp_draft_id = item.id
		state.insp_title = item.title or ""
		state.insp_body = item.body or ""
		state.insp_subtask = ""
		state.insp_tag_menu = false
	end
end

local function cycle_in(list: { string }, current: string?): string
	for i, value in list do
		if value == current then
			return list[(i % #list) + 1]
		end
	end
	return list[1]
end

-- Computes a live, truthful answer and optional planner mutations/actions from
-- the user's current planner state.
local function answer_prompt(prompt: string, planner: any): (AssistantTurn, { any })
	local low = string.lower(prompt)
	local items: { any } = (planner and planner.items) or {}
	local today = (planner and planner.today) or ""
	local sends: { any } = {}

	-- Action: create a packing list note
	if string.find(low, "packing", 1, true) or string.find(low, "create a note", 1, true) then
		local title = if string.find(low, "packing", 1, true) then "Packing list" else "Quick note"
		local body = if string.find(low, "packing", 1, true)
			then "## Essentials\n\n- Passport & boarding pass\n- Laptop, phone & USB-C charger\n- Rain jacket & walking shoes\n- Medications & toiletries"
			else "## Notes\n\nCreated from Daybook Assistant."
		table.insert(sends, {
			to = "provider:planner",
			msg = { add = title, kind = "note", emoji = "🧳", cover = "ocean", folder = "Personal", body = body },
		})
		return {
			from = "assistant",
			text = `I created a new **{title}** note in your Personal folder with a starter checklist.\n\nOpen Notes in the sidebar or click below to view and edit it.`,
			actions = {
				{ label = "Open Notes", icon = "file-text", msg = { nav_view = "notes" } },
			},
		}, sends
	end

	-- Action: add task from prompt ("add task ...")
	local task_title = string.match(prompt, "^[Aa]dd%s+task%s+(.+)$") or string.match(prompt, "^[Cc]reate%s+task%s+(.+)$")
	if task_title and string.match(task_title, "%S") then
		table.insert(sends, { to = "provider:planner", msg = { add = task_title, date = today } })
		return {
			from = "assistant",
			text = `Added **“{task_title}”** to Today's schedule.`,
			actions = { { label = "Go to Today", icon = "sun", msg = { nav_view = "today" } } },
		}, sends
	end

	-- Overdue & rescheduling
	if string.find(low, "overdue", 1, true) or string.find(low, "reschedule", 1, true) then
		local overdue: { string } = {}
		for _, it in items do
			if it.overdue then
				table.insert(overdue, `• {it.title} ({it.when or it.date})`)
			end
		end
		if #overdue == 0 then
			return {
				from = "assistant",
				text = "You have **no overdue tasks** — everything is caught up!",
			}, sends
		end
		return {
			from = "assistant",
			text = `You have {#overdue} overdue {if #overdue == 1 then "task" else "tasks"}:\n\n{table.concat(overdue, "\n")}\n\nWant me to move {if #overdue == 1 then "it" else "all of them"} to Today?`,
			actions = {
				{ label = "Move overdue to Today", icon = "calendar-check", msg = "asst-reschedule-overdue" },
			},
		}, sends
	end

	-- Habits & streaks
	if string.find(low, "habit", 1, true) or string.find(low, "streak", 1, true) then
		local lines: { string } = {}
		local done_n = 0
		for _, it in items do
			if it.habit then
				if it.done then
					done_n += 1
				end
				local goal_text = if it.goal and it.goal > 1 then ` · {it.progress or 0}/{it.goal}` else ""
				local mark = if it.done then "✓" else "○"
				table.insert(lines, `{mark} {it.title} — {it.streak or 0}d streak{goal_text}`)
			end
		end
		if #lines == 0 then
			return {
				from = "assistant",
				text = "You don't have any daily habits tracked yet. Open Habits to add your first one!",
				actions = { { label = "Open Habits", icon = "zap", msg = { nav_view = "habits" } } },
			}, sends
		end
		return {
			from = "assistant",
			text = `Today's habits ({done_n}/{#lines} done):\n\n{table.concat(lines, "\n")}`,
			actions = { { label = "Open Habits", icon = "zap", msg = { nav_view = "habits" } } },
		}, sends
	end

	-- Plan my week / upcoming
	if string.find(low, "week", 1, true) or string.find(low, "upcoming", 1, true) then
		local lines: { string } = {}
		local details: { any } = (planner and planner.day_details) or {}
		for i = 1, math.min(5, #details) do
			local d = details[i]
			local day_items: { string } = {}
			for _, it in items do
				if it.date == d.id and it.kind ~= "note" and it.state ~= "done" then
					table.insert(day_items, if it.time then `{it.title} ({it.time})` else it.title)
				end
			end
			if #day_items > 0 then
				local label = if d.today then "Today" else `{d.full_weekday} ({d.short_date})`
				table.insert(lines, `{label}: {table.concat(day_items, ", ")}`)
			end
		end
		local summary = if #lines > 0
			then `Here is your upcoming 5-day workload:\n\n{table.concat(lines, "\n\n")}`
			else "Your next 5 days are completely clear."
		return {
			from = "assistant",
			text = summary,
			actions = {
				{ label = "Upcoming Board", icon = "columns-2", msg = { nav_view = "upcoming" } },
				{ label = "3-Day Calendar", icon = "calendar-days", msg = { nav_view = "calendar" } },
			},
		}, sends
	end

	-- Default / "What's on my plate today?"
	local ev_lines: { string }, task_lines: { string } = {}, {}
	local overdue_n = 0
	for _, it in items do
		if it.overdue then
			overdue_n += 1
		elseif it.date == today and it.kind ~= "note" and it.state ~= "done" then
			local prefix = if it.time then `{it.time} · ` else ""
			if it.kind == "event" then
				table.insert(ev_lines, `• {prefix}{it.title}`)
			else
				table.insert(task_lines, `• {prefix}{it.title}`)
			end
		end
	end
	local parts: { string } = {}
	if #ev_lines > 0 then
		table.insert(parts, `Events today ({#ev_lines}):\n{table.concat(ev_lines, "\n")}`)
	end
	if #task_lines > 0 then
		table.insert(parts, `Open tasks today ({#task_lines}):\n{table.concat(task_lines, "\n")}`)
	end
	if overdue_n > 0 then
		table.insert(parts, `You also have {overdue_n} overdue {if overdue_n == 1 then "task" else "tasks"} waiting in Inbox.`)
	end
	if #parts == 0 then
		table.insert(parts, "Today is clear — no open tasks or events left! You can type “Add task …” to schedule something new.")
	end
	local actions: { AssistantAction } = {
		{ label = "View Calendar", icon = "calendar-days", msg = { nav_view = "calendar" } },
	}
	if overdue_n > 0 then
		table.insert(actions, 1, { label = "Move overdue to Today", icon = "calendar-check", msg = "asst-reschedule-overdue" })
	end
	return {
		from = "assistant",
		text = table.concat(parts, "\n\n"),
		actions = actions,
	}, sends
end

return mod {
	region = "right",
	title = "Inbox",
	subscribe = { "provider/planner" },

	styles = {
		[".inbox"] = { padding = { 26, 22, 16, 22 }, gap = 10 },
		[".inbox-title"] = { size = "title", weight = "bold" },
		[".inbox-new"] = { padding = { 4, 8 }, gap = 10 },
		[".inbox-section"] = { size = "small", weight = "semibold", color = "muted-foreground", padding = { 12, 8, 2, 8 } },
		[".inbox-note"] = { padding = { 8, 8 }, radius = 10, gap = 12 },
		[".inbox-note:hover"] = { background = "list-hover" },
		[".bare-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 2, 0 } },
	},

	init = function(previous: State?): State
		return {
			mode = previous and previous.mode or "inbox",
			filter = previous and previous.filter or "all",
			picking = false,
			adding = "",
			selected_id = nil,
			insp_draft_id = nil,
			insp_title = "",
			insp_body = "",
			insp_subtask = "",
			insp_tag_menu = false,
			asst_messages = {},
			asst_draft = "",
			planner = nil,
		}
	end,

	update = function(state: State, msg)
		if msg == "pick-filter" then
			state.picking = not state.picking
			return
		elseif msg == "close-filter" then
			state.picking = false
			return
		elseif msg == "insp-open-tags" then
			state.insp_tag_menu = not state.insp_tag_menu
			return
		elseif msg == "insp-close-tags" then
			state.insp_tag_menu = false
			return
		elseif msg == "asst-clear" then
			state.asst_messages = {}
			state.asst_draft = ""
			return
		elseif msg == "asst-send" then
			if string.match(state.asst_draft, "%S") then
				local prompt = state.asst_draft
				state.asst_draft = ""
				table.insert(state.asst_messages, { from = "user", text = prompt })
				local reply, sends = answer_prompt(prompt, state.planner)
				table.insert(state.asst_messages, reply)
				if #sends > 0 then
					return state, sends
				end
			end
			return
		elseif msg == "asst-reschedule-overdue" then
			table.insert(state.asst_messages, {
				from = "assistant",
				text = "Done — moved your overdue tasks to **Today**.",
				actions = { { label = "Go to Today", icon = "sun", msg = { nav_view = "today" } } },
			})
			return state, { { to = "provider:planner", msg = { reschedule_overdue = true } } }
		end

		if type(msg) ~= "table" then
			return
		end

		if msg.topic == "provider/planner" then
			state.planner = msg.value
			if state.mode == "detail" and state.selected_id then
				local item = find_item(state.planner, state.selected_id)
				if item then
					load_insp_drafts(state, item)
				else
					state.mode = "inbox"
					state.selected_id = nil
					return state, {
						{ to = "topbar", msg = { right_mode = "inbox" } },
						{ to = "agenda", msg = { selected_id = "" } },
					}
				end
			end
			return
		end

		if msg.mode then
			state.mode = msg.mode
			local sends: { any } = { { to = "topbar", msg = { right_mode = msg.mode } } }
			if msg.mode ~= "detail" then
				state.selected_id = nil
				table.insert(sends, { to = "agenda", msg = { selected_id = "" } })
			end
			return state, sends
		elseif msg.inspect then
			state.selected_id = msg.inspect
			state.mode = "detail"
			load_insp_drafts(state, find_item(state.planner, msg.inspect))
			return state, {
				{ to = "topbar", msg = { right_mode = "detail" } },
				{ to = "agenda", msg = { selected_id = msg.inspect } },
			}
		elseif msg.open_note then
			return state, { { to = "agenda", msg = { open_note = msg.open_note } } }
		elseif msg.nav_view then
			return state, { { to = "sidebar", msg = { view = msg.nav_view } } }
		elseif msg.pick then
			state.filter = msg.pick
			state.picking = false
		elseif msg.msg == "typing" then
			state.adding = msg.value
		elseif msg.msg == "add" then
			state.adding = ""
			if string.match(msg.value, "%S") then
				return state, { { to = "provider:planner", msg = { add = msg.value, inbox = true } } }
			end
		elseif msg.msg == "insp-title-typing" then
			state.insp_title = msg.value
		elseif msg.msg == "insp-title-save" and state.selected_id then
			state.insp_title = msg.value
			if string.match(msg.value, "%S") then
				return state, { { to = "provider:planner", msg = { update = state.selected_id, title = msg.value } } }
			end
		elseif msg.msg == "insp-body-typing" then
			state.insp_body = msg.value
		elseif msg.msg == "insp-body-save" and state.selected_id then
			state.insp_body = msg.value
			return state, { { to = "provider:planner", msg = { update = state.selected_id, body = msg.value } } }
		elseif msg.msg == "insp-sub-typing" then
			state.insp_subtask = msg.value
		elseif msg.msg == "insp-sub-add" and state.selected_id then
			local title = msg.value
			state.insp_subtask = ""
			if string.match(title, "%S") then
				return state, { { to = "provider:planner", msg = { subtask_add = state.selected_id, title = title } } }
			end
		elseif msg.insp_cycle_date then
			local item = find_item(state.planner, msg.insp_cycle_date)
			local quick = state.planner and state.planner.quick_dates
			local today = (quick and quick.today) or (state.planner and state.planner.today) or ""
			local tomorrow = (quick and quick.tomorrow) or today
			local plus3 = (quick and quick.plus3) or tomorrow
			local next_date = cycle_in({ today, tomorrow, plus3, "" }, (item and item.date) or "")
			return state, { { to = "provider:planner", msg = { update = msg.insp_cycle_date, date = next_date } } }
		elseif msg.insp_cycle_time then
			local item = find_item(state.planner, msg.insp_cycle_time)
			local cur = item and item.time or ""
			if #cur == 4 then
				cur = `0{cur}`
			end
			local next_time = cycle_in(TIMES, cur)
			return state, { { to = "provider:planner", msg = { update = msg.insp_cycle_time, time = next_time } } }
		elseif msg.insp_cycle_folder then
			local item = find_item(state.planner, msg.insp_cycle_folder)
			local folders: { string } = (state.planner and state.planner.folders) or {}
			local list = { "" }
			for _, f in folders do
				table.insert(list, f)
			end
			local next_folder = cycle_in(list, (item and item.folder) or "")
			return state, { { to = "provider:planner", msg = { update = msg.insp_cycle_folder, folder = next_folder } } }
		elseif msg.insp_toggle_tag and state.selected_id then
			state.insp_tag_menu = false
			return state, { { to = "provider:planner", msg = { update = state.selected_id, toggle_tag = msg.insp_toggle_tag } } }
		elseif msg.insp_delete then
			state.mode = "inbox"
			state.selected_id = nil
			state.insp_draft_id = nil
			return state, {
				{ to = "provider:planner", msg = { remove = msg.insp_delete } },
				{ to = "topbar", msg = { right_mode = "inbox" } },
				{ to = "agenda", msg = { selected_id = "" } },
			}
		elseif msg.msg == "asst-typing" then
			state.asst_draft = msg.value
		elseif msg.msg == "asst-submit" then
			local prompt = msg.value
			state.asst_draft = ""
			if string.match(prompt, "%S") then
				table.insert(state.asst_messages, { from = "user", text = prompt })
				local reply, sends = answer_prompt(prompt, state.planner)
				table.insert(state.asst_messages, reply)
				if #sends > 0 then
					return state, sends
				end
			end
		elseif msg.asst_prompt then
			local prompt = msg.asst_prompt
			table.insert(state.asst_messages, { from = "user", text = prompt })
			local reply, sends = answer_prompt(prompt, state.planner)
			table.insert(state.asst_messages, reply)
			if #sends > 0 then
				return state, sends
			end
		elseif msg.toggle or msg.cycle or msg.subtask_toggle or msg.subtask_remove or msg.habit_step then
			return state, { { to = "provider:planner", msg = msg } }
		end
	end,

	view = function(state: State, app)
		local planner = app.planner
		if not planner then
			return ui.column { class = "inbox" }
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

		if state.mode == "assistant" then
			return daybook_assistant {
				messages = state.asst_messages,
				draft = state.asst_draft,
			}
		end

		if state.mode == "detail" and state.selected_id then
			local item = find_item(planner, state.selected_id)
			if item then
				local title_draft = if state.insp_draft_id == item.id then state.insp_title else (item.title or "")
				local body_draft = if state.insp_draft_id == item.id then state.insp_body else (item.body or "")
				return daybook_inspector {
					item = item,
					planner = planner,
					tags_map = tags_map,
					notes_map = notes_map,
					title_draft = title_draft,
					body_draft = body_draft,
					new_subtask = state.insp_subtask,
					tag_menu = state.insp_tag_menu,
				}
			end
		end

		local show_tasks = state.filter ~= "notes"
		local show_notes = state.filter ~= "tasks"

		local function row(item: any): Element
			if item.kind == "note" then
				return ui.row {
					class = "inbox-note",
					align = "center",
					msg = { open_note = item.id },
					ui.text { item.emoji or "📝" },
					ui.text { item.title, weight = "medium", truncate = true },
				}
			end
			local pills = {}
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

		local overdue: { any }, undated: { any }, today: { any } = {}, {}, {}
		for _, item in planner.items or {} do
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
		table.sort(today, function(a: any, b: any)
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

		local sections: { Element } = {}
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
				icon_button { icon = "sparkles", tooltip = "AI Assistant", size = "small", msg = { mode = "assistant" } },
				icon_button { icon = "panel-right", tooltip = "Hide right pane", msg = "toggle-right", to = "shell" },
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
