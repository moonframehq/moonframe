-- Two sidebar groups: click a heading to fold it, a row to select it, and
-- "a-z" to sort the agents by name.
local section = use "sidebar-section"

return mod {
	init = function(previous)
		return previous
			or {
				selected = "ledger-api",
				folded = {} :: { [string]: boolean },
				sorted = false,
				agents = {
					{ name = "pixel-forge", agent = "codex" },
					{ name = "ledger-api", agent = "claude" },
					{ name = "harbor-docs", agent = "pi" },
				},
			}
	end,

	update = function(state, msg)
		if msg == "sort" then
			state.sorted = not state.sorted
		elseif typeof(msg) == "table" then
			if msg.toggle then
				state.folded[msg.toggle] = not state.folded[msg.toggle]
			elseif msg.open then
				state.selected = msg.open
			end
		end
	end,

	view = function(state)
		local agents = table.clone(state.agents)
		if state.sorted then
			table.sort(agents, function(a, b)
				return a.name < b.name
			end)
		end
		local rows = {}
		for i, a in agents do
			rows[i] = ui.item { a.name, detail = a.agent, selected = a.name == state.selected, msg = { open = a.name } }
		end
		return ui.column {
			width = 260,
			gap = "none",
			section {
				title = "Agents",
				id = "agents",
				count = #rows,
				collapsed = state.folded.agents,
				actions = { ui.button { if state.sorted then "a-z ✓" else "a-z", small = true, variant = "ghost", msg = "sort" } },
				table.unpack(rows),
			},
			section {
				title = "Machines",
				id = "machines",
				count = 2,
				collapsed = state.folded.machines,
				ui.item { "studio", detail = "online", selected = state.selected == "studio", msg = { open = "studio" } },
				ui.item { "laptop", detail = "this computer", selected = state.selected == "laptop", msg = { open = "laptop" } },
			},
			section { title = "Pinned", fixed = true, ui.item { "release checklist", detail = "note" } },
		}
	end,
}
