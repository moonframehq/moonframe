-- The sidebar: the workspace, a search that filters what is below it, the
-- pages, saved views as coloured folders that open, and a reminder card
-- for the next notice (dismissible). It owns which page shows and tells
-- the shell; other mods send it { page = id } to go somewhere.

local kbd = use "kbd"

type State = { page: string, query: string, open: { [string]: boolean }, reminder: boolean }

local TOP: { any } = {
	{ id = "reports", label = "Reports", icon = "chart-column" },
	{ id = "rules", label = "Rules", icon = "book-open" },
}

local PAGES: { any } = {
	{ id = "today", label = "Today", icon = "layout-dashboard" },
	{ id = "contracts", label = "Contracts", icon = "file-text" },
	{ id = "payments", label = "Payments", icon = "file" },
	{ id = "performance", label = "Performance", icon = "activity" },
	{ id = "notices", label = "Notices", icon = "bell" },
	{ id = "public", label = "Public record", icon = "globe" },
	{ id = "settings", label = "Settings", icon = "settings" },
}

local function label_of(id: string): string
	for _, item in TOP do
		if item.id == id then
			return item.label
		end
	end
	for _, item in PAGES do
		if item.id == id then
			return item.label
		end
	end
	return id
end

local function matches(text: string, query: string): boolean
	return query == "" or string.find(string.lower(text), string.lower(query), 1, true) ~= nil
