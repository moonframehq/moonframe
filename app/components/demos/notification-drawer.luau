-- A notification centre: the bell opens the drawer, its tabs filter by
-- kind, Mark all read clears the dots, × on a card dismisses it, and the
-- bell's dot goes once nothing is unread. Reset brings the samples back.
local notification_drawer = use "notification-drawer"
local icon_button = use "icon-button"

local function samples()
	return {
		{ id = 1, title = "CRM sync failed - auth token expired", body = "The OAuth token for the CRM has expired. Re-authenticate to resume syncing.", time = "15m ago", level = "error", read = false },
		{ id = 2, title = "Lead scoring pipeline completed", body = "Processed and scored 284 new leads from the sales inbox.", time = "30m ago", level = "success", read = false },
		{ id = 3, title = "New template: Chat Router", body = "A new community template is now available in the library.", time = "1h ago", level = "info", read = false },
		{ id = 4, title = "Support router processed 47 tickets", body = "Routed and answered 47 incoming tickets. Average reply in 42s.", time = "2h ago", level = "success", read = true },
		{ id = 5, title = "Draft \"Onboarding Flow\" not saved", body = "The canvas lost its connection while saving 12 nodes.", time = "6h ago", level = "error", read = true },
	}
end

return mod {
	init = function(previous)
		return previous or { open = true, filter = "all", items = samples() }
	end,

	update = function(state, msg)
		if msg == "toggle" then
			state.open = not state.open
		elseif msg == "close" then
			state.open = false
		elseif msg == "read-all" then
			for _, item in state.items do
				item.read = true
			end
		elseif msg == "reset" then
			state.items = samples()
		elseif type(msg) == "table" and msg.filter then
			state.filter = msg.filter
		elseif type(msg) == "table" and msg.remove then
			for i, item in state.items do
				if item.id == msg.remove then
					table.remove(state.items, i)
					break
				end
			end
		elseif type(msg) == "table" and msg.read then
			for _, item in state.items do
				if item.id == msg.read then
					item.read = true
				end
			end
		end
	end,

	view = function(state)
		local unread = 0
		local items = {}
		for _, item in state.items do
			if not item.read then
				unread += 1
			end
			-- A click on a card marks it read.
			table.insert(items, {
				id = item.id,
				title = item.title,
				body = item.body,
				time = item.time,
				level = item.level,
				read = item.read,
				msg = { read = item.id },
			})
		end
		return ui.row {
			gap = "normal",
			align = "center",
			icon_button { icon = "bell", tooltip = "Notifications", variant = "outline", msg = "toggle", badge = if unread > 0 then unread else nil },
			ui.button { "Reset", small = true, variant = "ghost", msg = "reset" },
			notification_drawer {
				open = state.open,
				dismiss = "close",
				mark_read = "read-all",
				filter = state.filter,
				items = items,
			},
		}
	end,
}
