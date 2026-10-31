-- A panel header: the sidebar button toggles (shown pressed when on),
-- + opens a terminal (and a notification), the bell clears its count,
-- and the trash closes a terminal until none are left.
local icon_button = use "icon-button"

return mod {
	init = function(previous)
		return previous or { sidebar = true, unread = 3, terminals = 3, refreshed = 0 }
	end,

	update = function(state, msg)
		if msg == "sidebar" then
			state.sidebar = not state.sidebar
		elseif msg == "notifications" then
			state.unread = 0
		elseif msg == "new" then
			state.terminals += 1
			state.unread += 1
		elseif msg == "delete" then
			state.terminals = math.max(0, state.terminals - 1)
		elseif msg == "refresh" then
			state.refreshed += 1
		end
	end,

	view = function(state)
		return ui.column {
			gap = "large",
			ui.row {
				gap = "small",
				align = "center",
				icon_button { icon = "panel-left", tooltip = "Toggle sidebar", msg = "sidebar", active = state.sidebar },
				icon_button { icon = "plus", tooltip = "New terminal", msg = "new", variant = "solid" },
				icon_button { icon = "refresh-cw", tooltip = "Refresh", msg = "refresh" },
				ui.spacer(),
				icon_button {
					icon = "bell",
					tooltip = "Notifications",
					msg = "notifications",
					variant = "outline",
					badge = if state.unread > 0 then state.unread else nil,
				},
				icon_button {
					icon = "trash-2",
					tooltip = "Close terminal",
					msg = "delete",
					size = "small",
					tone = "danger",
					disabled = state.terminals == 0,
				},
			},
			ui.text {
				`Sidebar {if state.sidebar then "shown" else "hidden"}, {state.terminals} terminals, {state.unread} unread, refreshed {state.refreshed}×`,
				tone = "muted",
				size = "small",
			},
		}
	end,
}
