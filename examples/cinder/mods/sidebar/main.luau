-- The sidebar: New and Search, then the sessions: those started
-- here with an agent, then the CLI sessions grouped by repository (code
-- mode; chat mode has none yet), and the agent new sessions get. Groups
-- fold from their heading; Search turns its row into a field that filters
-- the sessions as you type.
--
-- Sessions started here come from the acp provider (`app.acp`, the runs
-- the canvas started, each follow-up under its first); the others from the
-- history provider (`app.history`): your coding-agent CLI sessions,
-- read-only. Each CLI row says whether the session opened a pull request,
-- ran on this computer, or ran in a folder that is gone.

type State = {
	folded: { [string]: boolean },
	searching: boolean,
	query: string,
	mode: string,
	open: string,
}

-- A session started here: its first run, and how its newest is going.
type Started = { id: string, title: string, status: string }

local STATUS_ICONS = {
	starting = { icon = "loader-circle", spin = true, tip = "Starting" },
	running = { icon = "loader-circle", spin = true, tip = "Working" },
	waiting = { icon = "circle-alert", class = "em-asks", tip = "Needs your answer" },
	failed = { icon = "circle-x", tip = "Failed" },
}

-- The sessions started here, newest first.
local function started(acp: any): { Started }
	local list: { Started } = {}
	local index: { [string]: Started } = {}
	-- Runs come newest first, so a session's newest run is met first.
	local newest: { [string]: string } = {}
	for _, run in (acp and acp.runs) or {} do
		if run.owner == "canvas" then
			local first = run.thread or run.id
			if not newest[first] then
				newest[first] = run.status
			end
			if not run.thread then
				local entry = { id = run.id, title = run.text, status = "" }
				index[run.id] = entry
				table.insert(list, entry)
			end
		end
	end
	for id, entry in index do
		entry.status = newest[id] or ""
	end
	return list
end

-- The agent a new session gets by default, and whether it can run.
local function agent_line(acp: any): string
	if acp == nil then
		return "Looking for agents…"
	end
	local usable = 0
	local default = nil
	for _, agent in acp.agents or {} do
		if agent.installed or agent.fetched then
			usable += 1
			if agent.name == acp.agent then
				default = agent
			end
		end
	end
	if usable == 0 then
		return "No coding agent installed"
	end
	if default then
		return if default.installed then default.title else `{default.title} (via npx)`
	end
	return `{usable} agent{if usable == 1 then "" else "s"} installed`
end

local function matches(title: string, query: string): boolean
	if query == "" then
		return true
	end
	return string.find(string.lower(title), string.lower(query), 1, true) ~= nil
end

