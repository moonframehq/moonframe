-- The sidebar: the views (Today, Upcoming, Calendar, Notes, Tasks, Events),
-- then favourite and recent notes, folders and tags, each section folding.
-- Choosing a view tells the agenda; back and forward walk the views chosen.
-- The … at the bottom picks the look.

local icon_button = use "icon-button"
local menu = use "menu"

type State = {
	view: string,
	back: { string },
	forward: { string },
	folded: { [string]: boolean? },
	looks: boolean,
}

local VIEWS = {
	{ id = "today", label = "Today", icon = "sun" },
	{ id = "upcoming", label = "Upcoming", icon = "calendar" },
	{ id = "calendar", label = "Calendar", icon = "layout-dashboard" },
	{ id = "notes", label = "Notes", icon = "file-text" },
	{ id = "tasks", label = "Tasks", icon = "square-check" },
	{ id = "events", label = "Events", icon = "list" },
}

local function go(state: State, view: string)
	if view ~= state.view then
		table.insert(state.back, state.view)
		state.forward = {}
		state.view = view
	end
	return state, { { to = "agenda", msg = { view = view } } }
end

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".nav"] = { padding = { 0, 12, 10, 12 }, gap = 2 },
		[".nav-row"] = { padding = { 8, 12 }, radius = 10, gap = 12 },
		[".nav-dark:hover"] = { background = "#ffffff0f" },
		[".nav-pale:hover"] = { background = "#0000000a" },
		[".nav-dark:selected"] = { background = "#ffffff1a" },
		[".nav-pale:selected"] = { background = "#00000010" },
		[".nav-label"] = { weight = "medium" },
		[".nav-today"] = { color = "danger" },
		[".nav-icon"] = { color = "muted-foreground" },
		[".nav-heading"] = { padding = { 16, 12, 4, 12 }, gap = 4 },
		[".nav-heading-text"] = { size = "small", weight = "medium", color = "muted-foreground" },
		[".nav-emoji"] = { size = "normal" },
		[".nav-footer"] = { padding = { 8, 14, 12, 14 } },
	},

	init = function(previous: State?): State
		return previous or { view = "today", back = {}, forward = {}, folded = {}, looks = false }
	end,

	update = function(state: State, msg)
		if msg == "back" then
			local view = table.remove(state.back)
			if view then
				table.insert(state.forward, state.view)
				state.view = view
				return state, { { to = "agenda", msg = { view = view } } }
			end
			return
		elseif msg == "forward" then
			local view = table.remove(state.forward)
			if view then
				table.insert(state.back, state.view)
				state.view = view
				return state, { { to = "agenda", msg = { view = view } } }
			end
			return
		elseif msg == "looks" then
			state.looks = not state.looks
			return
		elseif msg == "close-looks" then
			state.looks = false
			return
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.view then
			return go(state, msg.view)
		elseif msg.fold then
			state.folded[msg.fold] = if state.folded[msg.fold] then nil else true
		elseif msg.note then
			return go(state, "notes")
		elseif msg.pick then
			state.looks = false
			if msg.pick == "themes" then
				return
			end
			return state, { { to = "provider:planner", msg = { look = msg.pick } } }
		end
	end,

	view = function(state: State, app)
		local planner = app.planner
		local look = (planner and planner.look) or "classic"
		local tint = if app.dark or look ~= "light" then "nav-dark" else "nav-pale"
		local items = (planner and planner.items) or {}

		local function row(icon: any, label: string, msg: any, selected: boolean, extra: string?)
			return ui.row {
				class = { "nav-row", tint },
				align = "center",
				selected = selected,
				msg = msg,
				icon,
				ui.text { label, class = if extra then { "nav-label", extra } else "nav-label", truncate = true },
			}
		end

		local function heading(id: string, title: string, add: boolean?)
			local folded = state.folded[id] == true
			return ui.row {
				class = "nav-heading",
				align = "center",
				msg = { fold = id },
				ui.text { title, class = "nav-heading-text" },
				ui.icon { name = if folded then "chevron-right" else "chevron-down", size = "small", class = "nav-icon" },
				ui.spacer(),
				add and icon_button { icon = "plus", tooltip = `New {string.lower(title)}`, size = "small", msg = { fold = id } },
			}
		end

		local rows = {}
		for _, view in VIEWS do
			local today = view.id == "today"
			table.insert(rows, row(
				ui.icon { name = view.icon :: any, class = if today then "nav-today" else "nav-icon" },
				view.label,
				{ view = view.id },
				state.view == view.id
			))
		end

		local favorites, recent = {}, {}
		for _, item in items do
			if item.kind == "note" then
				local entry = row(ui.text { item.emoji or "📝", class = "nav-emoji" }, item.title, { note = item.id }, false)
				if item.favorite then
					table.insert(favorites, entry)
				else
					table.insert(recent, entry)
				end
			end
		end

		local function section(id: string, title: string, list: { any }, add: boolean?)
			table.insert(rows, heading(id, title, add))
			if not state.folded[id] then
				for _, entry in list do
					table.insert(rows, entry)
				end
			end
		end

		section("favorites", "Favorites", favorites)
		section("recent", "Recent", recent)
		local folders = {}
		for _, name in (planner and planner.folders) or {} do
			table.insert(folders, row(ui.icon { name = "folder", class = "nav-icon" }, name, { view = "tasks" }, false))
		end
		section("folders", "Folders", folders, true)
		local tags = {}
		for _, tag in (planner and planner.tags) or {} do
			table.insert(tags, row(
				ui.row { width = 8, height = 8, style = { background = tag.color, radius = 4 } },
				tag.name,
				{ view = "tasks" },
				false
			))
		end
		section("tags", "Tags", tags, true)

		-- A typed list: the heading and the items differ in shape.
		local looks: { any } = {
			{ heading = "Look" },
			{ id = "classic", label = "Classic", checked = look == "classic" },
			{ id = "sepia", label = "Sepia", checked = look == "sepia" },
			{ id = "light", label = "Light", checked = look == "light" },
		}

		return ui.column {
			grow = true,
			gap = "none",
			ui.column { grow = true, scroll = true, gap = "none", ui.column { class = "nav", shrink = false, table.unpack(rows) } },
			ui.row {
				class = "nav-footer",
				ui.popover {
					open = state.looks,
					dismiss = "close-looks",
					side = "top",
					width = 220,
					content = menu { items = looks },
					icon_button { icon = "ellipsis", tooltip = "Look", msg = "looks" },
				},
			},
		}
	end,
}
