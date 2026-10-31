-- A pull request's (or merge request's, or issue's) number as a tinted
-- pill: "#850" or "!3", coloured and iconed by where it stands (draft,
-- waiting, ready, attention, merged, closed, held), with an optional dot
-- for its checks.

type Props = {
	number: number | string,
	kind: string?,
	tone: string?,
	icon: string?,
	checks: string?,
	label: string?,
	tooltip: string?,
	msg: any,
	to: string?,
}

-- Each tone's icon; an issue uses `circle-dot` (open) or `circle-check`.
local ICONS = {
	draft = "git-pull-request-draft",
	waiting = "git-pull-request",
	ready = "git-pull-request",
	attention = "git-pull-request",
	merged = "git-merge",
	closed = "git-pull-request-closed",
	held = "git-pull-request",
}

local WORDS = {
	draft = "Draft",
	waiting = "Waiting for review",
	ready = "Ready to merge",
	attention = "Needs attention",
	merged = "Merged",
	closed = "Closed",
	held = "On hold",
}

return component {
	title = "PR Badge",
	category = "Git",
	icon = "git-pull-request",
	description = "A PR, MR or issue number (#850, !3) as a pill tinted by state: draft, waiting, ready, attention, merged, closed, held; with a checks dot.",
	when = "A pull or merge request referenced inline: beside a branch in a sidebar, in a title bar, in a tab. For a full list row use `pr-row`; for CI details use `checks`; for an agent's state use `status-dot`.",
	props = {
		["number"] = "the PR, MR or issue number",
		["kind"] = "pr (\"#\", default) | issue (\"#\") | mr (GitLab, \"!\")",
		["tone"] = "draft | waiting | ready | attention | merged | closed | held (default waiting)",
		["icon"] = "an icon name instead of the tone's",
		["checks"] = "passed | failed | pending: a small dot after the number",
		["label"] = "text after the number, e.g. \"Ready\"",
		["tooltip"] = "text under the pointer (default the tone in words)",
		["msg"] = "message sent on a click (e.g. { open = 850 })",
		["to"] = "the mod to send `msg` to",
	},
	styles = {
		[".pr-badge"] = {
			radius = 999,
			padding = { 1, 7 },
			gap = 4,
			background = "muted-foreground/15",
			color = "muted-foreground",
		},
		[".pr-badge-number"] = { size = "small", weight = "medium", font = "mono" },
		[".pr-badge-label"] = { size = "small", weight = "medium" },
		[".pr-badge-draft"] = { background = "muted-foreground/15", color = "muted-foreground" },
		[".pr-badge-waiting"] = { background = "warning/15", color = "warning" },
		[".pr-badge-ready"] = { background = "success/15", color = "success" },
		[".pr-badge-attention"] = { background = "danger/15", color = "danger" },
		[".pr-badge-merged"] = { background = "#c678dd26", color = "#c678dd" },
		[".pr-badge-closed"] = { background = "muted-foreground/12", color = "muted-foreground" },
		[".pr-badge-held"] = { background = "#e3814f26", color = "#e3814f" },
		[".pr-badge-checks"] = { radius = 999 },
		[".pr-badge-checks-passed"] = { background = "success" },
		[".pr-badge-checks-failed"] = { background = "danger" },
		[".pr-badge-checks-pending"] = { background = "warning" },
	},
	example = { number = 850, tone = "ready", checks = "passed", label = "Ready" },

	build = function(props: Props)
		local tone = props.tone or "waiting"
		if ICONS[tone] == nil then
			tone = "waiting"
		end
		local prefix = if props.kind == "mr" then "!" else "#"
		local icon = props.icon or ICONS[tone]
		if props.kind == "issue" and not props.icon then
			icon = if tone == "merged" or tone == "closed" then "circle-check" else "circle-dot"
		end
		return ui.row {
			class = { "pr-badge", `pr-badge-{tone}` },
			hug = true,
			shrink = false,
			align = "center",
			tooltip = props.tooltip or WORDS[tone],
			msg = props.msg,
			to = props.to,
			ui.icon { name = icon :: any, size = "small" },
			ui.text { `{prefix}{props.number}`, class = "pr-badge-number" },
			props.label and ui.text { props.label, class = "pr-badge-label" },
			props.checks and ui.row {
				class = { "pr-badge-checks", `pr-badge-checks-{props.checks}` },
				width = 6,
				height = 6,
			},
		}
	end,
}
