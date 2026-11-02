-- Something failed to load: an icon, a message, optional details in mono
-- (the error text, a command) and a Retry button. Inline in a pane or list,
-- or centred as a block.

type Props = {
	title: string?,
	message: string?,
	details: string?,
	icon: string?,
	retry: any,
	retry_label: string?,
	-- A second, quieter button: { label, msg }.
	secondary: any,
	block: boolean?,
	-- A retry in flight: the button spins and is disabled.
	retrying: boolean?,
	to: string?,
}

return component {
	title = "Error state",
	category = "Feedback",
	icon = "circle-alert",
	description = "An inline or centred failure: icon, message, mono details and a Retry button.",
	when = "In place of content that failed to load (a list, a pane, a provider's data). For an in-page warning that isn't a failed load use `notice`; for a lost daemon or SSH connection use `reconnecting`; for nothing to show use `empty-state`; for a transient failure use `toast`.",
	props = {
		["title"] = "what failed, e.g. \"Couldn't load pull requests\"",
		["message"] = "why, or what to do",
		["details"] = "the raw error, shown in mono in a box",
		["icon"] = "icon; default `circle-alert`",
		["retry"] = "msg for the Retry button (hidden when unset)",
		["retry_label"] = "default \"Retry\"",
		["secondary"] = "a second button { label, msg }, e.g. Open logs",
		["retrying"] = "show the retry as running",
		["block"] = "centre it in the space it gets",
		["to"] = "mod the buttons send to, if not the drawing one",
	},
	styles = {
		[".error-state"] = { gap = 10, padding = 12 },
		[".error-state-block"] = { padding = 24 },
		[".error-state-icon"] = { color = "danger" },
		[".error-state-block-icon"] = { size = "title" },
		[".error-state-text"] = { gap = 2 },
		[".error-state-title"] = { weight = "medium" },
		[".error-state-message"] = { size = "small", color = "muted-foreground" },
		[".error-state-details"] = {
			background = "danger/6",
			border = "all",
			border_color = "danger/25",
			radius = 8,
			padding = { 8, 10 },
			font = "mono",
			size = "small",
			color = "foreground",
		},
		[".error-state-actions"] = { gap = 8 },
	},
	example = {
		title = "Couldn't load pull requests",
		message = "GitHub refused the request. Check the token in Settings › Integrations.",
		details = "HTTP 401 Unauthorized: Bad credentials (GET /repos/moonframehq/moonframe/pulls)",
		retry = "retry",
		secondary = { label = "Open settings", msg = "open-settings" },
	},

	build = function(props: Props)
		local block = props.block == true
		local actions: any = false
		if props.retry ~= nil or props.secondary then
			actions = ui.row {
				class = "error-state-actions",
				align = "center",
				props.retry ~= nil and ui.button {
					props.retrying and "Retrying…" or (props.retry_label or "Retry"),
					small = true,
					variant = "outline",
					disabled = props.retrying,
					msg = props.retry,
					to = props.to,
				},
				props.secondary ~= nil and ui.button {
					props.secondary.label,
					small = true,
					variant = "ghost",
					msg = props.secondary.msg,
					to = props.to,
				},
			}
		end

		local text = ui.column {
			class = "error-state-text",
			align = if block then "center" else nil,
			props.title ~= nil and ui.text { props.title, class = "error-state-title" },
			props.message ~= nil and ui.text { props.message, class = "error-state-message" },
		}
		local details: any = props.details ~= nil and ui.column { class = "error-state-details", ui.text { props.details } }
		local icon = ui.icon {
			name = (props.icon or "circle-alert") :: any,
			class = if block then "error-state-icon error-state-block-icon" else "error-state-icon",
		}

		if block then
			return ui.column {
				class = "error-state error-state-block",
				grow = true,
				align = "center",
				justify = "center",
				icon,
				text,
				details,
				actions,
			}
		end
		return ui.row {
			class = "error-state",
			align = "start",
			icon,
			ui.column { grow = true, gap = "small", text, details, actions },
		}
	end,
}
