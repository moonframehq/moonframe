-- A view switcher: click a tile to switch views; opening a view clears its
-- count bubble, and "New issue" adds one to Issues.
local activity_bar = use "activity-bar"

local VIEWS = {
	{ id = "terminals", icon = "square-terminal", tooltip = "Terminals" },
	{ id = "pulls", icon = "git-pull-request", tooltip = "Pull requests" },
	{ id = "issues", icon = "circle-dot", tooltip = "Issues" },
	{ id = "agents", icon = "bot", tooltip = "Agents" },
	{ id = "usage", icon = "chart-column", tooltip = "Usage" },
}

return mod {
	init = function(previous)
		return previous or { selected = "terminals", counts = { pulls = 3, issues = 1, agents = 2 } }
	end,

	update = function(state, msg)
		if msg == "new-issue" then
			state.counts.issues = (state.counts.issues or 0) + 1
		elseif typeof(msg) == "table" and msg.activity then
			state.selected = msg.activity
			state.counts[msg.activity] = nil
		end
	end,

	view = function(state)
		local items = {}
		local title = "Settings"
		for i, view in VIEWS do
			items[i] = { id = view.id, icon = view.icon, tooltip = view.tooltip, count = state.counts[view.id] }
			if view.id == state.selected then
				title = view.tooltip
			end
		end
		return ui.row {
			height = 360,
			gap = "none",
			activity_bar {
				selected = state.selected,
				items = items,
				bottom = { { id = "settings", icon = "settings", tooltip = "Settings" } },
			},
			ui.column {
				grow = true,
				padding = "large",
				gap = "normal",
				ui.text { title, size = "large", weight = "bold" },
				ui.text { `Showing the {string.lower(title)} view.`, tone = "muted" },
				ui.row { ui.button { "New issue", small = true, variant = "outline", icon = "plus", msg = "new-issue" } },
			},
		}
	end,
}
