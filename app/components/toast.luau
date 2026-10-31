-- One toast card: a level dot (or an icon), an optional source label, a
-- title, a one-line body, an optional action button and a ×. Stateless; a
-- stack of them in a window corner is `toasts`.

type Props = {
	id: any,
	level: string?,
	icon: any,
	source: string?,
	title: string?,
	body: string?,
	-- Show the whole body instead of one truncated line.
	full: boolean?,
	action: any,
	dismiss: any,
	event: string?,
	width: number?,
	to: string?,
}

local LEVEL_ICONS = {
	info = "info",
	success = "circle-check",
	warning = "triangle-alert",
	error = "circle-x",
}

return component {
	title = "Toast",
	category = "Feedback",
	icon = "bell",
	description = "One notification card: level dot or icon, source label, title, one-line body, an action button and ×.",
	when = "A single transient notification, usually inside `toasts`, which stacks them in a window corner. For a message that stays in the page use `notice`; for a blocking question use `confirm`; for an app update use `update-pill`.",
	props = {
		["id"] = "sent back on dismiss: `{ dismiss = id }`",
		["level"] = "`info` (default), `success`, `warning` or `error`: the dot and border color",
		["icon"] = "an icon instead of the level dot (`true` for the level's own icon)",
		["source"] = "small muted label over the title: a host, a mod, \"Local\"",
		["title"] = "first line",
		["body"] = "muted text under the title, cut to one line unless `full`",
		["full"] = "show the whole body, wrapped",
		["action"] = "a button under the body: { label, msg }",
		["dismiss"] = "msg sent by the ×; default `{ [event or \"dismiss\"] = id }`; `false` hides the ×",
		["event"] = "key of the default dismiss msg",
		["width"] = "pixels; default 340",
		["to"] = "mod the action and × send to, if not the drawing one",
	},
	styles = {
		[".toast"] = {
			background = "background",
			border = "all",
			border_color = "border",
			radius = 10,
			padding = 10,
			gap = 10,
			shadow = "0 8px 24px -8px #00000040, 0 1px 3px #0000001a",
		},
		[".toast-warning"] = { border_color = "warning/60" },
		[".toast-error"] = { border_color = "danger/60" },
		[".toast-dot"] = { radius = 4, background = "accent", margin = { top = 6 } },
		[".toast-dot-success"] = { background = "success" },
		[".toast-dot-warning"] = { background = "warning" },
		[".toast-dot-error"] = { background = "danger" },
		[".toast-icon"] = { color = "accent", margin = { top = 1 } },
		[".toast-icon-success"] = { color = "success" },
		[".toast-icon-warning"] = { color = "warning" },
		[".toast-icon-error"] = { color = "danger" },
		[".toast-content"] = { gap = 2 },
		[".toast-source"] = { size = "small", color = "muted-foreground" },
		[".toast-title"] = { weight = "medium" },
		[".toast-body"] = { size = "small", color = "muted-foreground" },
		[".toast-action"] = { margin = { top = 6 } },
		[".toast-close"] = { padding = 3, radius = 6, color = "muted-foreground", size = "small" },
		[".toast-close:hover"] = { background = "list-hover", color = "foreground" },
	},
	example = {
		id = "git-permissions",
		level = "warning",
		source = "Local",
		title = "Local Git and PR details unavailable",
		body = "The daemon socket directory must be owned by your user and not world-writable.",
		full = true,
		action = { label = "Reconnect", msg = "reconnect" },
	},

	build = function(props: Props)
		local level = props.level or "info"
		local mark
		if props.icon then
			local name = if props.icon == true or props.icon == "true" then LEVEL_ICONS[level] else props.icon
			mark = ui.icon { name = (name or "info") :: any, class = "toast-icon toast-icon-" .. level }
		else
			mark = ui.row { class = "toast-dot toast-dot-" .. level, width = 8, height = 8 }
		end

		local dismiss = props.dismiss
		if dismiss == nil then
			dismiss = { [props.event or "dismiss"] = props.id or props.title }
		end

		local action: any = false
		if props.action then
			action = ui.row {
				class = "toast-action",
				ui.button { props.action.label, small = true, variant = "outline", msg = props.action.msg, to = props.to },
			}
		end

		return ui.row {
			class = "toast toast-" .. level,
			align = "start",
			width = props.width or 340,
			mark,
			ui.column {
				class = "toast-content",
				grow = true,
				props.source ~= nil and ui.text { props.source, class = "toast-source", truncate = true },
				props.title ~= nil and ui.text { props.title, class = "toast-title", truncate = not props.full },
				props.body ~= nil and ui.text { props.body, class = "toast-body", truncate = not props.full },
				action,
			},
			dismiss ~= false and ui.row {
				class = "toast-close",
				msg = dismiss,
				to = props.to,
				tooltip = "Dismiss",
				ui.icon "close",
			},
		}
	end,
}
