-- The sessions: Home, search, New session, then a row per session. A
-- session is a conversation with a coding agent in one folder: the chat's
-- runs of the `acp` provider that share its id as their `thread`, kept
-- across launches as the provider keeps them, plus the sessions started
-- here that have no run yet. This mod owns the list, with what the user did
-- to it (renames, deletions, what is unread), and shares it on
-- `cairn/sessions` as { current, list }, and which page shows on
-- `cairn/page`.

local menu = use "menu"
local search_field = use "search-field"

-- Folders offered for a new session, from the sessions and the projects.
local MAX_FOLDERS = 10

type Draft = { id: string, folder: string }

type Session = {
	id: string,
	title: string,
	folder: string,
	name: string,
	status: string,
	unread: boolean,
	runs: number,
	finished: number,
}

type State = {
	drafts: { Draft },
	renamed: { [string]: string },
	deleted: { [string]: boolean },
	seen: { [string]: number },
	unread: { [string]: boolean },
	current: string?,
	page: string,
	searching: boolean,
	query: string,
	next_id: number,
	picking: boolean,
	-- The chat's runs, newest first, from `provider/acp`; not kept.
	runs: { any },
}

-- The last part of a path: a folder's name.
local function base(path: string): string
	return string.match(path, "([^/\\]+)[/\\]*$") or path
end

local function going(status: string?): boolean
	return status == "starting" or status == "running" or status == "waiting"
end

-- A request as a title: its first line, cut short.
local function title_of(text: string): string
	local line = string.match(text, "^%s*([^\n]*)") or text
	if #line > 60 then
		line = string.sub(line, 1, 57) .. "…"
	end
	return if line == "" then "New session" else line
end

