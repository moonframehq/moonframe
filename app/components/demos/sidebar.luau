-- A workspace sidebar: click a row to open it, click a heading to fold its
-- section, "new" adds a space, Settings clears its update dot.
local sidebar = use "sidebar"
local status_dot = use "status-dot"

local function row(space, selected: boolean)
	return ui.row {
		class = "sidebar-example-row",
		selected = selected,
		align = "center",
		msg = { open = space.id },
		status_dot { status = space.status, size = "small" },
		ui.column {
			gap = "none",
			grow = true,
			ui.text { space.name, weight = "medium", truncate = true },
			ui.text { space.branch, size = "small", tone = "muted", truncate = true },
		},
	}
end

return mod {
	init = function(previous)
		return previous
			or {
				selected = "multi",
				folded = { archived = true } :: { [string]: boolean },
				update_ready = true,
				spaces = {
					{ id = "ledger", name = "ledger-api", branch = "main", status = "idle" },
					{ id = "multi", name = "feat/multi-currency", branch = "feat/multi-currency", status = "working" },
					{ id = "rounding", name = "fix/rounding", branch = "fix/rounding", status = "blocked" },
				},
				agents = {
					{ id = "a1", name = "ledger-api", branch = "claude · editor", status = "working" },
					{ id = "a2", name = "harbor-docs", branch = "pi · docs", status = "done" },
				},
			}
	end,

	update = function(state, msg)
		if msg == "new-space" then
			local n = #state.spaces + 1
			local id = `space-{n}`
			table.insert(state.spaces, { id = id, name = `scratch-{n}`, branch = "main", status = "idle" })
			state.selected = id
		elseif msg == "settings" then
			state.update_ready = false
			state.selected = "settings"
		elseif msg == "add-machine" then
			state.selected = "add-machine"
		elseif typeof(msg) == "table" then
			if msg.toggle then
				state.folded[msg.toggle] = not state.folded[msg.toggle]
			elseif msg.open then
				state.selected = msg.open
			end
		end
	end,

	view = function(state)
		local spaces, agents = {}, {}
		for i, space in state.spaces do
			spaces[i] = row(space, space.id == state.selected)
		end
		for i, agent in state.agents do
			agents[i] = row(agent, agent.id == state.selected)
		end
		return sidebar {
			width = 280,
			height = 420,
			header = ui.row {
				align = "center",
				ui.text { "Moonframe", weight = "bold" },
				ui.spacer(),
				ui.text { state.selected, size = "small", tone = "muted" },
			},
			sections = {
				{
					title = "Spaces",
					id = "spaces",
					collapsed = state.folded.spaces,
					actions = { ui.button { "new", small = true, variant = "ghost", msg = "new-space" } },
					table.unpack(spaces),
				},
				{ title = "Agents", id = "agents", collapsed = state.folded.agents, table.unpack(agents) },
				{ title = "Archived", id = "archived", count = 12, collapsed = state.folded.archived, ui.item { "old-billing", detail = "merged" } },
			} :: { any },
			footer = {
				{ icon = "settings", label = "Settings", msg = "settings", dot = state.update_ready },
				{ icon = "plus", label = "Add machine", msg = "add-machine", muted = true },
			} :: { any },
		}
	end,
}
