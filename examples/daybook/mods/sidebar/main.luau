-- The sidebar: primary views (Today, Upcoming, Calendar, Notes, Tasks, Events,
-- Habits), then favorite and recent notes, folders and tags (each foldable,
-- clickable to filter or open, with inline + creators). Back and forward walk
-- full route history; the … at the bottom picks the look.

local icon_button = use "icon-button"
local menu = use "menu"

type Route = {
	view: string,
	note_id: string?,
	folder: string?,
	tag: string?,
}

type State = {
	view: string,
	note_id: string?,
	folder: string?,
	tag: string?,
	back: { Route },
	forward: { Route },
	folded: { [string]: boolean? },
	looks: boolean,
	adding_folder: boolean,
	new_folder: string,
	adding_tag: boolean,
	new_tag: string,
}

local VIEWS = {
	{ id = "today", label = "Today", icon = "sun" },
	{ id = "upcoming", label = "Upcoming", icon = "calendar" },
	{ id = "calendar", label = "Calendar", icon = "calendar-days" },
	{ id = "notes", label = "Notes", icon = "file-text" },
	{ id = "tasks", label = "Tasks", icon = "square-check" },
	{ id = "events", label = "Events", icon = "list" },
	{ id = "habits", label = "Habits", icon = "zap" },
}

local function current_route(state: State): Route
	return { view = state.view, note_id = state.note_id, folder = state.folder, tag = state.tag }
end

local function same_route(a: Route, b: Route): boolean
	return a.view == b.view and a.note_id == b.note_id and a.folder == b.folder and a.tag == b.tag
end

local function apply_route(state: State, route: Route)
	state.view = route.view
	state.note_id = route.note_id
	state.folder = route.folder
	state.tag = route.tag
end

local function go(state: State, route: Route)
	local now = current_route(state)
	if not same_route(now, route) then
		table.insert(state.back, now)
		state.forward = {}
		apply_route(state, route)
	end
	return state, { { to = "agenda", msg = { route = route } } }
end

