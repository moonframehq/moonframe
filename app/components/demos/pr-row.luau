-- A pull request list: click a row to select it; right-click marks it as
-- needing attention.
local pr_row = use "pr-row"

local PRS: { any } = {
	{ number = 850, title = "Show GitLab merge requests in Pull requests", branch = "feature/gitlab-mrs", author = "fanfei", seconds = 4 * 86400, checks = "passed", added = 412, removed = 87, comments = 3, labels = { "ui" }, tone = "ready" },
	{ number = 847, title = "Reconnect SSH hosts with backoff", branch = "fix/reconnect", author = "penso", seconds = 6 * 3600, checks = "pending", added = 96, removed = 21, tone = "waiting" },
	{ number = 842, title = "Settings window", branch = "feature/settings", author = "alice", seconds = 2 * 86400, checks = "failed", added = 1204, removed = 310, comments = 12, tone = "attention" },
	{ number = 839, title = "Cache workspace scrollback", branch = "wip/scrollback", author = "claude", seconds = 9 * 86400, draft = true, added = 58, removed = 4, tone = "draft" },
	{ number = 831, title = "Toasts stack upward", branch = "fix/toasts", author = "penso", seconds = 14 * 86400, checks = "passed", added = 22, removed = 9, tone = "merged" },
}

return mod {
	init = function(previous)
		return previous or { selected = 850, flagged = {} }
	end,

	update = function(state, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.selected = msg.select
		elseif msg.flag then
			state.flagged[msg.flag] = not state.flagged[msg.flag]
		end
	end,

	view = function(state)
		local rows = {}
		for index, pr in PRS do
			rows[index] = pr_row {
				number = pr.number,
				title = pr.title,
				branch = pr.branch,
				target = "main",
				author = pr.author,
				seconds = pr.seconds,
				checks = pr.checks,
				added = pr.added,
				removed = pr.removed,
				comments = pr.comments,
				labels = pr.labels,
				draft = pr.draft,
				tone = if state.flagged[pr.number] then "attention" else pr.tone,
				selected = state.selected == pr.number,
				context = { flag = pr.number },
			}
		end
		return ui.column { gap = "none", table.unpack(rows) }
	end,
}
