-- What a panel shows when it has nothing yet: an icon, a line saying so,
-- and optionally the one action that fixes it.

type Props = {
	-- A name from the bundled icons (`IconName`).
	icon: string?,
	title: string,
	message: string?,
	action: string?,
	msg: any,
}

return component {
	title = "Empty state",
	category = "Feedback",
	icon = "inbox",
	description = "A centered icon, title, message and optional button for a panel with nothing to show yet.",
	when = "A list, panel or page with nothing to show yet, optionally with one action to fix that. For loading use `loading-row`; for failures use `notice`.",
	props = {
		["icon"] = "icon name",
		["title"] = "what is missing",
		["message"] = "one line on why or what to do",
		["action"] = "button label",
		["msg"] = "sent when the button is clicked",
	},
	example = { icon = "inbox", title = "No worktrees", message = "Create one to work on two branches at once.", action = "New worktree", msg = "new" },

	build = function(props: Props)
		return ui.column {
			align = "center",
			gap = "small",
			style = { padding = 24 },
			props.icon and ui.icon { name = props.icon :: any, size = "title", tone = "muted" },
			ui.text { props.title, weight = "medium" },
			props.message and ui.text { props.message, size = "small", tone = "muted" },
			props.action and ui.button { props.action, small = true, variant = "outline", msg = props.msg },
		}
	end,
}
