-- The sidebar: the workspace switcher beside the traffic lights, a search
-- field with its "/" key, the pages (Leads Report raised), favourite
-- companies and people tagged by what they are, and saved searches with
-- coloured marks. Choosing a page tells the report.

type State = { page: string, query: string }

local PAGES: { any } = {
	{ id = "assigned", label = "Assigned to me", icon = "message-square" },
	{ id = "drafts", label = "Drafts", icon = "file" },
	{ id = "schedule", label = "Schedule", icon = "calendar" },
	{ id = "customers", label = "Customers", icon = "user" },
	{ id = "leads", label = "Leads Report", icon = "network" },
	{ id = "companies", label = "Companies", icon = "building-2" },
}

-- A mark is an icon in a colour, or a person's initials on a disc.
local FAVORITES: { any } = {
	{ id = "orchard", label = "Orchard", kind = "Company", icon = "heart", color = "#f4f4f4" },
	{ id = "gamut", label = "Gamut", kind = "Company", icon = "palette", color = "#4b9dff" },
	{ id = "loopwise", label = "Loopwise", kind = "Company", icon = "workflow", color = "#b56bff" },
	{ id = "theo", label = "Theo Marsh", kind = "Investor", initials = "TM", color = "#3f7cf0" },
	{ id = "ines", label = "Ines Duval", kind = "Designer", initials = "ID", color = "#d9822b" },
}

local SEARCHES: { any } = {
	{ id = "success", label = "Customer Success", icon = "sparkles", color = "#ff7a1a" },
	{ id = "outsourcing", label = "Outsourcing", icon = "chart-pie", color = "#1fc77a" },
	{ id = "fundraising", label = "Fundraising", icon = "chart-column", color = "#ff3d6e" },
	{ id = "recruiting", label = "Recruiting", icon = "search", color = "#3b82f6" },
}

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 0, 10, 16, 10 }, gap = 2 },
		[".side-top"] = { padding = { 0, 6 }, gap = 8 },
		[".side-logo"] = { gap = 2 },
		[".side-blob"] = { background = "foreground", radius = 5 },
		[".side-name"] = { weight = "medium" },
		[".side-toggle"] = { color = "muted-foreground", radius = 6, padding = 4 },
		[".side-toggle:hover"] = { background = "list-hover", color = "foreground" },
		[".side-row"] = { padding = { 0, 8 }, radius = 8, gap = 10, color = "#d6d6d6" },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { background = "#232323", color = "foreground" },
		[".side-icon"] = { color = "muted-foreground" },
		[".side-row-on"] = { color = "foreground" },
		[".side-search-input"] = { background = "#00000000", border = "none", shadow = "none", padding = { 4, 0 }, color = "foreground" },
		[".side-slash"] = { background = "#222222", radius = 6, color = "muted-foreground", font = "mono", size = "small" },
		[".side-heading"] = { font = "mono", size = "small", color = "faint", padding = { 22, 8, 8, 8 } },
		[".side-tag"] = { font = "mono", size = "small", color = "faint" },
		[".side-mark"] = { radius = 6 },
		[".side-disc"] = { radius = 9, size = "small", weight = "bold", color = "#ffffff" },
	},

	init = function(previous: State?): State
		return { page = previous and previous.page or "leads", query = "" }
	end,

	update = function(state: State, msg)
		if type(msg) == "table" then
			if msg.page then
				state.page = msg.page
				return state, { { to = "report", msg = { page = msg.page } } }
			elseif msg.msg == "query" then
				state.query = msg.value
			elseif msg.msg == "clear" then
				state.query = ""
			end
		end
	end,

	view = function(state: State, app)
		local window = app.window
		local function row(item: any, ...)
			return ui.row {
				class = "side-row",
				height = 32,
				align = "center",
				selected = item.id == state.page,
				msg = { page = item.id },
				...,
			}
		end

		local rows = {
			ui.row {
				class = "side-top",
				drag = true,
				height = 52,
				align = "center",
				window.platform == "macos" and window.titlebar == "hidden" and not window.fullscreen and ui.row { width = 62 },
				ui.row {
					class = "side-logo",
					align = "center",
					ui.row { class = "side-blob", width = 9, height = 12 },
					ui.row { class = "side-blob", width = 9, height = 12 },
					ui.row { class = "side-blob", width = 12, height = 12, style = { radius = 6 } },
				},
				ui.text { "Halcyon", class = "side-name" },
				ui.icon { name = "chevrons-up-down", size = "small", tone = "muted" },
				ui.spacer(),
				ui.row {
					class = "side-toggle",
					msg = "toggle-left",
					to = "shell",
					tooltip = "Hide the sidebar",
					ui.icon { name = "panel-left", size = "small" },
				},
			},
			ui.row {
				class = "side-row",
				height = 34,
				align = "center",
				ui.icon { name = "search", size = "small", class = "side-icon" },
				ui.input {
					id = "search",
					value = state.query,
					placeholder = "Search",
					change = "query",
					keys = { escape = "clear" },
					grow = true,
					class = "side-search-input",
				},
				ui.row { class = "side-slash", width = 22, height = 22, align = "center", justify = "center", ui.text "/" },
			},
		}
		local query = string.lower(state.query)
		local function matches(label: string): boolean
			return query == "" or string.find(string.lower(label), query, 1, true) ~= nil
		end

		for _, page in PAGES do
			if matches(page.label) then
				local on = page.id == state.page
				table.insert(rows, row(
					page,
					ui.icon { name = page.icon, size = "small", class = if on then "side-row-on" else "side-icon" },
					ui.text { page.label }
				))
			end
		end

		table.insert(rows, ui.text { "FAVORITES", class = "side-heading" })
		for _, item in FAVORITES do
			if matches(item.label) then
				local mark = if item.initials
					then ui.row {
						class = "side-disc",
						width = 18,
						height = 18,
						align = "center",
						justify = "center",
						style = { background = item.color },
						ui.text { (string.match(item.initials, "^" .. utf8.charpattern) or ""), size = "small" },
					}
					else ui.icon { name = item.icon, size = "small", style = { color = item.color } }
				table.insert(rows, row(
					item,
					ui.row { width = 18, justify = "center", mark },
					ui.text { item.label, truncate = true },
					ui.spacer(),
					ui.text { string.upper(item.kind), class = "side-tag" }
				))
			end
		end

		table.insert(rows, ui.text { "SEARCHES", class = "side-heading" })
		for _, item in SEARCHES do
			if matches(item.label) then
				table.insert(rows, row(
					item,
					ui.row {
						class = "side-mark",
						width = 18,
						height = 18,
						align = "center",
						justify = "center",
						style = { background = item.color, color = "#ffffff" },
						ui.icon { name = item.icon, size = "small", style = { size = "small" } },
					},
					ui.text { item.label, truncate = true }
				))
			end
		end

		return ui.column {
			grow = true,
			scroll = true,
			gap = "none",
			ui.column { class = "side", shrink = false, table.unpack(rows) },
		}
	end,
}
