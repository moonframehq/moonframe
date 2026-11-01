-- The bar over the report (the shell mounts it): the page's name and how many leads it covers,
-- then the period and an export button. With the sidebar hidden it makes
-- room for the traffic lights and offers the sidebar back.

type State = { page: string, sidebar: boolean }

local TITLES = {
	assigned = "Assigned to me",
	drafts = "Drafts",
	schedule = "Schedule",
	customers = "Customers",
	leads = "Leads Report",
	companies = "Companies",
}

return mod {
	title = "Header",

	-- The shell says on `sidebar` whether the sidebar shows.
	subscribe = { "sidebar" },

	styles = {
		[".head"] = { padding = { 0, 20, 0, 22 }, gap = 12 },
		[".head-count"] = { font = "mono", size = "small", color = "muted-foreground" },
		[".head-button"] = { background = "chip", border = true, border_color = "border", radius = 9, padding = { 0, 10 }, gap = 6, color = "#d6d6d6" },
		[".head-button:hover"] = { background = "#2a2a2a" },
		[".head-toggle"] = { color = "muted-foreground", radius = 6, padding = 4 },
		[".head-toggle:hover"] = { background = "list-hover", color = "foreground" },
	},

	init = function(previous: State?): State
		return { page = previous and previous.page or "leads", sidebar = true }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" then
			if msg.page then
				state.page = msg.page
			elseif msg.topic == "sidebar" then
				state.sidebar = msg.value == true
			end
		end
	end,

	view = function(state: State, app)
		local leads = app.leads
		local window = app.window
		local count = if leads and leads.available then `{leads.leads} LEADS` else ""
		local lights = not state.sidebar and window.platform == "macos" and window.titlebar == "hidden" and not window.fullscreen
		return ui.row {
			class = "head",
			drag = true,
			grow = true,
			height = 52,
			align = "center",
			lights and ui.row { width = 58 },
			not state.sidebar and ui.row {
				class = "head-toggle",
				msg = "toggle-left",
				to = "shell",
				tooltip = "Show the sidebar",
				ui.icon { name = "panel-left", size = "small" },
			},
			ui.text { TITLES[state.page] or "Leads Report" },
			ui.text { count, class = "head-count" },
			ui.spacer(),
			ui.row {
				class = "head-button",
				height = 30,
				align = "center",
				msg = "period",
				ui.icon { name = "calendar", size = "small", tone = "muted" },
				ui.text { "Jan – Dec 2026", size = "small" },
				ui.icon { name = "chevron-down", size = "small", tone = "muted" },
			},
			ui.row {
				class = "head-button",
				height = 30,
				align = "center",
				msg = "export",
				ui.icon { name = "download", size = "small", tone = "muted" },
				ui.text { "Export", size = "small" },
			},
		}
	end,
}
