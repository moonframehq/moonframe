-- Task / Event Inspector pane (right card): edit title, toggle completion,
-- adjust date/time/priority/folder/tags, track habit streaks, manage subtasks,
-- and write prose notes.

local icon_button = use "icon-button"
local menu = use "menu"
local pill = use "pill"
local progress = use "progress"
local task_check = use "task-check"

type Props = {
	item: any,
	planner: any,
	tags_map: { [string]: string },
	notes_map: { [string]: any },
	title_draft: string,
	body_draft: string,
	new_subtask: string,
	tag_menu: boolean,
}

return component {
	title = "Daybook Inspector",
	category = "Daybook",
	icon = "sliders-horizontal",
	description = "Task and event inspector with metadata chips, habit streak, subtasks, and notes.",
	styles = {
		[".insp"] = { padding = { 24, 22, 16, 22 }, gap = 14 },
		[".insp-dot"] = { background = "danger", radius = 5 },
		[".insp-title"] = { background = "#00000000", border = "none", shadow = "none", font_size = 20, weight = "bold", padding = { 2, 0 } },
		[".insp-pills"] = { gap = 6 },
		[".insp-chip"] = { background = "surface-soft", radius = 8, padding = { 4, 9 }, gap = 5, color = "muted-foreground", size = "small" },
		[".insp-chip:hover"] = { background = "list-hover", color = "foreground" },
		[".insp-add-tag"] = { border = "all", border_color = "border", radius = 12, padding = { 2, 7 }, color = "muted-foreground" },
		[".insp-add-tag:hover"] = { background = "list-hover", color = "foreground" },
		[".insp-section"] = { size = "small", weight = "semibold", color = "muted-foreground", padding = { 4, 0, 0, 0 } },
		[".insp-sub-row"] = { padding = { 4, 6 }, radius = 8, gap = 8 },
		[".insp-sub-row:hover"] = { background = "list-hover" },
		[".insp-sub-done"] = { color = "muted-foreground" },
		[".insp-bare"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		[".insp-notes"] = { background = "surface-soft", radius = 12, padding = { 10, 12 } },
		[".insp-step"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 2, 8 } },
		[".insp-step:hover"] = { background = "list-hover" },
	},
	example = {
		item = { id = "i1", kind = "task", title = "Pushups x100", state = "open", tags = {}, subtasks = {} },
		planner = { tags = {}, folders = {} },
		tags_map = {},
		notes_map = {},
		title_draft = "Pushups x100",
		body_draft = "",
		new_subtask = "",
		tag_menu = false,
	},

	build = function(props: Props)
		local item = props.item
		local planner = props.planner
		local tags_map = props.tags_map or {}
		local notes_map = props.notes_map or {}

		local lead: Element = if item.kind == "event"
			then ui.row {
				width = 24,
				height = 24,
				align = "center",
				justify = "center",
				ui.row { class = "insp-dot", width = 10, height = 10 },
			}
			else task_check { state = item.state, msg = { toggle = item.id }, tooltip = "Mark done" }

		local top = ui.row {
			align = "center",
			gap = "small",
			lead,
			ui.spacer(),
			icon_button { icon = "trash-2", tooltip = "Delete item", size = "small", msg = { insp_delete = item.id } },
			icon_button { icon = "close", tooltip = "Back to Inbox", size = "small", msg = { mode = "inbox" } },
		}

		local title_field = ui.input {
			id = `insp-title-{item.id}`,
			value = props.title_draft,
			placeholder = "Title",
			change = "insp-title-typing",
			blur = "insp-title-save",
			submit = "insp-title-save",
			class = "insp-title",
		}

		local item_tags: { [string]: boolean } = {}
		for _, t in item.tags or {} do
			item_tags[t] = true
		end
		local tag_options: { any } = { { heading = "Tags" } }
		for _, t in planner.tags or {} do
			table.insert(tag_options, { id = t.name, label = t.name, checked = item_tags[t.name] == true })
		end

		local chips: { Element } = {}
		if item.habit then
			local streak = item.streak or 0
			table.insert(chips, pill {
				label = if streak == 1 then "1 day" else `{streak} in a row`,
				icon = "zap",
				tone = "danger",
				size = "normal",
			})
			if item.goal and item.goal > 1 then
				local prog = item.progress or 0
				local step_val = if item.goal >= 50 then 10 elseif item.goal >= 10 then 5 else 1
				table.insert(chips, pill { label = `Daily · {prog}/{item.goal}`, tone = "neutral", size = "normal" })
				table.insert(chips, ui.row {
					class = "insp-step",
					msg = { habit_step = item.id, delta = step_val },
					tooltip = `Add +{step_val}`,
					ui.text { `+{step_val}`, size = "small", weight = "medium" },
				})
			end
		end

		local date_label = if item.date == planner.today
			then "Today"
			elseif item.when then item.when
			else "No date"
		table.insert(chips, ui.row {
			class = "insp-chip",
			align = "center",
			hug = true,
			tooltip = "Click to cycle date (Today / Tomorrow / +3d / Clear)",
			msg = { insp_cycle_date = item.id },
			ui.icon { name = "calendar", size = "small", tone = if item.due or item.overdue then "danger" else nil },
			ui.text { date_label, size = "small", tone = if item.due or item.overdue then "danger" else nil },
		})

		table.insert(chips, ui.row {
			class = "insp-chip",
			align = "center",
			hug = true,
			tooltip = "Click to cycle time",
			msg = { insp_cycle_time = item.id },
			ui.icon { name = "clock", size = "small" },
			ui.text { item.time or "Add time", size = "small" },
		})

		if item.kind == "task" then
			local prio_label = if item.state == "important"
				then "Important"
				elseif item.state == "progress" then "In progress"
				elseif item.state == "done" then "Done"
				else "Normal"
			local prio_tone = if item.state == "important"
				then "warning"
				elseif item.state == "progress" then "danger"
				elseif item.state == "done" then "success"
				else "neutral"
			table.insert(chips, pill {
				label = prio_label,
				tone = prio_tone,
				msg = { cycle = item.id },
				tooltip = "Click to cycle priority",
			})
		end

		table.insert(chips, ui.row {
			class = "insp-chip",
			align = "center",
			hug = true,
			tooltip = "Click to cycle folder",
			msg = { insp_cycle_folder = item.id },
			ui.icon { name = "folder", size = "small" },
			ui.text { item.folder or "Folder", size = "small" },
		})

		local linked_note = item.note_id and notes_map[item.note_id]
		if linked_note then
			table.insert(chips, ui.row {
				class = "insp-chip",
				align = "center",
				hug = true,
				tooltip = "Open linked note",
				msg = { open_note = linked_note.id },
				ui.text { linked_note.emoji or "📝", size = "small" },
				ui.text { linked_note.title, size = "small", truncate = true },
			})
		end

		for _, tag_name in item.tags or {} do
			table.insert(chips, pill {
				label = tag_name,
				color = tags_map[tag_name],
				msg = { insp_toggle_tag = tag_name },
				tooltip = "Click to remove tag",
			})
		end

		table.insert(chips, ui.popover {
			open = props.tag_menu,
			dismiss = "insp-close-tags",
			side = "bottom",
			align = "end",
			width = 180,
			content = menu { items = tag_options, event = "insp_toggle_tag" },
			ui.row {
				class = "insp-add-tag",
				align = "center",
				msg = "insp-open-tags",
				tooltip = "Add or remove tags",
				ui.icon { name = "plus", size = "small" },
			},
		})

		local habit_bar: Element? = nil
		if item.habit and item.goal and item.goal > 1 then
			habit_bar = progress { value = math.min(1, (item.progress or 0) / item.goal) }
		end

		local subtask_rows: { Element } = {}
		for _, sub: any in item.subtasks or {} do
			table.insert(subtask_rows, ui.row {
				class = "insp-sub-row",
				align = "center",
				task_check { state = if sub.done then "done" else "open", msg = { subtask_toggle = item.id, subtask = sub.id } },
				ui.row {
					grow = true,
					ui.text {
						sub.title,
						strike = sub.done == true,
						class = if sub.done then "insp-sub-done" else nil,
						truncate = true,
					},
				},
				icon_button { icon = "close", size = "small", tooltip = "Remove subtask", msg = { subtask_remove = item.id, subtask = sub.id } },
			})
		end

		local subtasks_block: Element? = if item.kind == "task"
			then ui.column {
				gap = "small",
				ui.text { "Subtasks", class = "insp-section" },
				#subtask_rows > 0 and ui.column { gap = "none", table.unpack(subtask_rows) },
				ui.row {
					align = "center",
					gap = "small",
					style = { padding = { 2, 6 } },
					ui.icon { name = "plus", size = "small", tone = "muted" },
					ui.input {
						id = `insp-sub-{item.id}`,
						value = props.new_subtask,
						placeholder = "Write a subtask…",
						change = "insp-sub-typing",
						submit = "insp-sub-add",
						grow = true,
						class = "insp-bare",
					},
				},
			}
			else nil

		local notes_block = ui.column {
			gap = "small",
			ui.text { "Notes", class = "insp-section" },
			ui.column {
				class = "insp-notes",
				ui.input {
					id = `insp-body-{item.id}`,
					value = props.body_draft,
					placeholder = "Write a note or context…",
					multiline = true,
					prose = true,
					rows = 5,
					max_rows = 16,
					change = "insp-body-typing",
					blur = "insp-body-save",
					class = "insp-bare",
				},
			},
		}

		return ui.column {
			class = "insp",
			grow = true,
			top,
			ui.column {
				grow = true,
				scroll = true,
				gap = "normal",
				title_field,
				ui.row { class = "insp-pills", align = "center", wrap = true, table.unpack(chips) },
				habit_bar,
				subtasks_block,
				notes_block,
			},
		}
	end,
}
