-- The sidebar: the workspace switcher, the places (only Mail is in this
-- example), and who is signed in. Mail's count is its unread mail, which
-- the mail mod publishes on `tidings/unread` as it changes.

local avatar = use "avatar"
local icon_button = use "icon-button"

type State = { selected: string, unread: number? }

local PLACES = {
	{ id = "next", label = "Up next", icon = "clock" },
	{ id = "inbox", label = "Inbox", icon = "inbox", count = 10 },
	{ id = "mail", label = "Mail", icon = "mail" },
	{ id = "calendar", label = "Calendar", icon = "calendar" },
	{ id = "support", label = "Support", icon = "headphones", count = 3 },
	{ id = "tasks", label = "Tasks", icon = "circle-check" },
	{ id = "notes", label = "Notes", icon = "file" },
}

return mod {
	region = "left",
	title = "Sidebar",
	subscribe = { "tidings/unread" },

	styles = {
		[".side"] = { padding = { 6, 8, 12, 16 }, gap = 0 },
		[".side-workspace"] = {
			background = "chrome",
			radius = 12,
			padding = { 9, 12, 9, 10 },
			gap = 10,
			shadow = "0 1px 2px #0000000d",
		},
		[".side-workspace:hover"] = { background = "background" },
		[".side-logo"] = { background = "#18181b", color = "#ffffff", radius = 6 },
		[".side-places"] = { padding = { 14, 0, 0, 0 }, gap = 2 },
		[".side-row"] = { padding = { 8, 10 }, radius = 10, gap = 12, color = "ink" },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { background = "#0000000f", color = "foreground" },
		[".side-icon"] = { color = "muted-foreground" },
		[".side-row-label"] = { size = "normal" },
		[".side-count"] = { size = "small", color = "muted-foreground" },
		[".side-user"] = { padding = { 6, 4, 0, 4 }, gap = 10 },
	},

	init = function(previous: State?): State
		return { selected = "mail", unread = previous and previous.unread }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" and msg.topic == "tidings/unread" then
			state.unread = msg.value
		elseif type(msg) == "table" and msg.select then
			state.selected = msg.select
		end
	end,

	view = function(state: State, app)
		local mailbox = app.mailbox
		local account = mailbox and mailbox.account
		local rows = {}
		for _, place in PLACES do
			local count = if place.id == "mail" then state.unread else place.count
			table.insert(rows, ui.row {
				class = "side-row",
				align = "center",
				selected = state.selected == place.id,
				msg = { select = place.id },
				tooltip = if place.id == "mail" then nil else "Only Mail is in this example",
				ui.icon { name = place.icon :: any, class = "side-icon" },
				ui.text { place.label, class = "side-row-label" },
				ui.spacer(),
				count and count > 0 and ui.text { tostring(count), class = "side-count" },
			})
		end

		return ui.column {
			class = "side",
			grow = true,
			ui.row {
				class = "side-workspace",
				align = "center",
				ui.row {
					class = "side-logo",
					width = 22,
					height = 22,
					align = "center",
					justify = "center",
					ui.icon { name = "sparkles", size = "small" },
				},
				ui.text { account and account.workspace or "Fernhill Studio", weight = "medium" },
				ui.spacer(),
				ui.icon { name = "chevrons-up-down", size = "small", tone = "muted" },
			},
			ui.column { class = "side-places", table.unpack(rows) },
			ui.spacer(),
			ui.row {
				class = "side-user",
				align = "center",
				avatar { name = account and account.name or "Robin Hale", shape = "circle", color = "#3f3f46" },
				ui.text { account and string.match(account.name, "^%S+") or "Robin" },
				ui.spacer(),
				icon_button { icon = "settings", tooltip = "Settings (not in this example)", size = "small", msg = { select = "settings" } },
			},
		}
	end,
}
