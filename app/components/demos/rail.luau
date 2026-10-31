-- The sidebar folded to a rail: click a tile to switch workspace, the top
-- button unfolds the names beside it, the gear opens settings.
local rail = use "rail"

local SPACES: { any } = {
	{ id = "1", label = "1", status = "idle", name = "ledger-api", branch = "main" },
	{ id = "2", label = "2", status = "working", name = "feat/multi-currency", branch = "feat/multi-currency" },
	{ id = "3", label = "3", status = "blocked", name = "fix/rounding", branch = "needs you" },
	{ id = "4", label = "PF", status = "done", name = "pixel-forge", branch = "main" },
	{ id = "docs", icon = "book-open", name = "harbor-docs", branch = "docs" },
}

local function button(icon: string, msg: string, tooltip: string)
	return ui.row { class = "rail-item", width = 36, justify = "center", msg = msg, tooltip = tooltip, ui.icon { name = icon :: any } }
end

return mod {
	init = function(previous)
		return previous or { selected = "2", expanded = false }
	end,

	update = function(state, msg)
		if msg == "expand" then
			state.expanded = not state.expanded
		elseif msg == "settings" then
			state.selected = "settings"
		elseif typeof(msg) == "table" and msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state)
		local items = {}
		local current = nil
		for i, s in SPACES do
			items[i] = { id = s.id, label = s.label, icon = s.icon, status = s.status, tooltip = `{s.name} · {s.branch}` }
			if s.id == state.selected then
				current = s
			end
		end
		local names = nil
		if state.expanded then
			local rows = {}
			for i, s in SPACES do
				rows[i] = ui.item { s.name, detail = s.branch, selected = s.id == state.selected, msg = { select = s.id } }
			end
			names = ui.column { width = 200, gap = "none", padding = "small", surface = "sidebar", table.unpack(rows) }
		end
		return ui.row {
			height = 360,
			gap = "none",
			rail {
				selected = state.selected,
				header = { button(if state.expanded then "panel-left-close" else "panel-left-open", "expand", "Expand sidebar") },
				items = items :: { any },
				footer = { button("settings", "settings", "Settings") },
			},
			names or false,
			ui.column {
				grow = true,
				padding = "large",
				gap = "small",
				ui.text { if current then current.name else "Settings", size = "large", weight = "bold" },
				ui.text { if current then current.branch else "Preferences for every window.", tone = "muted" },
			},
		}
	end,
}
