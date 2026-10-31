-- The connection to a host dropped: a card centred over the dimmed window
-- with a spinner, "Reconnecting to <host>…", the reason, the next attempt
-- and a Reconnect now button. The window behind stays visible but blocked.

type Props = {
	open: boolean?,
	host: string?,
	title: string?,
	reason: string?,
	-- "Next attempt in 4s", "Attempt 3 of 10".
	status: string?,
	retry: any,
	retry_label: string?,
	-- A second button, e.g. { label = "Work offline", msg = "offline" }.
	secondary: any,
	dismiss: any,
	icon: string?,
	to: string?,
}

return component {
	title = "Reconnecting",
	category = "Feedback",
	icon = "wifi-off",
	description = "A card over the dimmed window while a host reconnects: spinner, host, reason, next attempt and Reconnect now.",
	when = "A whole window or pane depends on a connection that dropped (the daemon, an SSH host, a cloud machine) and must wait for it. For a failed load inside one pane use `error-state`; for a brief note that something reconnected use `toast`; for plain loading use `loading`.",
	props = {
		["open"] = "show it",
		["host"] = "the host's name, in the title",
		["title"] = "instead of \"Reconnecting to <host>…\"",
		["reason"] = "why it dropped (muted)",
		["status"] = "small line such as \"Next attempt in 4s\"",
		["retry"] = "msg for Reconnect now (hidden when unset)",
		["retry_label"] = "default \"Reconnect now\"",
		["secondary"] = "a second button { label, msg }",
		["dismiss"] = "msg for Escape and backdrop clicks; unset keeps it up until the mod closes it",
		["icon"] = "an icon instead of the spinner",
		["to"] = "mod the buttons send to, if not the drawing one",
	},
	styles = {
		[".reconnecting"] = {
			background = "background",
			border = "all",
			border_color = "border",
			radius = 12,
			padding = { 20, 22 },
		},
		[".reconnecting-body"] = { gap = 8 },
		[".reconnecting-spinner"] = { color = "accent", size = "title" },
		[".reconnecting-title"] = { weight = "medium", size = "large" },
		[".reconnecting-reason"] = { size = "small", color = "muted-foreground" },
		[".reconnecting-status"] = { size = "small", color = "muted-foreground", font = "mono" },
		[".reconnecting-actions"] = { gap = 8, margin = { top = 8 } },
	},
	example = {
		open = true,
		host = "devbox",
		reason = "The SSH connection closed: Connection reset by peer.",
		status = "Next attempt in 4s · attempt 3",
		retry = "reconnect",
		secondary = { label = "Work offline", msg = "offline" },
	},

	build = function(props: Props)
		local title = props.title or ("Reconnecting to " .. (props.host or "the daemon") .. "…")
		local mark = if props.icon
			then ui.icon { name = props.icon :: any, class = "reconnecting-spinner" }
			else ui.icon { name = "loader-circle", spin = true, class = "reconnecting-spinner" }

		local actions: any = false
		if props.retry ~= nil or props.secondary then
			actions = ui.row {
				class = "reconnecting-actions",
				align = "center",
				props.secondary ~= nil and ui.button {
					props.secondary.label,
					variant = "ghost",
					msg = props.secondary.msg,
					to = props.to,
				},
				props.retry ~= nil and ui.button {
					props.retry_label or "Reconnect now",
					variant = "primary",
					msg = props.retry,
					to = props.to,
				},
			}
		end

		return ui.overlay {
			open = props.open,
			dismiss = props.dismiss,
			place = "center",
			backdrop = true,
			width = 380,
			to = props.to,
			class = "reconnecting",
			content = ui.column {
				class = "reconnecting-body",
				align = "center",
				mark,
				ui.text { title, class = "reconnecting-title" },
				props.reason ~= nil and ui.text { props.reason, class = "reconnecting-reason" },
				props.status ~= nil and ui.text { props.status, class = "reconnecting-status" },
				actions,
			},
		}
	end,
}
