local workspace_shell = use "workspace-shell"
local segmented = use "segmented"
local function loose(items: { any }): { any } return items end

return mod {
	init = function() return { activity = "home", selected = "inbox", filter = "all", sidebar = true } end,
	update = function(state, msg)
		if type(msg) ~= "table" then return end
		if msg.activity then state.activity = msg.activity
		elseif msg.select then state.selected = msg.select
		elseif msg.filter then state.filter = msg.filter
		elseif msg.sidebar then state.sidebar = not state.sidebar end
	end,
	view = function(state)
		return ui.column { gap = "normal",
			ui.button { "Toggle sidebar", icon = "panel-left", msg = { sidebar = true }, small = true, variant = "ghost" },
			workspace_shell {
				title = "Acme Studio", height = 380, sidebar_visible = state.sidebar,
				activity = state.activity, activities = {
					{ id = "home", icon = "house", tooltip = "Home" },
					{ id = "workflows", icon = "workflow", tooltip = "Workflows" },
				}, bottom = { { id = "settings", icon = "settings", tooltip = "Settings" } },
				selected = state.selected, sections = loose {
					{ items = loose { { id = "inbox", label = "Inbox", icon = "inbox", count = 16 }, { id = "sessions", label = "Sessions", icon = "messages-square" } } },
					{ title = "Folders", items = { { id = "checkout", label = "Checkout", icon = "folder" }, { id = "billing", label = "Billing", icon = "folder" } } },
				},
				content = ui.column { gap = "large",
					ui.text { if state.activity == "home" then state.selected else state.activity, size = "title", weight = "medium" },
					segmented { selected = state.filter, event = "filter", options = {
						{ id = "all", label = "All" }, { id = "mine", label = "Mine" }, { id = "agents", label = "Agents" },
					} },
					ui.text { `Showing {state.filter} · sample workspace`, tone = "muted", size = "small" },
				},
			},
		}
	end,
}
