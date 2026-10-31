-- One pull request, merge request or issue in a list: its icon coloured
-- by state and its number, the title (with a draft tag) over the branch in
-- mono, then the author, a checks dot, the diff size and the age.
-- Clicking it sends `{ select = number }`.

type Props = {
	number: number | string,
	kind: string?,
	title: string,
	tone: string?,
	draft: boolean?,
	branch: string?,
	target: string?,
	author: string?,
	age: string?,
	seconds: number?,
	checks: string?,
	added: number?,
	removed: number?,
	comments: number?,
	labels: { string }?,
	selected: boolean?,
	event: string?,
	to: string?,
	msg: any,
	context: any,
}

local diff_counts = use "diff-counts"

local ICONS = {
	draft = "git-pull-request-draft",
	merged = "git-merge",
	closed = "git-pull-request-closed",
}

local CHECKS = {
	passed = "All checks passed",
	failed = "Checks failed",
	pending = "Checks running",
}

local function compact(seconds: number): string
	if seconds < 3600 then
		return `{math.max(1, math.floor(seconds / 60))}m`
	elseif seconds < 86400 then
		return `{math.floor(seconds / 3600)}h`
	elseif seconds < 604800 then
		return `{math.floor(seconds / 86400)}d`
	end
	return `{math.floor(seconds / 604800)}w`
end