-- The sessions as they stand: those with no run yet first, then the rest
-- by their newest run.
local function sessions(state: State): { Session }
	local threads: { [string]: { any } } = {}
	local order: { string } = {}
	-- Runs come newest first, so a thread's first run seen is its newest.
	for _, run in state.runs do
		local id = run.thread or run.id
		if not threads[id] then
			threads[id] = {}
			table.insert(order, id)
		end
		table.insert(threads[id], run)
	end
	local list: { Session } = {}
	local function add(id: string, folder: string?, runs: { any })
		if state.deleted[id] then
			return
		end
		local newest, oldest = runs[1], runs[#runs]
		local finished = (newest and newest.finished) or 0
		local status = if newest then newest.status else "new"
		local unread = state.unread[id] == true
			or (newest ~= nil and not going(status) and finished > (state.seen[id] or 0))
		local where = folder or (oldest and oldest.cwd) or ""
		table.insert(list, {
			id = id,
			title = state.renamed[id] or (if oldest then title_of(oldest.text) else "New session"),
			folder = where,
			name = if where == "" then "?" else base(where),
			status = status,
			unread = unread,
			runs = #runs,
			finished = finished,
		})
	end
	for _, draft in state.drafts do
		if not threads[draft.id] then
			add(draft.id, draft.folder, {})
		end
	end
	for _, id in order do
		local folder = nil
		for _, draft in state.drafts do
			if draft.id == id then
				folder = draft.folder
			end
		end
		add(id, folder, threads[id])
	end
	return list
end

local function publish(state: State)
	local sends: { any } = {
		{ to = "topic:cairn/sessions", msg = { current = state.current, list = sessions(state) } },
		{ to = "topic:cairn/page", msg = state.page },
	}
	return sends
end

-- Marks what the open session shows as seen.
local function see(state: State)
	if state.page ~= "chat" or not state.current then
		return
	end
	for _, session in sessions(state) do
		if session.id == state.current then
			state.seen[session.id] = math.max(state.seen[session.id] or 0, session.finished)
		end
	end
end

local function select(state: State, id: string?)
	state.current = id
	state.page = if id then "chat" else "home"
	if id then
		state.unread[id] = nil
		see(state)
	end
end

local function new_session(state: State, folder: string)
	local id = `cairn-{state.next_id}`
	state.next_id += 1
	table.insert(state.drafts, 1, { id = id, folder = folder })
	state.picking = false
	select(state, id)
end

-- Where a new session may start: the folders of the sessions, then the
-- user's projects, each once.
local function folders(state: State, app): { any }
	local items: { any } = { { heading = "Start in" } }
	local listed: { [string]: boolean } = {}
	local function add(path: string?, detail: string?)
		if path and path ~= "" and not listed[path] and #items <= MAX_FOLDERS then
			listed[path] = true
			table.insert(items, { id = path, label = base(path), detail = detail, icon = "folder", msg = { folder = path } })
		end
	end
	for _, session in sessions(state) do
		add(session.folder, "recent")
	end
	local projects: { any } = (app.projects and app.projects.projects) or {}
	for _, project in projects do
		add(project.path, project.home)
	end
	table.insert(items, "separator")
	table.insert(items, { id = "choose", label = "Choose a folder…", icon = "folder-open", msg = "choose-folder" })
	return items
end

return mod {
	subscribe = { "provider/acp" },

	styles = {
		[".nav"] = { padding = { 5.5, 10 }, gap = 10, radius = 8, margin = { 0, 2 }, color = "#b4b4b4" },
		[".nav:hover"] = { background = "list-hover" },
		[".nav:selected"] = { background = "secondary", color = "foreground" },
		[".session"] = { padding = { 5.5, 10 }, gap = 10, radius = 8, margin = { 0, 2 }, color = "#9a9a9a" },
		[".session:hover"] = { background = "list-hover" },
		[".session:selected"] = { background = "secondary", color = "foreground" },
		[".session-title"] = { clip = true },
		[".fade"] = { gradient = "90deg, #0b0b0c00, #0b0b0c" },
		[".fade-on"] = { gradient = "90deg, #24242400, #242424" },
		[".unread"] = { background = "accent", radius = 999 },
		[".asking"] = { color = "warning" },
		[".search"] = { margin = { 2, 6 } },
		[".empty"] = { padding = { 8, 12 } },
	},

	init = function(previous: any)
		local state: State = previous or {
			drafts = {}, renamed = {}, deleted = {}, seen = {}, unread = {},
			page = "home", next_id = 1,
		} :: any
		-- Older state (the sample sessions) had none of these.
		state.drafts = state.drafts or {}
		state.seen = state.seen or {}
		state.unread = state.unread or {}
		state.renamed = state.renamed or {}
		state.deleted = state.deleted or {}
		state.next_id = state.next_id or 1
		state.searching = false
		state.query = ""
		state.picking = false
		state.runs = {}
		if state.current and not string.match(state.current, "^cairn%-") then
			state.current = nil
			state.page = "home"
		end
		return state, publish(state)
	end,

	update = function(state: State, msg)
		if msg == "home" then
			select(state, nil)
		elseif msg == "new-session" then
			state.picking = not state.picking
			return state, { { to = "shell", msg = { show_left = true } } }
		elseif msg == "close-picker" then
			state.picking = false
			return
		elseif msg == "choose-folder" then
			state.picking = false
			return state, { { to = "app:pick", msg = { kind = "folder", prompt = "Start a session in", reply = { chosen = true } } } }
		elseif msg == "search" then
			state.searching = not state.searching
			state.query = ""
			return
		elseif type(msg) ~= "table" then
			return
		elseif msg.msg == "query" then
			state.query = msg.value
			return
		elseif msg.msg == "close-search" then
			state.searching = false
			state.query = ""
			return
		elseif msg.topic == "provider/acp" then
			local runs = {}
			for _, run in (msg.value and msg.value.runs) or {} do
				if run.owner == "chat" then
					table.insert(runs, run)
					-- Never reuse an id the provider still keeps runs of.
					local number = tonumber(string.match(run.thread or "", "^cairn%-(%d+)$"))
					if number and number >= state.next_id then
						state.next_id = number + 1
					end
				end
			end
			state.runs = runs
			see(state)
		elseif msg.chosen then
			if msg.paths and msg.paths[1] then
				new_session(state, msg.paths[1])
			else
				return
			end
		elseif msg.folder then
			new_session(state, msg.folder)
		elseif msg.select then
			select(state, msg.select)
		elseif msg.rename then
			state.renamed[msg.rename] = msg.title
		elseif msg.mark_unread then
			state.unread[msg.mark_unread] = true
		elseif msg.delete then
			local id = msg.delete
			state.deleted[id] = true
			for index = #state.drafts, 1, -1 do
				if state.drafts[index].id == id then
					table.remove(state.drafts, index)
				end
			end
			local first = sessions(state)[1]
			select(state, first and first.id)
			local sends = publish(state)
			-- The chat started its runs, so only it may stop and forget them.
			table.insert(sends, { to = "chat", msg = { forget_session = id } })
			return state, sends
		else
			return
		end
		return state, publish(state)
	end,

	view = function(state: State, app)
		local query = string.lower(state.query)
		local rows = {}
		for _, session in sessions(state) do
			local haystack = string.lower(session.title .. " " .. session.name)
			if query == "" or string.find(haystack, query, 1, true) then
				local selected = state.page == "chat" and session.id == state.current
				local mark
				if session.status == "waiting" then
					mark = ui.icon { name = "circle-alert", class = "asking" }
				elseif going(session.status) then
					mark = ui.icon { name = "loader-circle", tone = "muted", spin = true }
				elseif session.unread and not selected then
					mark = ui.row { class = "unread", width = 7, height = 7, shrink = false }
				end
				table.insert(rows, ui.row {
					class = "session",
					key = session.id,
					selected = selected,
					align = "center",
					msg = { select = session.id },
					tooltip = `{session.title} · {session.folder}`,
					ui.row { class = "tile", width = 18, height = 18, shrink = false, align = "center", justify = "center",
						ui.text(string.upper(string.sub(session.name, 1, 1))) },
					ui.row { class = "session-title", grow = true, width = 0, align = "center",
						ui.text { session.title, wrap = false, weight = if selected then "medium" else nil },
						ui.row { class = if selected then "fade-on" else "fade", width = 26, float = "right" },
					},
					mark,
				})
			end
		end
		if #rows == 0 then
			local empty = if query ~= "" then "No sessions match" else "No sessions yet"
			table.insert(rows, ui.text { empty, tone = "muted", class = "empty" })
		end
		return ui.column { grow = true, gap = "none",
			ui.column { gap = "none", shrink = false,
				ui.row { class = "nav", selected = state.page == "home", align = "center", msg = "home",
					ui.icon "layout-grid", ui.text "Home" },
				ui.row { class = "nav", selected = state.searching, align = "center", msg = "search",
					ui.icon "search", ui.text "Search sessions" },
				state.searching and ui.column { class = "search", shrink = false,
					search_field { id = "sessions", value = state.query, icon = true, placeholder = "Search…", change = "query", cancel = { msg = "close-search" }, focus = true },
				},
				ui.popover {
					open = state.picking, dismiss = "close-picker", side = "bottom", align = "start", width = 260,
					content = menu { items = folders(state, app), max_height = 360 },
					ui.row { class = "nav", selected = state.picking, align = "center", msg = "new-session",
						ui.icon "square-pen", ui.text "New session" },
				},
			},
			ui.row { height = 13, shrink = false },
			ui.column { grow = true, scroll = true, gap = "none", table.unpack(rows) },
		}
	end,
}
