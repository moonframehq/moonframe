-- A settings sidebar: click a page to select it; visiting Notifications
-- clears its warning dot.
local settings_nav = use "settings-nav"

return mod {
	init = function(previous)
		return previous or { selected = "sidebar", seen = {} :: { [string]: boolean } }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.select then
			state.selected = msg.select
			state.seen[msg.select] = true
		end
	end,

	view = function(state)
		return ui.row {
			gap = "large",
			height = 360,
			settings_nav {
				title = "Moonframe",
				subtitle = "Settings",
				selected = state.selected,
				groups = {
					{
						title = "Appearance",
						items = {
							{ id = "theme", label = "Theme", icon = "palette" },
							{ id = "sidebar", label = "Sidebar", icon = "panel-left" },
							{ id = "fonts", label = "Fonts", icon = "a-large-small" },
							{ id = "status-bar", label = "Status bar", icon = "panel-bottom" },
						},
					},
					{
						title = "Behaviour",
						items = {
							{ id = "general", label = "General", icon = "settings" },
							{ id = "tabs", label = "Tabs & panes", icon = "columns-2" },
							{
								id = "notifications",
								label = "Notifications",
								icon = "bell",
								dot = if state.seen.notifications then nil else "warning",
							},
						},
					},
					{
						title = "Connect",
						items = {
							{ id = "integrations", label = "Integrations", icon = "square-terminal" },
							{ id = "plugins", label = "Plugins", icon = "plug" },
							{ id = "cloud", label = "Cloud devices", icon = "globe" },
						},
					},
				},
			},
			ui.text { `Page: {state.selected}`, tone = "muted", size = "small" },
		}
	end,
}
