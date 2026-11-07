-- The left column: New thread, the app's pages, and the threads by day or by
-- project. Each row says at a glance what its thread is doing: a spinner while
-- it runs, a dot when something is new, a warning when it needs you.

local menu_button = use "menu-button"
local context_menu = use "context-menu"
local dialog = use "dialog"

local PAGES = {
	{ id = "threads", label = "Threads", icon = "folder" },
	{ id = "scheduled", label = "Scheduled", icon = "clock" },
	{ id = "skills", label = "Skills", icon = "layers" },
	{ id = "extensions", label = "Extensions", icon = "plug" },
	{ id = "settings", label = "Settings", icon = "settings" },
}

type State = {
	page: string,
	filter_open: boolean,
	show_archived: boolean,
	menu: string?,
	renaming: string?,
	deleting: { id: string, title: string, worktree: boolean }?,
	name: string,
	recent: { string },
	recent_at: number,
}

-- What the view last read, for `update`, which has no `app`.
local latest: any = nil

local function age(seconds: number): string
	if seconds < 60 then
		return "now"
	elseif seconds < 3600 then
		return `{seconds // 60}m`
	elseif seconds < 86400 then
		return `{seconds // 3600}h`
	end
	return `{seconds // 86400}d`
end

local function indicator(thread: any): any
	if thread.status == "running" then
		return ui.icon { "loader-circle", spin = true, class = "thread-spinner" }
	elseif thread.status == "waiting" then
		return ui.icon { "circle-alert", class = "thread-waiting" }
	elseif thread.status == "failed" then
		return ui.icon { "circle-x", class = "thread-failed" }
	elseif thread.unread then
		return ui.row { class = "thread-dot" }
	end
	return nil
end

local function thread_row(thread: any, state: State, pi: any): any
	local selected = thread.id == pi.selected and state.page == "threads"
	local items: { any } = {
		{ id = `pin:{thread.id}`, label = if thread.pinned then "Unpin" else "Pin", icon = "pin" },
		{ id = `rename:{thread.id}`, label = "Rename…", icon = "pencil" },
		{ id = `archive:{thread.id}`, label = if thread.archived then "Unarchive" else "Archive", icon = "archive" },
		"separator",
		{ id = `delete:{thread.id}`, label = "Delete", icon = "trash-2", danger = true },
	}
	return context_menu {
		open = state.menu == thread.id,
		context = { menu = thread.id },
		dismiss = "close-menu",
		items = items,
		ui.row {
			key = string.gsub(thread.id, "[^%w%-_]", ""),
			class = "thread-row",
			selected = selected,
			align = "center",
			msg = { open = thread.id },
			ui.row { class = "thread-mark", width = 16, justify = "center", shrink = false, indicator(thread) },
			ui.column {
				grow = true,
				gap = "none",
				ui.text { thread.title, truncate = true, class = "thread-title" },
				ui.row {
					align = "center",
					gap = "small",
					ui.text { thread.workspace, truncate = true, class = "thread-line" },
					ui.spacer(),
					thread.branch and ui.icon { "git-branch", class = "thread-branch" },
					ui.text { age(math.max(0, pi.now - thread.updated)), class = "thread-line" },
				},
				thread.preview ~= "" and ui.text { thread.preview, truncate = true, class = "thread-line" },
			},
		},
	}
end

local function heading(text: string): any
	return ui.text { text, class = "thread-heading" }
end

-- The rows in the order they show, for ⌘1 to ⌘9.
local function grouped(pi: any, state: State): { { title: string, rows: { any } } }
	local groups: { { title: string, rows: { any } } } = {}
	local by_title: { [string]: { title: string, rows: { any } } } = {}
	local function into(title: string, thread: any)
		if not by_title[title] then
			by_title[title] = { title = title, rows = {} }
			table.insert(groups, by_title[title])
		end
		table.insert(by_title[title].rows, thread)
	end
	local by_project = pi.prefs.group == "workspace"
	for _, thread in pi.threads do
		if thread.pinned and not thread.archived then
			into("Pinned", thread)
		end
	end
	for _, thread in pi.threads do
		if thread.archived then
			if state.show_archived then
				into("Archived", thread)
			end
		elseif not thread.pinned then
			if by_project then
				into(thread.workspace, thread)
			elseif thread.updated >= pi.day_start then
				into("Today", thread)
			elseif thread.updated >= pi.day_start - 86400 then
				into("Yesterday", thread)
			elseif thread.updated >= pi.day_start - 6 * 86400 then
				into("This week", thread)
			else
				into("Earlier", thread)
			end
		end
	end
	return groups
end

local function filter_items(pi: any, state: State): { any }
	return {
		{ heading = "Group by" },
		{ id = "group-time", label = "Time", checked = pi == nil or pi.prefs.group ~= "workspace" },
		{ id = "group-workspace", label = "Project", checked = pi ~= nil and pi.prefs.group == "workspace" },
		"separator",
		{ id = "show-archived", label = "Show archived", checked = state.show_archived },
	}
