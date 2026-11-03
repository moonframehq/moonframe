local picker = use "picker"
local date_picker = use "date-picker"
local text_editor = use "text-editor"
local mention = use "mention-field"
local comment = use "comment-card"
local emoji = use "emoji-picker"
local dropzone = use "dropzone"
local file_card = use "file-card"
local task_check = use "task-check"

return component {
	title = "Orbit task detail", category = "Data", icon = "panel-right",
	description = "Editable properties, description, subtasks, local attachments and comments.",
	when = "Orbit's task panel.", props = { task = "selected task", state = "workspace state", choices = "property choices", dates = "date picker data" },
	example = {},
	build = function(props: any)
		local task, state = props.task, props.state
		if not task then return ui.text "Select a task" end
		local function property(label: string, field: string, options: any, icon: string)
			return ui.row { class = "orbit-property", align = "center",
				ui.row { width = 94, ui.text { label, class = "orbit-label" } },
				picker { options = options, selected = task[field], icon = icon, event = field,
					open = state.picker == field, toggle = { picker = field }, dismiss = "close-picker",
					query = state.picker_query, search = "picker-search", id = `pick-{field}`, highlighted = state.highlighted },
			}
		end
		local checks, files, comments = {}, {}, {}
		for index, sub in task.subtasks do
			table.insert(checks, ui.row { align = "center", gap = "small",
				task_check { state = if sub.done then "done" else "open", msg = { subtask = index } },
				ui.text { sub.title, strike = sub.done, tone = if sub.done then "muted" else nil, size = "small" },
			})
		end
		for index, file in task.files do
			table.insert(files, file_card { name = file.name, detail = file.path or "Sample attachment", layout = "row",
				remove = { remove_file = index } })
		end
		for index, entry in task.comments do
			local reactions = {}
			for _, reaction in entry.reactions do
				local count = reaction.others + (if reaction.mine then 1 else 0)
				if count > 0 then table.insert(reactions, { emoji = reaction.emoji, count = count, mine = reaction.mine }) end
			end
			table.insert(comments, comment { author = entry.author, age = entry.age, body = entry.body,
				reactions = reactions, react = `react-{index}`, add_reaction = { reaction_picker = index },
				picker_open = state.reaction_picker == index,
				picker = emoji { query = state.emoji_query, search = "emoji-search", event = "emoji", cancel = "close-reactions", focus = true },
			})
		end
		return ui.column { class = "orbit-panel", grow = true,
			ui.row { class = "orbit-panel-head", align = "center", ui.text { task.id, tone = "muted", size = "small" }, ui.spacer(),
				ui.button { if task.status == "done" then "Reopen" else "Complete", icon = "circle-check", small = true,
					variant = "ghost", msg = { status = if task.status == "done" then "todo" else "done" } },
				ui.button { icon = "close", small = true, variant = "ghost", tooltip = "Close task", msg = "close-task" },
			},
			ui.column { grow = true, scroll = "y", gap = "none",
				ui.column { class = "orbit-panel-body", shrink = false,
					ui.input { id = `title-{task.id}`, value = task.title, change = "title", class = "orbit-task-title" },
					ui.column { gap = "small",
						property("Status", "status", props.choices.status, "circle-dashed"),
						property("Assignee", "assignee", props.choices.people, "user"),
						property("Priority", "priority", props.choices.priority, "flag"),
						ui.row { align = "center", gap = "small",
							ui.row { width = 94, ui.text { "Due date", class = "orbit-label" } },
							date_picker { weeks = props.dates.weeks, title = props.dates.title, selected = task.date, value = task.due,
								open = state.picker == "date", toggle = { picker = "date" }, dismiss = "close-picker", event = "due",
								previous = "due-prev", next = "due-next", today = "due-today" },
						},
					},
					ui.column { gap = "small", ui.text { "Description", class = "orbit-small-head" },
						text_editor { id = `description-{task.id}`, value = task.description, change = "description",
							preview = state.preview, switch = "description_view", rows = 5, max_rows = 12 },
					},
					ui.column { gap = "small", ui.text { "Subtasks", class = "orbit-small-head" }, table.unpack(checks) },
					ui.column { gap = "small", ui.text { "Attachments", class = "orbit-small-head" }, table.unpack(files) },
					dropzone { compact = true, title = "Drop files to attach", drop = { attach = task.id }, browse = { browse = task.id } },
					ui.text { "Local file references · nothing is uploaded", class = "orbit-label" },
					ui.divider(),
					ui.text { "Activity", class = "orbit-small-head" },
					ui.column { gap = "normal", table.unpack(comments) },
					mention { id = `comment-{task.id}`, value = state.draft, caret = state.caret,
						people = if state.mention_closed then {} else props.choices.mentions,
						change = "comment", submit = "post-comment", dismiss = "close-mentions", highlighted = state.mention_at,
						multiline = true, rows = 2, placeholder = "Leave a comment… @ to mention" },
					ui.row { justify = "end", ui.button { "Comment", small = true, variant = "primary", msg = "post-comment", disabled = not string.find(state.draft, "%S") } },
				},
			},
		}
	end,
}
