-- The bar across the top: the org switcher on the left, the sections in the
-- middle (the current one raised), notifications and the user on the right.
-- It follows the console's `gantry/nav` topic and asks the console to go
-- elsewhere.

local menu = use "menu"
local tag = use "tag"

type State = { page: string, open: string?, org: string, unread: boolean }

local NAV: { any } = {
	{ id = "home", label = "Home", icon = "house" },
	{ id = "services", label = "Services", icon = "layout-dashboard" },
	{ id = "tasks", label = "Tasks", icon = "list-checks" },
	"|",
	{ id = "docs", label = "Docs", icon = "file-text" },
	{ id = "status", label = "Status", icon = "activity" },
	{ id = "changelog", label = "Changelog", icon = "history" },
}

local ORGS = {
	{ id = "halcyon", label = "HALCYON LABS", detail = "PRO" },
	{ id = "night-shift", label = "NIGHT SHIFT", detail = "FREE" },
	{ id = "lattice", label = "LATTICE RESEARCH", detail = "TEAM" },
}

local USER_MENU: { any } = {
	{ heading = "ines@halcyon.dev" },
	{ id = "profile", label = "Profile", icon = "user" },
	{ id = "keys", label = "API keys", icon = "key-round" },
	{ id = "billing", label = "Billing", icon = "layers" },
	"separator",
	{ id = "logout", label = "Log out", icon = "log-out" },
}

local NOTES = {
	{ icon = "triangle-alert", tone = "warning", title = "GPU usage above 90%", detail = "eval-scheduler · 8× H100", when = "4m" },
	{ icon = "rocket", tone = "muted", title = "v142 rolled out", detail = "token-relay · Tomas Reyes", when = "3h" },
	{ icon = "circle-pause", tone = "muted", title = "sandbox-pool paused", detail = "Kernel image rebuild", when = "1d" },
}

return mod {
	region = "top",
	subscribe = { "gantry/nav" },

	styles = {
		[".topbar"] = { padding = { 0, 16 } },
		[".org"] = { border = true, border_color = "border", padding = { 0, 10 }, gap = 8 },
		[".org:hover"] = { background = "#1c1c1c" },
		[".org-mark"] = { background = "#5b5bd6", color = "#ffffff" },
		[".org-name"] = { weight = "medium" },
		[".nav"] = { gap = 2 },
		[".nav-item"] = { padding = { 0, 12 }, gap = 8, color = "muted-foreground" },
		[".nav-item:hover"] = { color = "foreground" },
		[".nav-item:selected"] = { background = "#242424", color = "bright" },
		[".nav-rule"] = { background = "border" },
		[".bell"] = { background = "#1f1f1f", color = "muted-foreground" },
		[".bell:hover"] = { color = "foreground", background = "#262626" },
		[".bell-dot"] = { background = "#ef4444", radius = 999 },
		[".user"] = { background = "#1f1f1f", padding = { 0, 10, 0, 6 }, gap = 8 },
		[".user:hover"] = { background = "#262626" },
		[".user-mark"] = { background = "#f4a582", color = "#5a2a16", radius = 999 },
		[".note"] = { padding = { 10, 12 }, gap = 12, border = "bottom", border_color = "hairline" },
		[".note-title"] = { weight = "medium" },
		[".muted-small"] = { color = "muted-foreground", size = "small" },
		[".popover-head"] = { padding = { 8, 12 }, color = "muted-foreground", size = "small" },
	},

	init = function(previous: State?): State
		return { page = if previous then previous.page else "services", open = nil, org = if previous then previous.org else "halcyon", unread = true }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "close" then
				state.open = nil
			end
			return
		end
		if msg.topic == "gantry/nav" and type(msg.value) == "table" then
			state.page = msg.value.page
		elseif msg.toggle then
			state.open = if state.open == msg.toggle then nil else msg.toggle
			if msg.toggle == "bell" then
				state.unread = false
			end
		elseif msg.pick then
			if state.open == "org" then
				state.org = msg.pick
			end
			state.open = nil
			if msg.pick == "logout" or msg.pick == "profile" then
				return state
			end
		end
	end,

	view = function(state: State, app)
		local window = app.window
		local inset = window and window.platform == "macos" and window.titlebar == "hidden" and not window.fullscreen

		local org = ORGS[1]
		for _, entry in ORGS do
			if entry.id == state.org then
				org = entry
			end
		end
		local org_options = {}
		for _, entry in ORGS do
			table.insert(org_options, { id = entry.id, label = entry.label, detail = entry.detail, checked = entry.id == state.org })
		end

		local switcher = ui.popover {
			open = state.open == "org",
			dismiss = "close",
			width = 240,
			content = menu { items = org_options },
			ui.row {
				class = "org",
				height = 34,
				align = "center",
				msg = { toggle = "org" },
				ui.row { class = "org-mark", width = 20, height = 20, align = "center", justify = "center", ui.icon { name = "zap", size = "small" } },
				ui.text { org.label, class = "org-name", wrap = false },
				tag { label = org.detail },
				ui.icon { name = "chevron-down", size = "small", tone = "muted" },
			},
		}

		local items: { any } = {}
		for _, entry in NAV do
			if type(entry) == "string" then
				table.insert(items, ui.row { class = "nav-rule", width = 1, height = 18 })
			else
				table.insert(items, ui.row {
					class = "nav-item",
					height = 34,
					align = "center",
					selected = state.page == entry.id or (entry.id == "services" and state.page == "service"),
					msg = { go = entry.id },
					to = "console",
					ui.icon { name = entry.icon :: any, size = "small" },
					ui.text { entry.label, wrap = false },
				})
			end
		end

		local notes: { any } = {}
		for _, note in NOTES do
			table.insert(notes, ui.row {
				class = "note",
				align = "start",
				ui.icon { name = note.icon :: any, tone = note.tone :: any },
				ui.column { gap = "none", grow = true, ui.text { note.title, class = "note-title" }, ui.text { note.detail, class = "muted-small" } },
				ui.text { note.when, class = "muted-small" },
			})
		end

		local bell = ui.popover {
			open = state.open == "bell",
			dismiss = "close",
			width = 320,
			align = "end",
			content = ui.column { gap = "none", ui.text { "NOTIFICATIONS", class = "popover-head" }, table.unpack(notes) },
			ui.row {
				class = "bell",
				width = 34,
				height = 34,
				align = "center",
				justify = "center",
				msg = { toggle = "bell" },
				tooltip = "Notifications",
				ui.icon { name = "bell", size = "small" },
				state.unread and ui.row { class = "bell-dot", width = 6, height = 6, float = { top = 7, right = 8 } },
			},
		}

		local user = ui.popover {
			open = state.open == "user",
			dismiss = "close",
			width = 220,
			align = "end",
			content = menu { items = USER_MENU },
			ui.row {
				class = "user",
				height = 34,
				align = "center",
				msg = { toggle = "user" },
				ui.row { class = "user-mark", width = 20, height = 20, align = "center", justify = "center", ui.icon { name = "circle-user", size = "small" } },
				ui.text { "Ines Varga", class = "org-name", wrap = false },
				ui.icon { name = "chevron-down", size = "small", tone = "muted" },
			},
		}

		return ui.row {
			class = "topbar",
			grow = true,
			height = 56,
			align = "center",
			drag = true,
			ui.row { width = 380, shrink = false, align = "center", gap = "none", inset and ui.row { width = 72 }, switcher },
			ui.spacer(),
			ui.row { class = "nav", align = "center", table.unpack(items) },
			ui.spacer(),
			ui.row { width = 380, shrink = false, align = "center", justify = "end", bell, user },
		}
	end,
}
