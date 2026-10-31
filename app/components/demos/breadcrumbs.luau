-- A merge request's path: ▲ ▼ step through the merge requests, a crumb or
-- ← goes up a level, × closes the detail, and a row opens one again.
local breadcrumbs = use "breadcrumbs"

local MRS = {
	{ id = "!1", title = "Add multi-currency ledger" },
	{ id = "!2", title = "Draft: cache MR pages" },
	{ id = "!3", title = "Fix rounding in totals" },
}

return mod {
	init = function(previous)
		-- level: 1 the repository, 2 the merge request list, 3 one merge request.
		return previous or { level = 3, index = 2 }
	end,

	update = function(state, msg)
		if msg == "back" then
			state.level = math.max(1, state.level - 1)
		elseif msg == "previous" then
			state.index = math.max(1, state.index - 1)
		elseif msg == "next" then
			state.index = math.min(#MRS, state.index + 1)
		elseif msg == "close" then
			state.level = 2
		elseif typeof(msg) == "table" then
			if msg.crumb == "repo" then
				state.level = 1
			elseif msg.crumb == "pulls" then
				state.level = 2
			elseif msg.open then
				state.index = msg.open
				state.level = 3
			elseif msg.level then
				state.level = msg.level
			end
		end
	end,

	view = function(state)
		local crumbs: { any } = { { label = "group/sub/app", icon = "brand-gitlab", id = "repo" } }
		if state.level >= 2 then
			crumbs[2] = { label = "Merge requests", icon = "git-pull-request", id = "pulls" }
		end
		local mr = MRS[state.index]
		if state.level == 3 then
			crumbs[3] = { label = `{mr.id} {mr.title}`, icon = "git-pull-request-draft" }
		end

		local body
		if state.level == 3 then
			body = ui.text { `Showing {mr.id}: {mr.title}`, tone = "muted" }
		elseif state.level == 2 then
			local rows = {}
			for i, m in MRS do
				rows[i] = ui.item { `{m.id} {m.title}`, msg = { open = i } }
			end
			body = ui.column { gap = "none", table.unpack(rows) }
		else
			body = ui.item { "Merge requests", detail = tostring(#MRS), msg = { level = 2 } }
		end

		return ui.column {
			gap = "normal",
			width = 560,
			breadcrumbs {
				back = if state.level > 1 then "back" else nil,
				crumbs = crumbs,
				position = if state.level == 3 then `{state.index} of {#MRS}` else nil,
				up = if state.level == 3 and state.index > 1 then "previous" else nil,
				down = if state.level == 3 and state.index < #MRS then "next" else nil,
				actions = if state.level == 3
					then { ui.row { class = "breadcrumbs-step", msg = "close", tooltip = "Close", ui.icon "close" } }
					else nil,
			},
			ui.divider(),
			body,
		}
	end,
}