return mod {
	title = "Sidebar",
	subscribe = { "cinder/nav" },

	styles = {
		[".em-nav"] = { padding = { 4, 8, 0, 8 }, gap = 0 },
		[".em-row"] = { height = 34, padding = { 0, 10 }, gap = 10, radius = 8 },
		[".em-row:hover"] = { background = "list-hover" },
		[".em-row:selected"] = { background = "list-active" },
		[".em-new"] = { background = "em-raised" },
		[".em-new:hover"] = { background = "#4e4e4f" },
		[".em-row-icon"] = { color = "#d4d4d5" },
		[".em-list"] = { padding = { 0, 8, 12, 8 } },
		[".em-group"] = { height = 34, padding = { 0, 10 }, gap = 6, margin = { top = 18 }, radius = 6, color = "muted-foreground" },
		[".em-group:hover"] = { color = "foreground" },
		[".em-group-first"] = { margin = { top = 19 } },
		[".em-group-name"] = { size = "small" },
		[".em-caret"] = { color = "muted-foreground" },
		[".em-session"] = { height = 34, padding = { 0, 13, 0, 10 }, gap = 12, radius = 8 },
		[".em-session:hover"] = { background = "list-hover" },
		[".em-session:selected"] = { background = "list-active" },
		[".em-status"] = { color = "em-faint" },
		[".em-pr"] = { color = "success" },
		[".em-empty"] = { padding = { 10, 10 }, color = "muted-foreground", size = "small" },
		[".em-sample"] = { padding = { 2, 10, 6, 10 }, color = "em-faint", size = "small" },
		[".em-asks"] = { color = "#f2a65a" },
		[".em-foot"] = { height = 52, padding = { 0, 14, 0, 18 }, gap = 8 },
		[".em-plan"] = { color = "muted-foreground", size = "small" },
		[".em-search"] = { background = "list-active" },
	},

	init = function(previous: State?): State
		return {
			folded = (previous and previous.folded) or {},
			searching = false,
			query = "",
			mode = "code",
			open = "",
		}
	end,

	update = function(state: State, msg)
		if msg == "search" then
			state.searching = true
		elseif msg == "close-search" then
			state.searching = false
			state.query = ""
		elseif type(msg) == "table" then
			if msg.topic == "cinder/nav" and msg.value then
				state.mode = msg.value.mode or "code"
				state.open = msg.value.open or ""
			elseif msg.msg == "query" then
				state.query = msg.value or ""
			elseif msg.fold then
				state.folded[msg.fold] = not state.folded[msg.fold]
			end
		end
	end,

	view = function(state: State, app)
		local history = app.history
		local code = state.mode == "code"

		-- New and Search.
		local nav = {}
		table.insert(nav, ui.row {
			class = "em-row em-new",
			align = "center",
			msg = "new",
			to = "shell",
			tooltip = "⌘N",
			ui.icon { name = "plus", class = "em-row-icon" },
			ui.text { if code then "New" else "New chat" },
		})
		if state.searching then
			table.insert(nav, ui.row {
				class = "em-row em-search",
				align = "center",
				ui.icon { name = "search", class = "em-row-icon" },
				ui.input {
					id = "search",
					class = "em-input",
					grow = true,
					focus = true,
					value = state.query,
					placeholder = if code then "Search sessions" else "Search chats",
					change = "query",
					keys = { escape = "close-search" },
				},
				ui.row { class = "em-tool", msg = "close-search", tooltip = "Close search", ui.icon { name = "close", size = "small" } },
			})
		else
			table.insert(nav, ui.row {
				class = "em-row",
				align = "center",
				msg = "search",
				tooltip = "⌘K",
				ui.icon { name = "search", class = "em-row-icon" },
				ui.text "Search",
			})
		end

		-- The sessions by repository, or the chats.
		local rows = {}
		local first = true
		if not code then
			table.insert(rows, ui.text { "Chats are not in Cinder yet. Code mode runs an agent in a folder.", class = "em-empty", wrap = true })
		else
			-- The sessions started here.
			local mine = {}
			for _, session in started(app.acp) do
				if matches(session.title, state.query) then
					table.insert(mine, session)
				end
			end
			if #mine > 0 then
				local folded = state.folded["cinder:started"] == true and state.query == ""
				table.insert(rows, ui.row {
					class = "em-group em-group-first",
					align = "center",
					self_align = "start",
					msg = { fold = "cinder:started" },
					ui.text { "Started in Cinder", class = "em-group-name" },
					ui.icon { name = if folded then "chevron-right" else "chevron-down", size = "small", class = "em-caret" },
				})
				first = false
				if not folded then
					for _, session in mine do
						local look = STATUS_ICONS[session.status]
						table.insert(rows, ui.row {
							class = "em-session",
							align = "center",
							selected = "run:" .. session.id == state.open,
							msg = { open = "run:" .. session.id },
							to = "shell",
							tooltip = session.title,
							ui.row { grow = true, ui.text { session.title, truncate = true } },
							if look
								then ui.icon { name = look.icon :: any, class = look.class or "em-status", spin = look.spin, tooltip = look.tip }
								else ui.icon { name = "bot", class = "em-status", tooltip = "An agent session" },
						})
					end
				end
			end
		end
		if code and not history then
			table.insert(rows, ui.text { "Looking for sessions…", class = "em-empty" })
		elseif code then
			for _, group in history.groups or {} do
				local sessions = {}
				for _, session in group.sessions do
					if matches(session.title, state.query) or matches(group.name, state.query) then
						table.insert(sessions, session)
					end
				end
				if #sessions > 0 then
					local folded = state.folded[group.name] == true and state.query == ""
					table.insert(rows, ui.row {
						class = if first then "em-group em-group-first" else "em-group",
						align = "center",
						self_align = "start",
						msg = { fold = group.name },
						tooltip = group.path,
						ui.text { group.name, class = "em-group-name" },
						ui.icon { name = "folder-git-2", size = "small", class = "em-caret" },
						ui.icon { name = if folded then "chevron-right" else "chevron-down", size = "small", class = "em-caret" },
					})
					first = false
					if not folded then
						for _, session in sessions do
							table.insert(rows, ui.row {
								class = "em-session",
								align = "center",
								selected = session.id == state.open,
								msg = { open = session.id },
								to = "shell",
								tooltip = (if history.source == "sample" then "Sample · " else "")
									.. (if session.branch ~= "" then `{session.title} · {session.branch}` else session.title),
								ui.row { grow = true, ui.text { session.title, truncate = true } },
								session.pr and ui.icon { name = "git-pull-request", class = "em-pr", tooltip = "Opened a pull request" },
								if session.gone
									then ui.icon { name = "cloud", class = "em-status", tooltip = "Its folder is gone" }
									else ui.icon { name = "laptop", class = "em-status", tooltip = "On this computer" },
							})
						end
					end
				end
			end
			if first then
				table.insert(rows, ui.text { if state.query ~= "" then "No session matches." else "No sessions yet.", class = "em-empty" })
			end
			if history.source == "sample" then
				table.insert(rows, ui.text { "Sample sessions (CINDER_SAMPLE is set).", class = "em-sample" })
			end
		end

		return ui.column {
			grow = true,
			gap = "none",
			ui.column { class = "em-nav", shrink = false, table.unpack(nav) },
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column { class = "em-list", gap = "none", shrink = false, table.unpack(rows) },
			},
			ui.row {
				class = "em-foot",
				align = "center",
				shrink = false,
				ui.icon { name = "bot", class = "em-plan" },
				ui.text { agent_line(app.acp), class = "em-plan", truncate = true },
			},
		}
	end,
}
