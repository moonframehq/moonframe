-- The sessions: Home, search, New session, then a row per session. It owns
-- the list: the sample sessions (from `provider/sessions`) with what the user
-- did to them on top, shared on `opencode/sessions` as { current, list }, and
-- which page shows on `opencode/page`.

local search_field = use "search-field"

-- What the scripted agent charges, to keep the counters moving.
local COST_PER_TOKEN = 0.000008

type Session = { id: string, title: string, letter: string, project: string, unread: boolean, tokens: number, cost: number }

type State = {
	base: { Session },
	created: { Session },
	renamed: { [string]: string },
	deleted: { [string]: boolean },
	read: { [string]: boolean },
	unread: { [string]: boolean },
	spent: { [string]: number },
	current: string?,
	page: string,
	searching: boolean,
	query: string,
	next_id: number,
}

-- The sessions as they stand: new ones first, then the samples.
local function sessions(state: State): { Session }
	local list: { Session } = {}
	local function add(session: Session)
		if not state.deleted[session.id] then
			local unread = state.unread[session.id] or (session.unread and not state.read[session.id])
			local spent = state.spent[session.id] or 0
			table.insert(list, {
				id = session.id, letter = session.letter, project = session.project,
				title = state.renamed[session.id] or session.title,
				unread = unread == true, tokens = session.tokens + spent, cost = session.cost + spent * COST_PER_TOKEN,
			})
		end
	end
	for _, session in state.created do
		add(session)
	end
	for _, session in state.base do
		add(session)
	end
	return list
end

local function publish(state: State)
	local sends: { any } = {
		{ to = "topic:opencode/sessions", msg = { current = state.current, list = sessions(state) } },
		{ to = "topic:opencode/page", msg = state.page },
	}
	return sends
end

local function select(state: State, id: string?)
	state.current = id
	state.page = if id then "chat" else "home"
	if id then
		state.read[id] = true
		state.unread[id] = nil
	end
end

return mod {
	subscribe = { "provider/sessions" },

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
		[".search"] = { margin = { 2, 6 } },
		[".empty"] = { padding = { 8, 12 } },
	},

	init = function(previous: State?)
		local state: State = previous or {
			base = {}, created = {}, renamed = {}, deleted = {}, read = {}, unread = {}, spent = {},
			current = "s2", page = "chat", searching = false, query = "", next_id = 1,
		}
		state.spent = state.spent or {}
		state.searching = false
		state.query = ""
		return state, publish(state)
	end,

	update = function(state: State, msg)
		if msg == "home" then
			select(state, nil)
		elseif msg == "new-session" then
			local id = `new-{state.next_id}`
			state.next_id += 1
			table.insert(state.created, 1, { id = id, title = "New session", letter = "L", project = "lumen", unread = false, tokens = 0, cost = 0 })
			select(state, id)
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
		elseif msg.topic == "provider/sessions" then
			local base: { Session } = {}
			for _, session in (msg.value and msg.value.sessions) or {} do
				table.insert(base, {
					id = session.id, title = session.title, letter = session.letter, project = session.project,
					unread = session.unread, tokens = session.tokens, cost = session.cost,
				})
			end
			state.base = base
		elseif msg.select then
			select(state, msg.select)
		elseif msg.rename then
			state.renamed[msg.rename] = msg.title
		elseif msg.spent then
			state.spent[msg.spent] = (state.spent[msg.spent] or 0) + msg.tokens
		elseif msg.mark_unread then
			state.unread[msg.mark_unread] = true
			state.read[msg.mark_unread] = nil
		elseif msg.delete then
			state.deleted[msg.delete] = true
			local first = sessions(state)[1]
			select(state, first and first.id)
		else
			return
		end
		return state, publish(state)
	end,

	view = function(state: State)
		local query = string.lower(state.query)
		local rows = {}
		for _, session in sessions(state) do
			if query == "" or string.find(string.lower(session.title), query, 1, true) then
				local selected = state.page == "chat" and session.id == state.current
				table.insert(rows, ui.row {
					class = "session",
					key = session.id,
					selected = selected,
					align = "center",
					msg = { select = session.id },
					tooltip = session.title,
					ui.row { class = "tile", width = 18, height = 18, shrink = false, align = "center", justify = "center", ui.text(session.letter) },
					ui.row { class = "session-title", grow = true, width = 0, align = "center",
						ui.text { session.title, wrap = false, weight = if selected then "medium" else nil },
						ui.row { class = if selected then "fade-on" else "fade", width = 26, float = "right" },
					},
					session.unread and ui.row { class = "unread", width = 7, height = 7, shrink = false },
				})
			end
		end
		if #rows == 0 then
			table.insert(rows, ui.text { "No sessions match", tone = "muted", class = "empty" })
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
				ui.row { class = "nav", align = "center", msg = "new-session",
					ui.icon "square-pen", ui.text "New session" },
			},
			ui.row { height = 13, shrink = false },
			ui.column { grow = true, scroll = true, gap = "none", table.unpack(rows) },
		}
	end,
}