return component {
	title = "PR Row",
	category = "Git",
	icon = "git-pull-request",
	description = "A pull request, MR or issue list row: state-coloured icon and number, title with draft tag, mono branch, author, checks dot, size and age; selectable.",
	when = "Lists of pull requests, merge requests or issues, as in an orchestrator or review queue; stack rows in a scrolling column, under `section` headings per group. For a PR referenced inline use `pr-badge`; for a generic data grid use `table`; for its CI use `checks`.",
	props = {
		["number"] = "the PR, MR or issue number",
		["kind"] = "pr (\"#\", default) | mr (\"!\") | issue",
		["title"] = "the title",
		["tone"] = "draft | waiting | ready | attention | merged | closed | held: the icon's colour (default waiting)",
		["draft"] = "a draft tag after the title (and the draft icon)",
		["branch"] = "source branch, mono on the second line",
		["target"] = "target branch, shown as branch → target",
		["author"] = "author's login",
		["age"] = "age text, e.g. \"2d\"",
		["seconds"] = "age in seconds instead of `age` (shown compact)",
		["checks"] = "passed | failed | pending: a coloured dot",
		["added"] = "lines added (with `removed`, shows diff counts)",
		["removed"] = "lines removed",
		["comments"] = "comment count",
		["labels"] = "label names, as small tags after the title",
		["selected"] = "highlight it",
		["event"] = "message key instead of \"select\": sends { [event] = number }",
		["msg"] = "message sent instead",
		["to"] = "the mod to send it to",
		["context"] = "sent on a right-click",
	},
	styles = {
		[".pr-row"] = { gap = 12, padding = { 6, 12 }, border = "bottom", border_color = "border/60" },
		[".pr-row:hover"] = { background = "list-hover" },
		[".pr-row:selected"] = { background = "list-active" },
		[".pr-row-ref"] = { gap = 6, color = "muted-foreground" },
		[".pr-row-number"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".pr-row-icon"] = { size = "small", color = "muted-foreground" },
		[".pr-row-icon-waiting"] = { color = "success" },
		[".pr-row-icon-ready"] = { color = "success" },
		[".pr-row-icon-attention"] = { color = "danger" },
		[".pr-row-icon-merged"] = { color = "#c678dd" },
		[".pr-row-icon-closed"] = { color = "danger" },
		[".pr-row-icon-held"] = { color = "#e3814f" },
		[".pr-row-icon-draft"] = { color = "muted-foreground" },
		[".pr-row-title"] = { weight = "medium" },
		[".pr-row-draft"] = { radius = 4, padding = { 0, 6 }, background = "accent/12", color = "accent" },
		[".pr-row-draft-text"] = { size = "small" },
		[".pr-row-label"] = { radius = 4, padding = { 0, 6 }, border = "all", border_color = "border" },
		[".pr-row-label-text"] = { size = "small", color = "muted-foreground" },
		[".pr-row-branch"] = { gap = 4, color = "muted-foreground" },
		[".pr-row-branch-text"] = { font = "mono", size = "small" },
		[".pr-row-branch-icon"] = { size = "small" },
		[".pr-row-meta"] = { size = "small", color = "muted-foreground" },
		[".pr-row-checks"] = { radius = 999 },
		[".pr-row-checks-passed"] = { background = "success" },
		[".pr-row-checks-failed"] = { background = "danger" },
		[".pr-row-checks-pending"] = { background = "warning" },
		[".pr-row-age"] = { size = "small", color = "muted-foreground" },
	},
	example = {
		number = 850,
		title = "Show GitLab merge requests in Pull requests",
		branch = "feature/gitlab-mrs",
		target = "main",
		author = "fanfei",
		seconds = 4 * 86400,
		checks = "passed",
		added = 412,
		removed = 87,
		comments = 3,
		labels = { "ui" },
		tone = "ready",
		selected = true,
	},
	usage = [[local pr_row = use "pr-row"

ui.column { gap = "none",
	pr_row { number = 3, kind = "mr", title = "Fix pagination of the merge request listing",
		branch = "fix/pagination", author = "alice", age = "2d", checks = "passed",
		selected = state.selected == 3 },
	pr_row { number = 2, kind = "mr", title = "Draft: cache MR pages", draft = true,
		branch = "wip/cache", author = "penso", age = "3d", checks = "pending" },
}]],

	build = function(props: Props)
		local tone = props.tone or "waiting"
		if props.draft then
			tone = "draft"
		end
		local icon = ICONS[tone] or "git-pull-request"
		if props.kind == "issue" then
			icon = if tone == "closed" or tone == "merged" then "circle-check" else "circle-dot"
		end
		local prefix = if props.kind == "mr" then "!" else "#"

		local tags = {}
		if props.draft then
			table.insert(tags, ui.row { class = "pr-row-draft", ui.text { "draft", class = "pr-row-draft-text" } })
		end
		local labels: { string } = props.labels or {}
		for _, label in labels do
			table.insert(tags, ui.row { class = "pr-row-label", ui.text { label, class = "pr-row-label-text" } })
		end

		local age = props.age
		if age == nil and props.seconds then
			age = compact(props.seconds)
		end

		return ui.row {
			class = "pr-row",
			shrink = false,
			align = "center",
			selected = props.selected,
			msg = props.msg or { [props.event or "select"] = props.number },
			to = props.to,
			context = props.context,
			ui.row {
				class = "pr-row-ref",
				width = 64,
				align = "center",
				ui.icon { name = icon :: any, class = { "pr-row-icon", `pr-row-icon-{tone}` } },
				ui.text { `{prefix}{props.number}`, class = "pr-row-number" },
			},
			ui.column {
				grow = true,
				gap = "none",
				ui.row {
					align = "center",
					gap = "small",
					ui.text { props.title, class = "pr-row-title", truncate = true },
					table.unpack(tags),
				},
				props.branch and ui.row {
					class = "pr-row-branch",
					align = "center",
					ui.icon { name = "git-branch", class = "pr-row-branch-icon" },
					ui.text {
						if props.target then `{props.branch} → {props.target}` else props.branch,
						class = "pr-row-branch-text",
						truncate = true,
					},
				},
			},
			props.author and ui.row { width = 90, ui.text { props.author, class = "pr-row-meta", truncate = true } },
			props.comments and props.comments > 0 and ui.row {
				width = 36,
				gap = "small",
				align = "center",
				ui.icon { name = "message-square", class = "pr-row-meta" },
				ui.text { tostring(props.comments), class = "pr-row-meta" },
			},
			props.checks and ui.row {
				width = 14,
				justify = "center",
				tooltip = CHECKS[props.checks] or props.checks,
				ui.row { class = { "pr-row-checks", `pr-row-checks-{props.checks}` }, width = 8, height = 8 },
			},
			(props.added or props.removed) and ui.row {
				width = 84,
				justify = "end",
				diff_counts { added = props.added, removed = props.removed },
			},
			age and ui.row { width = 32, justify = "end", ui.text { age, class = "pr-row-age" } },
		}
	end,
}
