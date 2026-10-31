-- Agents grouped by state: click a heading to fold its group, + adds an
-- agent to it, and clicking a blocked agent moves it to Working.
local group_header = use "group-header"
local status_dot = use "status-dot"

local GROUPS = {
	{ id = "blocked", title = "Needs you" },
	{ id = "working", title = "Working" },
	{ id = "done", title = "Done" },
}

return mod {
	init = function(previous)
		return previous
			or {
				folded = { done = true },
				next = 1,
				agents = {
					{ name = "fix-rounding", agent = "claude", status = "blocked" },
					{ name = "harbor-docs", agent = "pi", status = "blocked" },
					{ name = "feat-multi-currency", agent = "codex", status = "working" },
					{ name = "pixel-forge", agent = "claude", status = "done" },
				},
			}
	end,

	update = function(state, msg)
		if typeof(msg) ~= "table" then
			return
		end
		if msg.fold then
			state.folded[msg.fold] = not state.folded[msg.fold]
		elseif msg.add then
			table.insert(state.agents, { name = `task-{state.next}`, agent = "claude", status = msg.add })
			state.next += 1
			state.folded[msg.add] = false
		elseif msg.unblock then
			local agent = state.agents[msg.unblock]
			if agent and agent.status == "blocked" then
				agent.status = "working"
			end
		end
	end,

	view = function(state)
		local children: { any } = {
			group_header { title = "studio", icon = "server", status = "idle", detail = "online", count = #state.agents, toggle = false, band = true },
		}
		for _, group in GROUPS do
			local rows = {}
			for index, agent in state.agents do
				if agent.status == group.id then
					rows[#rows + 1] = ui.row {
						align = "center",
						style = { padding = { 5, 12 } },
						msg = if agent.status == "blocked" then { unblock = index } else nil,
						ui.row { align = "center", status_dot { status = agent.status, size = "small" } },
						ui.text { agent.name, truncate = true },
						ui.spacer(),
						ui.text { agent.agent, size = "small", tone = "muted" },
					}
				end
			end
			children[#children + 1] = group_header {
				title = group.title,
				status = group.id,
				count = #rows,
				collapsed = state.folded[group.id],
				toggle = { fold = group.id },
				add = { add = group.id },
				add_tooltip = `New agent in {group.title}`,
			}
			if not state.folded[group.id] then
				for _, row in rows do
					children[#children + 1] = row
				end
			end
		end
		return ui.column { gap = "none", width = 360, table.unpack(children) }
	end,
}
