-- ⌘K over everything: threads, projects and actions; ⌘P for files; and the
-- model picker, which is too long a list for a menu. All one field and one
-- list, each mode a source of rows.

local key_hints = use "key-hints"
local kbd = use "kbd"
local file_icon = use "file-icon"

type State = { open: boolean, mode: string, filter: string, query: string, highlighted: number, serial: number, page: string }

local PLACEHOLDERS = {
	all = "Search chats, workspaces and actions",
	files = "Go to file",
	models = "Switch model",
}
local FILTERS = { { "all", "All" }, { "chats", "Chats" }, { "workspaces", "Workspaces" }, { "actions", "Actions" } }
local SHOWN = 10
local KEYS: any = { up = "up", down = "down", enter = "enter", escape = "close", tab = "next-filter" }

local ACTIONS: { any } = {
	{ label = "New thread", icon = "plus", keys = "cmd-n", to = "shell", msg = { page = "new" } },
	{ label = "Add project folder…", icon = "folder-open", keys = "cmd-o", to = "sidebar", msg = "add-folder" },
	{ label = "Go to file…", icon = "file-text", keys = "cmd-p", to = "palette", msg = { open = "files" } },
	{ label = "Find in thread", icon = "search", keys = "cmd-f", to = "conversation", msg = "find" },
	{ label = "Switch model…", icon = "layers", keys = "cmd-shift-m", to = "palette", msg = { open = "models" } },
	{ label = "Toggle terminal", icon = "square-terminal", keys = "cmd-j", to = "workbench", msg = { toggle_tab = "terminal" } },
	{ label = "Toggle changes", icon = "git-compare", keys = "cmd-r", to = "workbench", msg = { toggle_tab = "changes" } },
	{ label = "Toggle files", icon = "folder", keys = "cmd-e", to = "workbench", msg = { toggle_tab = "files" } },
	{ label = "Toggle workbench", icon = "panel-right", keys = "cmd-alt-b", to = "shell", msg = "toggle-workbench" },
	{ label = "Toggle sidebar", icon = "panel-left", keys = "cmd-b", to = "shell", msg = "toggle-sidebar" },
	{ label = "Compact context", icon = "minimize-2", to = "provider:pi", msg = { prompt = "/compact" } },
	{ label = "Scheduled tasks", icon = "clock", to = "shell", msg = { page = "scheduled" } },
	{ label = "Skills", icon = "layers", to = "shell", msg = { page = "skills" } },
	{ label = "Extensions", icon = "plug", to = "shell", msg = { page = "extensions" } },
	{ label = "Settings", icon = "settings", keys = "cmd-,", to = "shell", msg = { page = "settings" } },
}

-- The rows the view last drew, for `update`, which has no `app`.
local rows_now: { any } = {}

local function fits(text: string, query: string): boolean
	if query == "" then
		return true
	end
	local lower = string.lower(text)
	local at = 1
	for index = 1, #query do
		local found = string.find(lower, string.sub(query, index, index), at, true)
		if not found then
			return false
		end
		at = found + 1
	end
	return true
end

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

local function collect(state: State, pi: any, repo: any): { any }
	local list: { any } = {}
	local query = string.lower(state.query)
	if state.mode == "files" then
		for _, path in (repo and repo.found and repo.found.results) or {} do
			table.insert(list, { label = string.match(path, "[^/]+$") or path, detail = path, file = true, to = "workbench", msg = { open_file = path } })
		end
		return list
	elseif state.mode == "models" then
		for _, model in (pi and pi.models) or {} do
			if fits(`{model.provider} {model.name} {model.id}`, query) then
				table.insert(list, { label = model.name, detail = model.provider, icon = "layers", to = "provider:pi", msg = { set_model = { provider = model.provider, id = model.id }, draft = state.page == "new" } })
			end
		end
		return list
	end
	local filter = state.filter
	if (filter == "all" or filter == "chats") and pi then
		local section = if query == "" then "Recents" else "Chats"
		for _, thread in pi.threads do
			if #list < 40 and not thread.archived and fits(`{thread.title} {thread.workspace}`, query) then
				table.insert(list, {
					section = section,
					label = thread.title,
					detail = `{thread.workspace} · {age(math.max(0, pi.now - thread.updated))}`,
					right = if thread.id == pi.selected then "Current" else nil,
					icon = "message-square",
					thread = thread.id,
				})
			end
		end
	end
	if (filter == "all" or filter == "workspaces") and pi then
		for _, workspace in pi.workspaces do
			if fits(workspace.name, query) then
				table.insert(list, { section = "Workspaces", label = workspace.name, detail = workspace.short, icon = "folder", workspace = workspace.path })
			end
		end
	end
	if filter == "all" or filter == "actions" then
		for _, action in ACTIONS do
			if fits(action.label, query) then
				table.insert(list, { section = "Actions", label = action.label, icon = action.icon, keys = action.keys, to = action.to, msg = action.msg })
			end
		end
	end
	return list
end

local function run(state: State, row: any): (State, { any })
	state.open = false
	if not row then
		return state, {}
	end
	if row.thread then
		return state, { { to = "shell", msg = { page = "threads" } }, { to = "provider:pi", msg = { select = row.thread } } }
	elseif row.workspace then
		return state, { { to = "topic:pilot/workspace", msg = row.workspace }, { to = "shell", msg = { page = "new" } } }
	elseif row.to == "palette" then
		state.open = true
		state.mode = row.msg.open
		state.query = ""
		state.highlighted = 1
		state.serial += 1
		return state, if state.mode == "files" then { { to = "provider:repo", msg = { find = "" } } } else {}
	end
	return state, { { to = row.to, msg = row.msg } }
end

