-- Two disclosures, a card and a flat one: click a header to fold it;
-- "Dispatch agent" changes the first one's summary.
local collapsible = use "collapsible"

return mod {
	init = function(previous)
		return previous or { open = { agent = true, advanced = false }, dispatched = false }
	end,

	update = function(state, msg)
		if msg == "dispatch" then
			state.dispatched = not state.dispatched
		elseif typeof(msg) == "table" and msg.toggle then
			state.open[msg.toggle] = not state.open[msg.toggle]
		end
	end,

	view = function(state)
		return ui.column {
			gap = "normal",
			width = 420,
			collapsible {
				id = "agent",
				title = if state.dispatched then "claude on feat/multi-currency" else "No agent yet",
				icon = if state.dispatched then "agent-claude" else nil,
				summary = if state.dispatched then "working" else "dispatch one",
				open = state.open.agent,
				ui.text {
					if state.dispatched
						then "The agent is working in ~/w/ledger-multi."
						else "Dispatch an agent to work on this merge request in a new worktree.",
					tone = "muted",
					size = "small",
				},
				ui.row {
					ui.button {
						if state.dispatched then "Stop agent" else "Dispatch agent",
						small = true,
						variant = if state.dispatched then "outline" else "primary",
						msg = "dispatch",
					},
				},
			},
			collapsible {
				id = "advanced",
				title = "Advanced options",
				summary = "2",
				flat = true,
				open = state.open.advanced,
				ui.text { "Base branch: main", size = "small" },
				ui.text { "Worktree folder: ~/w", size = "small" },
			},
		}
	end,
}