end

local function open_thread(id: string): { any }
	return { { to = "shell", msg = { page = "threads" } }, { to = "provider:pi", msg = { select = id } } }
end

return mod {
	subscribe = { "pilot/page", "pilot/thread" },

	styles = {
		[".side-head"] = { padding = { 12, 12, 8, 12 } },
		[".new-thread"] = { background = "card", border = true, radius = 10, padding = { 8, 12 }, gap = 10, margin = { 0, 12 } },
		[".new-thread:hover"] = { background = "list-hover" },
		[".side-page"] = { padding = { 7, 10 }, radius = 8, gap = 10, margin = { 0, 12 }, color = "foreground" },
		[".side-page:hover"] = { background = "list-hover" },
		[".side-page:selected"] = { background = "list-active" },
		[".side-page-icon"] = { color = "muted-foreground" },
		[".side-tools"] = { padding = { 16, 16, 4, 20 } },
		[".thread-heading"] = { padding = { 12, 20, 4, 20 }, size = "small", color = "muted-foreground", weight = "medium" },
		[".thread-row"] = { padding = { 7, 10, 7, 6 }, radius = 10, margin = { 0, 10 }, gap = 6, border = true, border_color = "#00000000" },
		[".thread-row:hover"] = { background = "list-hover" },
		[".thread-row:selected"] = { background = "card", border_color = "border" },
		[".thread-title"] = { weight = "normal" },
		[".thread-line"] = { size = "small", color = "muted-foreground" },
		[".thread-branch"] = { color = "muted-foreground", size = "small" },
		[".thread-spinner"] = { color = "success" },
		[".thread-waiting"] = { color = "warning" },
		[".thread-failed"] = { color = "danger" },
		[".thread-dot"] = { width = 8, height = 8, radius = 4, background = "accent" },
		[".rename-field"] = { background = "card", border = true, radius = 8, padding = { 8, 10 } },
		[".side-empty"] = { padding = { 10, 20 }, size = "small", color = "muted-foreground" },
	},

	init = function(previous: State?): State
		local state: State = previous or { page = "threads", filter_open = false, show_archived = false, name = "", recent = {}, recent_at = 1 }
		state.filter_open = false
		state.menu = nil
		state.renaming = nil
		state.deleting = nil
		return state
	end,

	update = function(state: State, msg: any)
		if msg == "toggle-filter" then
			state.filter_open = not state.filter_open
		elseif msg == "close-filter" then
			state.filter_open = false
		elseif msg == "close-menu" then
			state.menu = nil
		elseif msg == "add-folder" then
			return state, { { to = "app:pick", msg = { kind = "folder", prompt = "Add project", reply = { folder = true } } } }
		elseif msg == "cancel-delete" then
			state.deleting = nil
		elseif msg == "confirm-delete" then
			local doomed = state.deleting
			state.deleting = nil
			if doomed then
				return state, { { to = "provider:pi", msg = { delete_thread = doomed.id } } }
			end
		elseif msg == "cancel-rename" then
			state.renaming = nil
		elseif msg == "save-rename" then
			local id = state.renaming
			state.renaming = nil
			if id and state.name ~= "" then
				return state, { { to = "provider:pi", msg = { rename = { id = id, title = state.name } } } }
			end
		elseif msg == "next-recent" or msg == "previous-recent" then
			local count = #state.recent
			if count > 1 then
				state.recent_at = (state.recent_at + (if msg == "next-recent" then 0 else count - 2)) % count + 1
				return state, open_thread(state.recent[state.recent_at])
			end
		elseif type(msg) ~= "table" then
			return
		elseif msg.topic == "pilot/page" then
			state.page = msg.value
		elseif msg.topic == "pilot/thread" then
			-- Most recent first, for Ctrl-Tab.
			local selected = msg.value and msg.value.id
			if selected and state.recent[state.recent_at] ~= selected then
				local found = table.find(state.recent, selected)
				if found then
					table.remove(state.recent, found)
				end
				table.insert(state.recent, 1, selected)
				while #state.recent > 20 do
					table.remove(state.recent)
				end
				state.recent_at = 1
			end
		elseif msg.nav then
			return state, { { to = "shell", msg = { page = msg.nav } } }
		elseif msg.open then
			return state, open_thread(msg.open)
		elseif msg.menu then
			state.menu = msg.menu
		elseif msg.msg == "rename-text" then
			state.name = msg.value
		elseif msg.folder then
			if msg.paths and msg.paths[1] then
				return state, { { to = "provider:pi", msg = { add_workspace = msg.paths[1] } } }
			end
		elseif msg.pick then
			state.filter_open = false
			state.menu = nil
			local action, id = string.match(msg.pick, "^(%a+):(.*)$")
			if msg.pick == "group-time" or msg.pick == "group-workspace" then
				return state, { { to = "provider:pi", msg = { set_pref = { group = string.sub(msg.pick, 7) } } } }
			elseif msg.pick == "show-archived" then
				state.show_archived = not state.show_archived
			elseif action == "pin" or action == "archive" then
				local thread = nil
				for _, row in (latest and latest.threads) or {} do
					if row.id == id then
						thread = row
					end
				end
				local on = thread == nil or not (if action == "pin" then thread.pinned else thread.archived)
				return state, { { to = "provider:pi", msg = { [action] = { id = id, on = on } } } }
			elseif action == "rename" then
				state.renaming = id
				state.name = ""
			elseif action == "delete" then
				for _, row in (latest and latest.threads) or {} do
					if row.id == id then
						state.deleting = { id = row.id, title = row.title, worktree = row.branch ~= nil }
					end
				end
			end
		elseif msg.jump then
			local index = 0
			for _, group in grouped(latest, state) do
				for _, thread in group.rows do
					index += 1
					if index == msg.jump then
						return state, open_thread(thread.id)
					end
				end
			end
		end
	end,

	view = function(state: State, app: any)
		local pi = app.pi
		latest = pi
		local rows: { any } = {}
		if pi and pi.threads then
			rows = ui.memo("rows", { pi.threads_version, pi.selected, state.page, state.show_archived, state.menu, pi.now // 30, pi.prefs.group }, function()
				local built: { any } = {}
				for _, group in grouped(pi, state) do
					table.insert(built, heading(group.title))
					for _, thread in group.rows do
						table.insert(built, thread_row(thread, state, pi))
					end
				end
				return built
			end)
		end
		local nav = {}
		for _, page in PAGES do
			table.insert(nav, ui.row {
				class = "side-page",
				selected = state.page == page.id,
				align = "center",
				msg = { nav = page.id },
				ui.icon { page.icon :: any, class = "side-page-icon" },
				ui.text { page.label },
			})
		end
		local mac = app.window.platform == "macos" and not app.window.fullscreen
		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				drag = true,
				height = 52,
				shrink = false,
				align = "center",
				justify = "end",
				class = "side-head",
				mac and ui.row { width = 72 },
				ui.spacer(),
				ui.row { class = "quiet-button", msg = "toggle-sidebar", to = "shell", tooltip = "Hide sidebar", ui.icon { "panel-left" } },
			},
			ui.row { class = "new-thread", shrink = false, align = "center", msg = { nav = "new" }, ui.icon { "plus" }, ui.text { "New thread" } },
			ui.column { gap = "none", shrink = false, style = { padding = { 10, 0, 0, 0 } }, table.unpack(nav) },
			ui.row {
				class = "side-tools",
				shrink = false,
				align = "center",
				ui.text { "Threads", tone = "muted" },
				ui.spacer(),
				menu_button {
					icon = "sliders-horizontal",
					open = state.filter_open,
					toggle = "toggle-filter",
					dismiss = "close-filter",
					align = "end",
					tooltip = "Group and filter",
					items = filter_items(pi, state),
				},
				ui.row { class = "quiet-button", msg = "add-folder", tooltip = "Add a project folder", ui.icon { "folder-open" } },
			},
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column {
					shrink = false,
					gap = "none",
					style = { padding = { 0, 0, 12, 0 } },
					pi ~= nil and #rows == 0 and ui.text { "No threads yet.", class = "side-empty" },
					table.unpack(rows),
				},
			},
			dialog {
				open = state.deleting ~= nil,
				dismiss = "cancel-delete",
				title = "Delete this thread?",
				size = "small",
				plain = true,
				icon = "trash-2",
				tone = "danger",
				buttons = { { label = "Cancel", msg = "cancel-delete", variant = "ghost" }, { label = "Delete", msg = "confirm-delete", variant = "danger" } },
				state.deleting and ui.text {
					if state.deleting.worktree
						then `"{state.deleting.title}" and its pi session file go for good, and so does its worktree, with any changes not committed there.`
						else `"{state.deleting.title}" and its pi session file go for good. Archive it to keep it out of the way instead.`,
					wrap = true,
				},
			},
			dialog {
				open = state.renaming ~= nil,
				dismiss = "cancel-rename",
				title = "Rename thread",
				size = "small",
				submit = "save-rename",
				buttons = { { label = "Cancel", msg = "cancel-rename", variant = "ghost" }, { label = "Rename", msg = "save-rename", variant = "primary" } },
				ui.row { class = "rename-field", ui.input { id = "rename", grow = true, focus = true, value = state.name, change = "rename-text", submit = "save-rename", placeholder = "A name for this thread" } },
			},
		}
	end,
}