return mod {
	region = "left",
	title = "Sidebar",

	styles = {
		[".nav"] = { padding = { 0, 12, 10, 12 }, gap = 2 },
		[".nav-row"] = { padding = { 7, 12 }, radius = 10, gap = 12 },
		[".nav-dark:hover"] = { background = "#ffffff0f" },
		[".nav-pale:hover"] = { background = "#0000000a" },
		[".nav-dark:selected"] = { background = "#ffffff1a" },
		[".nav-pale:selected"] = { background = "#00000010" },
		[".nav-label"] = { weight = "medium" },
		[".nav-today"] = { color = "danger" },
		[".nav-icon"] = { color = "muted-foreground" },
		[".nav-heading"] = { padding = { 14, 12, 4, 12 }, gap = 4 },
		[".nav-heading-text"] = { size = "small", weight = "medium", color = "muted-foreground" },
		[".nav-emoji"] = { size = "normal" },
		[".nav-count"] = { size = "small", color = "muted-foreground" },
		[".nav-inline"] = { padding = { 2, 10 }, gap = 6 },
		[".nav-input"] = { background = "#ffffff12", border = "none", radius = 8, padding = { 5, 8 } },
		[".nav-input-pale"] = { background = "#0000000a", border = "none", radius = 8, padding = { 5, 8 } },
		[".nav-footer"] = { padding = { 8, 14, 12, 14 } },
	},

	init = function(previous: State?): State
		return {
			view = previous and previous.view or "today",
			note_id = previous and previous.note_id or nil,
			folder = previous and previous.folder or nil,
			tag = previous and previous.tag or nil,
			back = {},
			forward = {},
			folded = previous and previous.folded or {},
			looks = false,
			adding_folder = false,
			new_folder = "",
			adding_tag = false,
			new_tag = "",
		}
	end,

	update = function(state: State, msg)
		if msg == "back" then
			local route = table.remove(state.back)
			if route then
				table.insert(state.forward, current_route(state))
				apply_route(state, route)
				return state, { { to = "agenda", msg = { route = route } } }
			end
			return
		elseif msg == "forward" then
			local route = table.remove(state.forward)
			if route then
				table.insert(state.back, current_route(state))
				apply_route(state, route)
				return state, { { to = "agenda", msg = { route = route } } }
			end
			return
		elseif msg == "looks" then
			state.looks = not state.looks
			return
		elseif msg == "close-looks" then
			state.looks = false
			return
		elseif msg == "start-folder" then
			state.folded.folders = nil
			state.adding_folder = not state.adding_folder
			state.new_folder = ""
			return
		elseif msg == "start-tag" then
			state.folded.tags = nil
			state.adding_tag = not state.adding_tag
			state.new_tag = ""
			return
		elseif msg == "cancel-inline" then
			state.adding_folder = false
			state.adding_tag = false
			return
		end
		if type(msg) ~= "table" then
			return
		end
		if msg.sync then
			local route: Route = msg.sync
			local now = current_route(state)
			if not same_route(now, route) then
				table.insert(state.back, now)
				state.forward = {}
				apply_route(state, route)
			end
		elseif msg.view then
			return go(state, { view = msg.view })
		elseif msg.note then
			return go(state, { view = "note", note_id = msg.note })
		elseif msg.folder then
			return go(state, { view = "folder", folder = msg.folder })
		elseif msg.tag then
			return go(state, { view = "tag", tag = msg.tag })
		elseif msg.fold then
			state.folded[msg.fold] = if state.folded[msg.fold] then nil else true
		elseif msg.msg == "folder-typing" then
			state.new_folder = msg.value
		elseif msg.msg == "folder-submit" then
			local name = msg.value
			state.adding_folder = false
			state.new_folder = ""
			if string.match(name, "%S") then
				return state, { { to = "provider:planner", msg = { folder_add = name } } }
			end
		elseif msg.msg == "tag-typing" then
			state.new_tag = msg.value
		elseif msg.msg == "tag-submit" then
			local name = msg.value
			state.adding_tag = false
			state.new_tag = ""
			if string.match(name, "%S") then
				return state, { { to = "provider:planner", msg = { tag_add = name } } }
			end
		elseif msg.pick then
			state.looks = false
			return state, { { to = "provider:planner", msg = { look = msg.pick } } }
		end
	end,

	view = function(state: State, app)
		local planner = app.planner
		local look = (planner and planner.look) or "classic"
		local dark = app.dark or look ~= "light"
		local tint = if dark then "nav-dark" else "nav-pale"
		local input_class = if dark then "nav-input" else "nav-input-pale"
		local items = (planner and planner.items) or {}

		local folder_counts: { [string]: number } = {}
		local tag_counts: { [string]: number } = {}
		for _, item in items do
			if item.folder and item.state ~= "done" then
				folder_counts[item.folder] = (folder_counts[item.folder] or 0) + 1
			end
			if item.state ~= "done" then
				for _, t in item.tags or {} do
					tag_counts[t] = (tag_counts[t] or 0) + 1
				end
			end
		end

		local function row(icon: any, label: string, msg: any, selected: boolean, count: number?)
			return ui.row {
				class = { "nav-row", tint },
				align = "center",
				selected = selected,
				msg = msg,
				icon,
				ui.text { label, class = "nav-label", truncate = true },
				ui.spacer(),
				count and count > 0 and ui.text { tostring(count), class = "nav-count" },
			}
		end

		local function heading(id: string, title: string, add_msg: string?)
			local folded = state.folded[id] == true
			return ui.row {
				class = "nav-heading",
				align = "center",
				ui.row {
					align = "center",
					gap = "small",
					grow = true,
					msg = { fold = id },
					ui.text { title, class = "nav-heading-text" },
					ui.icon { name = if folded then "chevron-right" else "chevron-down", size = "small", class = "nav-icon" },
				},
				add_msg and icon_button { icon = "plus", tooltip = `New {string.lower(title)}`, size = "small", msg = add_msg },
			}
		end

		local rows: { Element } = {}
		for _, view in VIEWS do
			local is_today = view.id == "today"
			local selected = state.view == view.id
			table.insert(rows, row(
				ui.icon { name = view.icon :: any, class = if is_today then "nav-today" else "nav-icon" },
				view.label,
				{ view = view.id },
				selected
			))
		end

		local favorites: { Element }, recent: { Element } = {}, {}
		for _, item in items do
			if item.kind == "note" then
				local selected = state.view == "note" and state.note_id == item.id
				local entry = row(
					ui.text { item.emoji or "📝", class = "nav-emoji" },
					item.title,
					{ note = item.id },
					selected
				)
				if item.favorite then
					table.insert(favorites, entry)
				else
					table.insert(recent, entry)
				end
			end
		end

		local function section(id: string, title: string, list: { Element }, add_msg: string?, inline: Element?)
			table.insert(rows, heading(id, title, add_msg))
			if not state.folded[id] then
				if inline then
					table.insert(rows, inline)
				end
				for _, entry in list do
					table.insert(rows, entry)
				end
			end
		end

		section("favorites", "Favorites", favorites)
		section("recent", "Recent", recent)

		local folders: { Element } = {}
		for _, name in (planner and planner.folders) or {} do
			table.insert(folders, row(
				ui.icon { name = "folder", class = "nav-icon" },
				name,
				{ folder = name },
				state.view == "folder" and state.folder == name,
				folder_counts[name]
			))
		end
		local folder_input: Element? = if state.adding_folder
			then ui.row {
				class = "nav-inline",
				ui.input {
					id = "new-folder",
					value = state.new_folder,
					placeholder = "Folder name…",
					change = "folder-typing",
					submit = "folder-submit",
					keys = { escape = "cancel-inline" },
					focus = true,
					grow = true,
					class = input_class,
				},
			}
			else nil
		section("folders", "Folders", folders, "start-folder", folder_input)

		local tags: { Element } = {}
		for _, tag: any in (planner and planner.tags) or {} do
			table.insert(tags, row(
				ui.row { width = 8, height = 8, shrink = false, style = { background = tag.color, radius = 4 } },
				tag.name,
				{ tag = tag.name },
				state.view == "tag" and state.tag == tag.name,
				tag_counts[tag.name]
			))
		end
		local tag_input: Element? = if state.adding_tag
			then ui.row {
				class = "nav-inline",
				ui.input {
					id = "new-tag",
					value = state.new_tag,
					placeholder = "Tag name…",
					change = "tag-typing",
					submit = "tag-submit",
					keys = { escape = "cancel-inline" },
					focus = true,
					grow = true,
					class = input_class,
				},
			}
			else nil
		section("tags", "Tags", tags, "start-tag", tag_input)

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