local function row_view(row: any, index: number, highlighted: number): any
	return ui.row {
		class = "palette-row",
		selected = index == highlighted,
		align = "center",
		msg = { run = index },
		if row.file then file_icon { name = row.label } else ui.icon { row.icon :: any, class = "palette-icon" },
		ui.text { row.label, wrap = false },
		row.detail and ui.text { row.detail, truncate = true, class = "palette-detail" },
		ui.spacer(),
		row.right and ui.text { row.right, class = "palette-detail", wrap = false },
		row.keys and kbd { keys = row.keys, plain = true },
	}
end

return mod {
	styles = {
		[".palette"] = { gap = 0, padding = 0 },
		[".palette-search"] = { padding = { 14, 18 }, gap = 10, border = "bottom" },
		[".palette-icon"] = { color = "muted-foreground" },
		[".palette-input"] = { size = "large" },
		[".palette-filters"] = { padding = { 10, 16, 4, 16 }, gap = 6 },
		[".palette-filter"] = { padding = { 4, 11 }, radius = 14, border = true, color = "muted-foreground" },
		[".palette-filter:hover"] = { background = "list-hover" },
		[".palette-filter:selected"] = { background = "list-active", color = "foreground", border_color = "#00000000" },
		[".palette-list"] = { padding = { 6, 8, 8, 8 }, gap = 0 },
		[".palette-heading"] = { padding = { 8, 8, 4, 8 }, size = "small", color = "muted-foreground", weight = "medium" },
		[".palette-row"] = { padding = { 8, 10 }, radius = 8, gap = 10 },
		[".palette-row:hover"] = { background = "list-hover" },
		[".palette-row:selected"] = { background = "list-active" },
		[".palette-detail"] = { color = "muted-foreground", size = "small" },
		[".palette-empty"] = { padding = { 20, 10 }, color = "muted-foreground" },
		[".palette-foot"] = { padding = { 9, 16 }, border = "top" },
	},

	subscribe = { "pilot/page" },

	init = function(previous: State?): State
		return { open = false, mode = "all", filter = "all", query = "", highlighted = 1, serial = if previous then previous.serial else 0, page = "threads" }
	end,

	update = function(state: State, msg: any)
		if msg == "close" then
			state.open = false
		elseif msg == "up" or msg == "down" then
			local count = math.max(#rows_now, 1)
			state.highlighted = (state.highlighted - 1 + (if msg == "down" then 1 else count - 1)) % count + 1
		elseif msg == "enter" or (type(msg) == "table" and msg.msg == "enter") then
			return run(state, rows_now[state.highlighted])
		elseif msg == "next-filter" then
			for index, filter in FILTERS do
				if filter[1] == state.filter then
					state.filter = FILTERS[index % #FILTERS + 1][1]
					state.highlighted = 1
					break
				end
			end
		elseif type(msg) ~= "table" then
			return
		elseif msg.topic == "pilot/page" then
			state.page = msg.value
		elseif msg.open then
			state.open = true
			state.mode = msg.open
			state.filter = "all"
			state.query = ""
			state.highlighted = 1
			state.serial += 1
			if msg.open == "files" then
				return state, { { to = "provider:repo", msg = { find = "" } } }
			end
		elseif msg.msg == "query" then
			state.query = msg.value
			state.highlighted = 1
			if state.mode == "files" then
				return state, { { to = "provider:repo", msg = { find = msg.value } } }
			end
		elseif msg.filter then
			state.filter = msg.filter
			state.highlighted = 1
		elseif msg.run then
			return run(state, rows_now[msg.run])
		end
	end,

	view = function(state: State, app: any)
		if not state.open then
			rows_now = {}
			return ui.column { width = 0, height = 0 }
		end
		rows_now = collect(state, app.pi, app.repo)
		-- A window of rows around the highlight keeps it in sight.
		local first = math.max(1, math.min(state.highlighted - SHOWN + 1, #rows_now - SHOWN + 1))
		if state.highlighted <= SHOWN then
			first = 1
		end
		local shown: { any } = {}
		local section: string? = nil
		for index = first, math.min(first + SHOWN - 1, #rows_now) do
			local row = rows_now[index]
			if row.section and row.section ~= section then
				section = row.section
				table.insert(shown, ui.text { row.section, class = "palette-heading" })
			end
			table.insert(shown, row_view(row, index, state.highlighted))
		end
		if #rows_now == 0 then
			table.insert(shown, ui.text { "Nothing matches.", class = "palette-empty" })
		end
		local filters: { any } = {}
		if state.mode == "all" then
			for _, filter in FILTERS do
				table.insert(filters, ui.row { class = "palette-filter", selected = state.filter == filter[1], msg = { filter = filter[1] }, ui.text { filter[2] } })
			end
		end
		return ui.overlay {
			open = true,
			dismiss = "close",
			place = "top",
			backdrop = true,
			width = 680,
			keys = KEYS,
			content = ui.column {
				class = "palette",
				ui.row {
					class = "palette-search",
					align = "center",
					ui.icon { "search", class = "palette-icon" },
					ui.input {
						id = `palette-{state.serial}`,
						class = "palette-input",
						grow = true,
						focus = true,
						value = state.query,
						placeholder = PLACEHOLDERS[state.mode],
						change = "query",
						keys = KEYS,
					},
				},
				#filters > 0 and ui.row { class = "palette-filters", table.unpack(filters) },
				ui.column { class = "palette-list", table.unpack(shown) },
				ui.row {
					class = "palette-foot",
					key_hints {
						caps = true,
						separator = "",
						hints = { { keys = "up down", label = "Navigate" }, { keys = "enter", label = "Open" }, { keys = "tab", label = "Filters" }, { keys = "escape", label = "Close" } },
					},
				},
			},
		}
	end,
}
