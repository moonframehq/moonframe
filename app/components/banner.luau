-- A full-width coloured strip with a message: the worktree branch strip at
-- the bottom of the window, a "provider failed" notice, an update ready.
-- Soft (tinted) by default; `solid` fills it with the tone.

type Action = {
	label: string,
	msg: any,
	to: string?,
}

type Props = {
	text: string,
	detail: string?,
	icon: string?,
	tone: string?,
	solid: boolean?,
	mono: boolean?,
	action: any,
	dismiss: any,
	to: string?,
}

local TONES = { accent = true, warning = true, success = true, danger = true, muted = true }

return component {
	title = "Banner",
	category = "Feedback",
	icon = "info",
	description = "A full-width coloured strip with an icon, text, an optional action and a close button; soft or solid, in a tone.",
	when = "A message across the whole window or pane that stays until dealt with: the current worktree's branch, a failed provider, an update ready. For a transient message use a toast (`ui.overlay` at `bottom-end`); for a status inside a row use `status-dot` or `ui.badge`; for an empty pane use `empty-state`.",
	props = {
		["text"] = "the message",
		["detail"] = "muted text after it",
		["icon"] = "icon before the text",
		["tone"] = "accent (default), warning, success, danger or muted",
		["solid"] = "fill with the tone (a yellow branch strip) instead of a tint",
		["mono"] = "set the text in the monospace font (branch names, paths)",
		["action"] = "{ label, msg, to? } for a button on the right, or an element",
		["dismiss"] = "msg for a close button on the far right (omit for none)",
		["to"] = "mod to send the action and dismiss msgs to, if not the drawing one",
	},
	styles = {
		[".banner"] = { padding = { 6, 14 }, gap = 8 },
		[".banner-accent"] = { background = "accent/12", color = "accent" },
		[".banner-warning"] = { background = "warning/15", color = "warning" },
		[".banner-success"] = { background = "success/12", color = "success" },
		[".banner-danger"] = { background = "danger/12", color = "danger" },
		[".banner-muted"] = { background = "secondary", color = "muted-foreground" },
		[".banner-solid-accent"] = { background = "accent", color = "accent-foreground" },
		[".banner-solid-warning"] = { background = "warning", color = "#1c1917" },
		[".banner-solid-success"] = { background = "success", color = "#052e16" },
		[".banner-solid-danger"] = { background = "danger", color = "#ffffff" },
		[".banner-solid-muted"] = { background = "muted", color = "foreground" },
		[".banner-text"] = { weight = "medium" },
		[".banner-mono"] = { font = "mono", weight = "normal" },
		[".banner-detail"] = { opacity = 0.75, size = "small" },
		[".banner-action"] = { radius = 6, padding = { 2, 8 }, border = "all", border_color = "foreground/20", size = "small", weight = "medium" },
		[".banner-action:hover"] = { background = "foreground/10" },
		[".banner-close"] = { radius = 5, padding = 3 },
		[".banner-close:hover"] = { background = "foreground/10" },
	},
	usage = [[local banner = use "banner"

banner { text = app.git and app.git.branch or "main", icon = "git-branch", tone = "warning", solid = true, mono = true }
banner { text = "The git provider stopped", detail = "exit status 1", icon = "triangle-alert", tone = "danger",
	action = { label = "Restart", msg = "restart-git" }, dismiss = "hide-banner" }]],
	example = {
		text = "feat/gitlab-merge-requests-887",
		detail = "worktree of ledger-api",
		icon = "git-branch",
		tone = "warning",
		solid = true,
		mono = true,
		action = { label = "Open review", msg = "review" },
		dismiss = "hide",
	},

	build = function(props: Props)
		local tone = if props.tone and TONES[props.tone] then props.tone else "accent"
		local classes = { "banner", `banner-{tone}` }
		if props.solid then
			classes[#classes + 1] = `banner-solid-{tone}`
		end
		local action = props.action
		if typeof(action) == "table" and action.type == nil then
			action = ui.row {
				class = "banner-action",
				align = "center",
				msg = action.msg,
				to = action.to or props.to,
				ui.text { action.label },
			}
		end
		return ui.row {
			class = classes,
			align = "center",
			shrink = false,
			props.icon ~= nil and ui.icon { name = props.icon :: any, class = "banner-icon" },
			ui.text { props.text, class = if props.mono then "banner-text banner-mono" else "banner-text", truncate = true },
			props.detail ~= nil and ui.text { props.detail, class = "banner-detail", truncate = true },
			ui.spacer(),
			action or false,
			props.dismiss ~= nil and ui.row {
				class = "banner-close",
				msg = props.dismiss,
				to = props.to,
				tooltip = "Dismiss",
				ui.icon { name = "close", size = "small" },
			},
		}
	end,
}
