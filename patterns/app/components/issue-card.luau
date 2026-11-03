-- A readable task card: title, blockers and labels, then a quiet metadata row.
local pill = use "pill"
local priority_icon = use "priority-icon"
local avatar = use "avatar"
local relative_time = use "relative-time"

return component {
	title = "Issue card",
	category = "Workflows",
	icon = "square-check",
	description = "The right context, in the right order.",
	when = "Lead with a wrapping title. Put blockers before labels, and source, age, priority and owner on a consistent baseline. Use in a board or an inbox.",
	props = {
		["title"] = "Issue title; wraps rather than truncates.",
		["identifier"] = "Issue identifier, such as APP-150.",
		["blocked_by"] = "Optional blocker identifier.",
		["labels"] = "List of label strings.",
		["source_icon"] = "Bundled source icon, default circle-dot.",
		["seconds"] = "Age in seconds, optional.",
		["priority"] = "none, low, medium, high or urgent.",
		["assignee"] = "Optional person's name.",
		["compact"] = "Tighter padding.",
		["msg"] = "Message on opening the card.",
		["context"] = "Message on right-click.",
		["carry"] = "Optional drag payload for a kanban column.",
		["to"] = "Message recipient, optional.",
	},
	styles = {
		[".issue-card"] = { background = "card", border = "all", border_color = "border", radius = 10, padding = 20, gap = 16, shadow = "0 2px 4px #0000000a" },
		[".issue-card:compact"] = { padding = 12, gap = 10 },
		[".issue-card-title"] = { size = "large", weight = "medium" },
		[".issue-card-foot"] = { color = "muted-foreground", gap = 10 },
	},
	example = { title = "Dark mode for the settings page", identifier = "APP-150", blocked_by = "APP-142", labels = { "feature" }, seconds = 86400, priority = "high", assignee = "Jamie" },
	build = function(props: any)
		local labels = {}
		if props.blocked_by then table.insert(labels, pill { label = `Blocked by {props.blocked_by}`, tone = "danger" }) end
		for _, label in props.labels or {} do table.insert(labels, pill { label = label }) end
		return ui.column {
			class = "issue-card", state = if props.compact then "compact" else nil,
			msg = props.msg, context = props.context, carry = props.carry, to = props.to,
			ui.text { props.title, class = "issue-card-title" },
			#labels > 0 and ui.row { gap = "small", wrap = true, table.unpack(labels) },
			ui.row { class = "issue-card-foot", align = "center",
				ui.icon { name = props.source_icon or "circle-dot" }, ui.text { props.identifier or "", size = "small" }, ui.spacer(),
				props.seconds and relative_time { seconds = props.seconds, compact = true },
				priority_icon { priority = props.priority or "none" },
				props.assignee and avatar { name = props.assignee, size = "small", shape = "circle" },
			},
		}
	end,
}
