-- A task's round check: empty while open, a ring in the warning colour when
-- important, with a dot in the danger colour while in progress, and filled
-- with a check when done.

type Props = {
	state: string?,
	msg: any,
	to: string?,
	tooltip: string?,
}

return component {
	title = "Task Check",
	category = "Forms",
	icon = "circle-check",
	description = "A round check for a task: open, important (warning ring), in progress (danger ring with a dot), or done (filled with a check).",
	when = "The check at the start of a task or to-do row; a click sends `msg` for the mod to flip it. For a form option use `checkbox`; for an agent's state use `status-dot`.",
	props = {
		["state"] = "open (default), important, progress or done",
		["msg"] = "message a click sends",
		["to"] = "mod to send `msg` to, if not the drawing one",
		["tooltip"] = "text under the pointer",
	},
	styles = {
		[".task-check"] = {
			radius = 11,
			border = "all",
			border_color = "muted-foreground/45",
			color = "background",
		},
		[".task-check:hover"] = { border_color = "foreground" },
		[".task-check-important"] = { border_color = "warning" },
		[".task-check-progress"] = { border_color = "danger" },
		[".task-check-done"] = { background = "foreground", border_color = "foreground", radius = 6 },
		[".task-check-dot"] = { background = "danger", radius = 4 },
	},
	example = { state = "progress", msg = "toggle" },
	usage = [[local task_check = use "task-check"

task_check { state = if task.done then "done" else "open", msg = { toggle = task.id } }]],

	build = function(props: Props)
		local state = props.state or "open"
		local class = { "task-check" }
		if state ~= "open" then
			table.insert(class, `task-check-{state}`)
		end
		return ui.row {
			class = class,
			width = 22,
			height = 22,
			align = "center",
			justify = "center",
			shrink = false,
			msg = props.msg,
			to = props.to,
			tooltip = props.tooltip,
			state == "done" and ui.icon { name = "check", size = "small" },
			state == "progress" and ui.row { class = "task-check-dot", width = 8, height = 8 },
		}
	end,
}
