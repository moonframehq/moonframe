-- The bar over the dashboard: where you are, a search field, Ask AI, and
-- the bell. The bell opens the notification drawer under the bar; this mod
-- keeps the notifications (sample data), their filter and what was read.

local notification_drawer = use "notification-drawer"

type Notification = {
	id: number,
	title: string,
	body: string,
	time: string,
	level: string,
	read: boolean,
}

type State = {
	open: boolean,
	filter: string,
	items: { Notification },
	next: number,
	query: string,
	focus: boolean,
}

local function samples(): { Notification }
	return {
		{ id = 1, title = "CRM Sync failed - auth token expired", body = "The OAuth token for the CRM has expired. Re-authenticate to resume the nightly sync.", time = "15m ago", level = "error", read = false },
		{ id = 2, title = "Lead Scoring Pipeline completed", body = "Successfully processed and scored 284 new leads from the sales inbox.", time = "30m ago", level = "success", read = false },
		{ id = 3, title = "New template: Chat Router", body = "A new community template \"Chat Router\" is now available in the library.", time = "1h ago", level = "info", read = false },
		{ id = 4, title = "Email Support Router processed 47 tickets", body = "Routed and responded to 47 incoming support tickets. Average reply in 42s.", time = "2h ago", level = "success", read = false },
		{ id = 5, title = "Weekly Report Generator scheduled", body = "Next run is scheduled for Monday 9:00 AM. The workflow is currently paused.", time = "3h ago", level = "info", read = false },
		{ id = 6, title = "Chat Digest - rate limit hit", body = "The chat API rate limit was briefly exceeded. 3 messages were retried.", time = "4h ago", level = "warning", read = false },
		{ id = 7, title = "Canvas workflow \"Onboarding Flow\" failed to save", body = "Your draft workflow could not be saved to the Canvas. 12 nodes are kept locally.", time = "6h ago", level = "error", read = false },
		{ id = 8, title = "CRM Sync completed", body = "Synced 1,204 contacts and 86 companies after re-authentication.", time = "Yesterday", level = "success", read = false },
		{ id = 9, title = "Token usage at 84% of monthly limit", body = "You have used 842K of your 1M monthly token allowance. Consider upgrading.", time = "Yesterday", level = "info", read = false },
	}
end

return mod {
	region = "top",
	title = "Top bar",

	styles = {
		[".bar"] = { padding = { 0, 16, 0, 16 }, gap = 10, background = "background", border = "bottom", border_color = "border" },
		[".bar-where"] = { color = "#4a4a50", gap = 8 },
		[".bar-search"] = { background = "background", border = true, border_color = "border", radius = 8, padding = { 0, 10 }, gap = 8 },
		[".bar-search-icon"] = { color = "muted-foreground" },
		[".bar-field"] = { background = "#00000000", border = "none", shadow = "none", padding = { 5, 0 } },
		[".bar-ask"] = { border = true, border_color = "border", radius = 8, padding = { 0, 10 }, gap = 6, background = "background" },
		[".bar-ask:hover"] = { background = "muted" },
		[".bar-bell"] = { border = true, border_color = "border", radius = 8, background = "background", color = "#4a4a50" },
		[".bar-bell:hover"] = { background = "muted" },
		[".bar-bell-open"] = { background = "ink", color = "background", border_color = "ink" },
		[".bar-bell-open:hover"] = { background = "ink" },
		[".bar-dot"] = { background = "bad", radius = 4, border = true, border_color = "background" },
	},

	init = function(previous: State?): State
		local state: State = previous
			or { open = false, filter = "all", items = samples(), next = 10, query = "", focus = false }
		state.focus = false
		return state
	end,

	update = function(state: State, msg)
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
		elseif msg == "focus" then
			state.focus = true
		elseif type(msg) == "table" and msg.msg == "query" then
			state.query = msg.value
			state.focus = false
		elseif type(msg) == "table" and msg.msg == "clear" then
			state.query = ""
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
		elseif type(msg) == "table" and msg.notify then
			-- Another mod reports something (a workflow created).
			table.insert(state.items, 1, {
				id = state.next,
				title = msg.notify,
				body = msg.body or "",
				time = "Just now",
				level = msg.level or "info",
				read = false,
			})
			state.next += 1
		end
	end,

	view = function(state: State)
		local unread = 0
		local items = {}
		for _, item in state.items do
			if not item.read then
				unread += 1
			end
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
			class = "bar",
			drag = true,
			grow = true,
			height = 56,
			align = "center",
			ui.row {
				class = "bar-where",
				align = "center",
				ui.icon { name = "layout-dashboard", size = "small" },
				ui.text { "Dashboard" },
			},
			ui.spacer(),
			ui.row {
				class = "bar-search",
				width = 240,
				height = 32,
				align = "center",
				ui.icon { name = "search", size = "small", class = "bar-search-icon" },
				ui.input {
					id = "search",
					value = state.query,
					placeholder = "Search for something...",
					change = "query",
					keys = { escape = "clear" },
					focus = state.focus,
					grow = true,
					class = "bar-field",
				},
			},
			ui.row {
				class = "bar-ask",
				height = 32,
				align = "center",
				tooltip = "Not in this example",
				msg = "focus",
				ui.icon { name = "sparkles", size = "small" },
				ui.text { "Ask AI", weight = "medium" },
			},
			ui.row {
				class = if state.open then "bar-bell bar-bell-open" else "bar-bell",
				width = 32,
				height = 32,
				align = "center",
				justify = "center",
				msg = "toggle",
				tooltip = if unread > 0 then `{unread} unread` else "Notifications",
				ui.icon { name = "bell", size = "small" },
				unread > 0 and not state.open and ui.row {
					class = "bar-dot",
					width = 8,
					height = 8,
					float = { top = 6, right = 7 },
				},
			},
			notification_drawer {
				open = state.open,
				dismiss = "close",
				mark_read = "read-all",
				filter = state.filter,
				items = items,
				top = 56,
				width = 440,
				empty = "Nothing here. View › Restore Sample Notifications brings them back.",
			},
		}
	end,
}