end

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".side"] = { padding = { 40, 12, 12, 12 }, gap = 0 },
		[".side-org"] = { background = "background", border = true, border_color = "border", radius = 10, padding = { 9, 10 }, gap = 10, shadow = "0 1px 2px #0000000a" },
		[".side-logo"] = { background = "foreground", color = "background", radius = 6 },
		[".side-search"] = { padding = { 4, 10 }, gap = 8, radius = 8 },
		[".side-search-input"] = { background = "#00000000", border = "none", padding = { 4, 0 } },
		[".side-row"] = { padding = { 6, 10 }, radius = 8, gap = 10, border = true, border_color = "#00000000" },
		[".side-row:hover"] = { background = "list-hover" },
		[".side-row:selected"] = { background = "background", border_color = "border", shadow = "0 1px 2px #0000000a" },
		[".side-icon"] = { color = "muted-foreground" },
		[".side-label"] = { color = "foreground/85" },
		[".side-label-on"] = { color = "foreground", weight = "medium" },
		[".side-rule"] = { background = "border" },
		[".side-caret"] = { color = "muted-foreground", size = "small" },
		[".side-child"] = { padding = { 5, 10, 5, 40 }, radius = 8 },
		[".side-child:hover"] = { background = "list-hover" },
		[".side-empty"] = { color = "muted-foreground", size = "small", padding = { 6, 10 } },
		[".side-card"] = { background = "secondary", border = true, border_color = "border", radius = 12, padding = 14, gap = 4 },
		[".side-card-tile"] = { background = "background", border = true, border_color = "border", radius = 8 },
		[".side-card-close"] = { color = "muted-foreground", radius = 4, padding = 2 },
		[".side-card-close:hover"] = { background = "list-hover", color = "foreground" },
		[".side-card-title"] = { weight = "medium", size = "large" },
		[".side-card-line"] = { color = "muted-foreground" },
		[".side-card-button"] = { background = "#e4e5e8", border = true, border_color = "border", radius = 8, padding = { 8, 12 } },
		[".side-card-button:hover"] = { background = "#dcdde1" },
	},

	init = function(previous: State?): State
		local state: State = previous or { page = "today", query = "", open = {}, reminder = true }
		state.query = ""
		return state
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "dismiss" then
				state.reminder = false
			elseif msg == "remind" then
				state.reminder = true
			end
			return
		end
		if msg.page then
			state.page = msg.page
			return state, { { to = "shell", msg = { page = msg.page, label = label_of(msg.page) } } }
		elseif msg.msg == "query" then
			state.query = msg.value
		elseif msg.msg == "clear" then
			state.query = ""
		elseif msg.fold then
			state.open[msg.fold] = not state.open[msg.fold]
		end
	end,

	view = function(state: State, app)
		local data = app.duly
		local query = state.query

		local function row(item: any)
			local on = state.page == item.id
			return ui.row {
				class = "side-row",
				align = "center",
				selected = on,
				msg = { page = item.id },
				ui.icon { name = item.icon :: any, size = "small", class = if on then "side-label-on" else "side-icon" },
				ui.text { item.label, class = if on then "side-label-on" else "side-label" },
			}
		end

		local top, pages, folders = {}, {}, {}
		for _, item in TOP do
			if matches(item.label, query) then
				table.insert(top, row(item))
			end
		end
		for _, item in PAGES do
			if matches(item.label, query) then
				table.insert(pages, row(item))
			end
		end
		for _, folder in (data and data.folders) or {} do
			local children = {}
			for _, child in folder.items do
				if matches(child, query) then
					table.insert(children, child)
				end
			end
			if matches(folder.label, query) or #children > 0 then
				local open = state.open[folder.id] or (query ~= "" and #children > 0)
				table.insert(folders, ui.row {
					class = "side-row",
					align = "center",
					msg = { fold = folder.id },
					ui.icon { name = "folder", size = "small", style = { color = folder.color } },
					ui.text { folder.label, class = "side-label" },
					ui.spacer(),
					ui.text { if open then "▼" else "▶", class = "side-caret" },
				})
				if open then
					for _, child in (if #children > 0 then children else folder.items) do
						table.insert(folders, ui.row {
							class = "side-child",
							msg = { page = if folder.id == "ratings" then "contracts" else "payments" },
							ui.text { child, size = "small", class = "side-label" },
						})
					end
				end
			end
		end
		if query == "" or matches("Create view", query) then
			table.insert(folders, ui.row {
				class = "side-row",
				align = "center",
				msg = { page = "views" },
				ui.icon { name = "plus", size = "small", class = "side-icon" },
				ui.text { "Create view", class = "side-label" },
			})
		end

		local reminder = data and data.reminder
		local card = state.reminder and reminder and ui.column {
			class = "side-card",
			ui.row {
				align = "start",
				ui.row { class = "side-card-tile", width = 40, height = 40, align = "center", justify = "center", ui.icon { name = "pencil" } },
				ui.spacer(),
				ui.row { class = "side-card-close", msg = "dismiss", tooltip = "Dismiss", ui.icon { name = "close", size = "small" } },
			},
			ui.row { height = 8 },
			ui.text { reminder.title, class = "side-card-title" },
			ui.text { reminder.lines[1], class = "side-card-line" },
			ui.text { reminder.lines[2], class = "side-card-line" },
			ui.row { height = 10 },
			ui.row { class = "side-card-button", justify = "center", msg = { page = "payments" }, ui.text { "Open notice" } },
		}

		local org = data and data.org or { name = "Duly" }
		return ui.column {
			class = "side",
			grow = true,
			ui.row { height = 1, drag = true },
			ui.row {
				class = "side-org",
				align = "center",
				ui.row { class = "side-logo", width = 24, height = 24, align = "center", justify = "center", shrink = false, ui.icon { name = "list-checks", size = "small" } },
				ui.text { org.name, weight = "medium" },
				ui.spacer(),
				ui.icon { name = "chevrons-up-down", size = "small", class = "side-icon" },
			},
			ui.row { height = 10 },
			ui.row {
				class = "side-search",
				align = "center",
				ui.icon { name = "search", size = "small", class = "side-icon" },
				ui.input {
					id = "nav-search",
					class = "side-search-input",
					value = state.query,
					placeholder = "Search",
					change = "query",
					keys = { escape = "clear" },
					grow = true,
				},
				kbd { keys = { "/" }, small = true },
			},
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column {
					gap = "none",
					shrink = false,
					ui.column { gap = "none", table.unpack(top) },
					#top > 0 and ui.row { class = "side-rule", height = 1, style = { margin = { 10, 0 } } },
					ui.column { gap = "none", table.unpack(pages) },
					#pages > 0 and ui.row { class = "side-rule", height = 1, style = { margin = { 10, 0 } } },
					ui.column { gap = "none", table.unpack(folders) },
					#top + #pages + #folders == 0 and ui.text { "Nothing matches", class = "side-empty" },
				},
			},
			card,
		}
	end,
}
