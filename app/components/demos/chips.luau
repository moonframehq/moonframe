-- Pull request filters: click chips to turn them on and off; the list
-- below shows what matches.
local chips = use "chips"

local PRS: { any } = {
	{ title = "Fix tab close focus", open = true, mine = true, agent = true },
	{ title = "Add sparkline to status bar", open = true, agent = true, failing = true },
	{ title = "Draft: provider sandbox", open = true, draft = true, mine = true },
	{ title = "Bump gpui", open = false },
	{ title = "Theme picker cards", open = true },
}

return mod {
	init = function(previous)
		return previous or { on = { open = true } }
	end,

	update = function(state, msg)
		if msg.toggle then
			state.on[msg.toggle] = not state.on[msg.toggle] or nil
		end
	end,

	view = function(state)
		local selected, shown = {}, {}
		for id in state.on do
			table.insert(selected, id)
		end
		local counts = { open = 0, mine = 0, agent = 0, draft = 0, failing = 0 }
		for _, pr in PRS do
			local keep = true
			for id in counts do
				if pr[id] then
					counts[id] += 1
				end
				if state.on[id] and not pr[id] then
					keep = false
				end
			end
			if keep then
				table.insert(shown, ui.text { pr.title, size = "small" })
			end
		end
		return ui.column {
			gap = "normal",
			chips {
				selected = selected,
				chips = {
					{ id = "open", label = "Open", count = counts.open },
					{ id = "mine", label = "Mine", icon = "user", count = counts.mine },
					{ id = "agent", label = "Has agent", icon = "agent-claude", count = counts.agent },
					{ id = "draft", label = "Draft", icon = "git-pull-request-draft", count = counts.draft },
					{ id = "failing", label = "CI failing", icon = "circle-x", count = counts.failing },
				},
			},
			ui.column {
				gap = "small",
				if #shown == 0 then ui.text { "No pull requests match", tone = "muted", size = "small" } else false,
				table.unpack(shown),
			},
		}
	end,
}
