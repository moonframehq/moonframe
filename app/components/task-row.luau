-- A task or an event in a list: its check (or a dot for an event), its
-- title, struck through when done, and a line under it with its date, its
-- time (in the danger colour when due), an icon (a habit's flame) and its
-- tags as pills.

local task_check = use "task-check"
local pill = use "pill"

type Props = {
	title: string,
	state: string?,
	event: boolean?,
	date: string?,
	time: string?,
	due: boolean?,
	icon: string?,
	tags: { any }?,
	carry: any,
	accept: any,
	leading: any,
	toggle: any,
	msg: any,
	context: any,
	to: string?,
	selected: boolean?,
}

return component {
	title = "Task Row",
	category = "Lists",
	icon = "list-checks",
	description = "A task (round check, title struck through when done) or an event (a dot), with date, due time and tag pills under it.",
	when = "Rows of tasks, to-dos, habits or events in a planner, an inbox or a day view; stack them in a column under `section` headings. For a plain selectable row use `list`; for pull requests use `pr-row`.",
	props = {
		["carry"] = "data the row carries when dragged to a row or list with `accept` (to reorder or file it)",
		["accept"] = "message sent as a carried row moves over this one and is dropped on it, with `phase`, `carry` and `y` (above or below its middle) added",
		["title"] = "what to do",
		["state"] = "the check: open (default), important, progress or done",
		["event"] = "an event: a dot instead of a check",
		["date"] = "its date, as shown (\"Sat, Oct 31\")",
		["time"] = "its time (\"17:00\")",
		["due"] = "the time in the danger colour: due or late",
		["icon"] = "an icon before the date (a habit's flame)",
		["tags"] = "pills after the time: { label, color? or tone?, icon? }",
		["leading"] = "an element instead of the check (an emoji for a note)",
		["toggle"] = "message the check sends",
		["msg"] = "message a click on the row sends (open it)",
		["context"] = "message a right-click sends (a menu, or cycling its priority)",
		["to"] = "mod to send both to, if not the drawing one",
		["selected"] = "the row is the chosen one",
	},
	styles = {
		[".task-row"] = { padding = { 6, 8 }, radius = 10, gap = 10 },
		[".task-row:hover"] = { background = "list-hover" },
		[".task-row:selected"] = { background = "accent/10" },
		[".task-row-body"] = { gap = 3 },
		[".task-row-line"] = { gap = 6 },
		[".task-row-title"] = { weight = "medium" },
		[".task-row-done"] = { color = "muted-foreground" },
		[".task-row-meta"] = { color = "muted-foreground", size = "small" },
		[".task-row-due"] = { color = "danger", size = "small", weight = "medium" },
		[".task-row-event"] = { background = "danger", radius = 4 },
	},
	example = {
		title = "Review the launch checklist",
		state = "progress",
		date = "Sat, Oct 31",
		time = "17:00",
		due = true,
		tags = { { label = "Work", color = "#3b82f6" } },
	},

	build = function(props: Props)
		local state = props.state or "open"
		local done = state == "done"
		local lead
		if props.leading then
			lead = props.leading
		elseif props.event then
			lead = ui.row {
				width = 22,
				height = 22,
				align = "center",
				justify = "center",
				shrink = false,
				ui.row { class = "task-row-event", width = 8, height = 8 },
			}
		else
			lead = task_check { state = state, msg = props.toggle, to = props.to }
		end
		local meta = {}
		if props.icon then
			table.insert(meta, ui.icon { name = props.icon :: any, size = "small", class = "task-row-meta" })
		end
		if props.date then
			table.insert(meta, ui.text { props.date, class = "task-row-meta" })
		end
		if props.time then
			table.insert(meta, ui.text { props.time, class = if props.due then "task-row-due" else "task-row-meta" })
		end
		local tags: { any } = props.tags or {}
		for _, tag in tags do
			table.insert(meta, pill { label = tag.label, color = tag.color, tone = tag.tone, icon = tag.icon })
		end
		return ui.row {
			class = "task-row",
			align = "start",
			selected = props.selected,
			msg = props.msg,
			context = props.context,
			carry = props.carry,
			accept = props.accept,
			to = props.to,
			lead,
			ui.column {
				class = "task-row-body",
				grow = true,
				ui.text {
					props.title,
					class = if done then { "task-row-title", "task-row-done" } else "task-row-title",
					strike = done,
					truncate = true,
				},
				#meta > 0 and ui.row { class = "task-row-line", align = "center", table.unpack(meta) },
			},
		}
	end,
}
