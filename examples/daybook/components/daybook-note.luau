-- Rich Note page: rounded gradient cover banner with floating action buttons,
-- editable emoji + title, metadata chips (created, edited, folder, tags),
-- linked tasks with inline creator, linked events, and live prose markdown body.

local icon_button = use "icon-button"
local menu = use "menu"
local pill = use "pill"
local task_check = use "task-check"
local task_row = use "task-row"

type Props = {
	note: any,
	planner: any,
	tags_map: { [string]: string },
	selected_id: string?,
	title_draft: string,
	body_draft: string,
	preview: boolean,
	tag_menu: boolean,
	new_task: string,
}

local COVERS: { [string]: string } = {
	sunset = "135deg, #ff7e5f, #e5484d",
	ocean = "135deg, #3b82f6, #06b6d4",
	forest = "135deg, #2f9e5b, #10b981",
	aurora = "135deg, #6366f1, #ec4899",
	amber = "135deg, #f59e0b, #ef4444",
	plum = "135deg, #8b5cf6, #3b6fd8",
}

return component {
	title = "Daybook Note Page",
	category = "Daybook",
	icon = "file-text",
	description = "Rich note page with cover banner, metadata chips, linked tasks and events, and prose markdown body.",
	styles = {
		[".note-scroll"] = { gap = 16, padding = { 0, 2, 24, 2 } },
		[".note-cover"] = { radius = 14, padding = { 12, 14 }, clip = true },
		[".note-cover-btn"] = { background = "#ffffffd8", color = "#1b1b1d", radius = 16 },
		[".note-cover-btn:hover"] = { background = "#ffffff" },
		[".note-cover-fav"] = { background = "#ffffffe6", color = "warning", radius = 16 },
		[".note-title-row"] = { gap = 10, padding = { 2, 0 } },
		[".note-emoji-btn"] = { radius = 10, padding = { 4, 8 } },
		[".note-emoji-btn:hover"] = { background = "list-hover" },
		[".note-emoji"] = { font_size = 28 },
		[".note-title-input"] = { background = "#00000000", border = "none", shadow = "none", font_size = 26, weight = "bold", padding = { 2, 0 } },
		[".note-meta"] = { gap = 6 },
		[".note-chip"] = { background = "surface-soft", radius = 8, padding = { 4, 9 }, gap = 5, color = "muted-foreground", size = "small" },
		[".note-chip:hover"] = { background = "list-hover", color = "foreground" },
		[".note-add-tag"] = { border = "all", border_color = "border", radius = 12, padding = { 2, 7 }, color = "muted-foreground" },
		[".note-add-tag:hover"] = { background = "list-hover", color = "foreground" },
		[".note-section-head"] = { size = "large", weight = "semibold", padding = { 6, 0, 2, 0 } },
		[".note-inline-add"] = { padding = { 4, 8 }, gap = 10 },
		[".note-bare-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 } },
		[".note-body-box"] = { background = "surface-soft", radius = 12, padding = { 12, 14 } },
		[".note-body-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 2, 0 } },
	},
	example = {
		note = { id = "n1", title = "Trip to Porto", emoji = "🧳", cover = "sunset", tags = {}, body = "" },
		planner = { items = {}, tags = {}, folders = {} },
		tags_map = {},
		title_draft = "Trip to Porto",
		body_draft = "",
		preview = false,
		tag_menu = false,
		new_task = "",
	},

	build = function(props: Props)
		local note = props.note
		local planner = props.planner
		local tags_map = props.tags_map or {}
		local cover_grad = COVERS[note.cover or "sunset"] or COVERS.sunset

		local linked_tasks: { any } = {}
		local linked_events: { any } = {}
		for _, item in planner.items or {} do
			if item.note_id == note.id then
				if item.kind == "event" then
					table.insert(linked_events, item)
				elseif item.kind == "task" then
					table.insert(linked_tasks, item)
				end
			end
		end

		local cover_bar = ui.row {
			class = "note-cover",
			height = 124,
			shrink = false,
			align = "start",
			justify = "end",
			style = { gradient = cover_grad },
			ui.row {
				gap = "small",
				ui.row {
					class = "note-cover-btn",
					icon_button { icon = "image", tooltip = "Change cover", size = "small", msg = { note_cycle_cover = note.id } },
				},
				ui.row {
					class = if note.favorite then "note-cover-fav" else "note-cover-btn",
					icon_button {
						icon = if note.favorite then "star-fill" else "star",
						tooltip = if note.favorite then "Remove from favorites" else "Add to favorites",
						size = "small",
						msg = { note_toggle_fav = note.id },
					},
				},
				ui.row {
					class = "note-cover-btn",
					icon_button {
						icon = if props.preview then "pencil" else "eye",
						tooltip = if props.preview then "Edit markdown" else "Preview markdown",
						size = "small",
						msg = "note-toggle-preview",
					},
				},
				ui.row {
					class = "note-cover-btn",
					icon_button { icon = "trash-2", tooltip = "Delete note", size = "small", msg = { note_delete = note.id } },
				},
			},
		}

		local title_row = ui.row {
			class = "note-title-row",
			align = "center",
			ui.row {
				class = "note-emoji-btn",
				tooltip = "Cycle icon",
				msg = { note_cycle_emoji = note.id },
				ui.text { note.emoji or "📝", class = "note-emoji" },
			},
			ui.input {
				id = `note-title-{note.id}`,
				value = props.title_draft,
				placeholder = "Untitled note",
				change = "note-title-typing",
				blur = "note-title-save",
				submit = "note-title-save",
				grow = true,
				class = "note-title-input",
			},
		}

		local note_tag_set: { [string]: boolean } = {}
		for _, t in note.tags or {} do
			note_tag_set[t] = true
		end
		local tag_options: { any } = { { heading = "Tags" } }
		for _, t in planner.tags or {} do
			table.insert(tag_options, { id = t.name, label = t.name, checked = note_tag_set[t.name] == true })
		end

		local chips: { Element } = {}
		if note.created then
			table.insert(chips, ui.row {
				class = "note-chip",
				align = "center",
				hug = true,
				ui.icon { name = "calendar", size = "small" },
				ui.text { `Created {note.created}`, size = "small" },
			})
		end
		if note.edited then
			table.insert(chips, ui.row {
				class = "note-chip",
				align = "center",
				hug = true,
				ui.icon { name = "history", size = "small" },
				ui.text { note.edited, size = "small" },
			})
		end
		table.insert(chips, ui.row {
			class = "note-chip",
			align = "center",
			hug = true,
			tooltip = "Click to cycle folder",
			msg = { note_cycle_folder = note.id },
			ui.icon { name = "folder", size = "small" },
			ui.text { note.folder or "No folder", size = "small" },
		})
		for _, tag_name in note.tags or {} do
			table.insert(chips, pill {
				label = tag_name,
				color = tags_map[tag_name],
				msg = { note_toggle_tag = tag_name },
				tooltip = "Click to remove tag",
			})
		end
		table.insert(chips, ui.popover {
			open = props.tag_menu,
			dismiss = "note-close-tags",
			side = "bottom",
			width = 180,
			content = menu { items = tag_options, event = "note_toggle_tag" },
			ui.row {
				class = "note-add-tag",
				align = "center",
				msg = "note-open-tags",
				tooltip = "Add or remove tags",
				ui.icon { name = "plus", size = "small" },
			},
		})

		local function build_row(item: any): Element
			local pills = {}
			if item.days_left and item.state ~= "done" then
				table.insert(pills, { label = item.days_left, tone = "neutral" })
			end
			for _, t in item.tags or {} do
				table.insert(pills, { label = t, color = tags_map[t] })
			end
			return task_row {
				title = item.title,
				state = item.state,
				event = item.kind == "event",
				date = item.when,
				time = item.time,
				due = item.due,
				tags = pills,
				selected = props.selected_id == item.id,
				toggle = { toggle = item.id },
				msg = { inspect = item.id },
				context = if item.kind == "task" then { cycle = item.id } else nil,
			}
		end

		local task_rows: { Element } = {}
		for _, t in linked_tasks do
			table.insert(task_rows, build_row(t))
		end

		local event_rows: { Element } = {}
		for _, e in linked_events do
			table.insert(event_rows, build_row(e))
		end

		local body_section: Element = if props.preview
			then ui.column {
				class = "note-body-box",
				ui.markdown { text = if props.body_draft ~= "" then props.body_draft else "_No notes yet. Click the pencil icon above to start writing._" },
			}
			else ui.column {
				class = "note-body-box",
				ui.input {
					id = `note-body-{note.id}`,
					value = props.body_draft,
					placeholder = "Write markdown notes, ideas, or an itinerary…",
					multiline = true,
					prose = true,
					rows = 8,
					max_rows = 24,
					change = "note-body-typing",
					blur = "note-body-save",
					class = "note-body-input",
				},
			}

		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column {
				class = "note-scroll",
				shrink = false,
				cover_bar,
				title_row,
				ui.row { class = "note-meta", align = "center", wrap = true, table.unpack(chips) },
				ui.column {
					gap = "none",
					ui.text { "Checklist", class = "note-section-head" },
					#task_rows > 0 and ui.column { gap = "none", table.unpack(task_rows) },
					ui.row {
						class = "note-inline-add",
						align = "center",
						task_check {},
						ui.input {
							id = `note-add-task-{note.id}`,
							value = props.new_task,
							placeholder = "Add a task to this note…",
							change = "note-task-typing",
							submit = "note-task-add",
							grow = true,
							class = "note-bare-input",
						},
					},
				},
				#event_rows > 0 and ui.column {
					gap = "none",
					ui.text { "Events", class = "note-section-head" },
					table.unpack(event_rows),
				},
				ui.column {
					gap = "small",
					ui.text { "Notes", class = "note-section-head" },
					body_section,
				},
			},
		}
	end,
}
