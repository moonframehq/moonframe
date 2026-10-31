-- A window body: the toolbar buttons show and hide the sidebar, the bottom
-- panel and the inspector; drag a divider to resize; click a worktree.
local split_view = use "split-view"
local icon_button = use "icon-button"

local WORKTREES = { "main", "feat/multi-currency", "fix/rounding" }

return mod {
	init = function(previous)
		return previous or { left = true, right = true, bottom = true, selected = "main" }
	end,

	update = function(state, msg)
		if msg == "left" or msg == "right" or msg == "bottom" then
			state[msg] = not state[msg]
		elseif typeof(msg) == "table" and msg.open then
			state.selected = msg.open
		end
	end,

	view = function(state)
		local items = {}
		for i, name in WORKTREES do
			items[i] = ui.item { name, selected = name == state.selected, msg = { open = name } }
		end
		return ui.column {
			gap = "small",
			ui.row {
				gap = "small",
				icon_button { icon = "panel-left", tooltip = "Sidebar", msg = "left", active = state.left },
				icon_button { icon = "panel-bottom", tooltip = "Bottom panel", msg = "bottom", active = state.bottom },
				icon_button { icon = "panel-right", tooltip = "Inspector", msg = "right", active = state.right },
			},
			split_view {
				id = "demo-split",
				height = 320,
				left = state.left,
				right = state.right,
				bottom = state.bottom,
				sidebar_size = 180,
				inspector_size = 180,
				bottom_size = 80,
				sidebar = ui.column { padding = "normal", gap = "small", ui.text { "WORKTREES", size = "small", tone = "muted" }, table.unpack(items) },
				main = ui.column {
					padding = "large",
					gap = "small",
					ui.text { state.selected, size = "large", weight = "bold" },
					ui.text { "Drag the dividers to resize the panes.", tone = "muted" },
				},
				inspector = ui.column { padding = "normal", gap = "small", ui.text { "INSPECTOR", size = "small", tone = "muted" }, ui.text { `{state.selected} · 2 changes`, size = "small" } },
				bottom_panel = ui.column { padding = "normal", ui.text { "cargo test — 2 passed", tone = "success", size = "small" } },
			},
		}
	end,
}
