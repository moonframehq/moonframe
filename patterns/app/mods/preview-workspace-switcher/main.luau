local workspace_switcher = use "workspace-switcher"
local WORKSPACES = { { id = "acme", label = "Acme Studio" }, { id = "personal", label = "Personal" } }

return mod {
	init = function() return { selected = "acme", open = false, highlighted = 1, created = false } end,
	update = function(state, msg)
		if msg == "toggle" then state.open = not state.open
		elseif msg == "dismiss" then state.open = false
		elseif msg == "next" then state.highlighted = state.highlighted % #WORKSPACES + 1
		elseif msg == "previous" then state.highlighted = (state.highlighted - 2) % #WORKSPACES + 1
		elseif msg == "choose" then state.selected = WORKSPACES[state.highlighted].id; state.open = false
		elseif msg == "create" then state.created = true; state.open = false
		elseif type(msg) == "table" and msg.workspace then state.selected = msg.workspace; state.open = false end
	end,
	view = function(state)
		local name = if state.selected == "acme" then "Acme Studio" else "Personal"
		return ui.column { gap = "large", height = 280,
			workspace_switcher {
				name = name, selected = state.selected, workspaces = WORKSPACES,
				open = state.open, toggle = "toggle", dismiss = "dismiss", create = "create",
				highlighted = WORKSPACES[state.highlighted].id,
				keys = { down = "next", up = "previous", enter = "choose" },
			},
			ui.text { if state.created then "Create workspace selected — connect your app's flow here." else "Click the workspace to switch. Arrow keys and Enter work in the menu.", tone = "muted", size = "small" },
		}
	end,
}
