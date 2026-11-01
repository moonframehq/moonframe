-- The sidebar: New, Search and the places, then the sessions grouped by
-- repository (code mode) or the recent chats (chat mode), and who is
-- signed in. Groups fold from their heading; Search turns its row into a
-- field that filters the sessions as you type.
--
-- Sessions come from the history provider (`app.history`): your coding-agent
-- CLI sessions, read-only, or samples when there are none. Each row says
-- whether the session opened a pull request, ran on this computer, or ran
-- in a folder that is gone. The chats are sample data: Cinder has no chat
-- history to read.

type State = {
	folded: { [string]: boolean },
	searching: boolean,
	query: string,
	mode: string,
	open: string,
}

local PLACES = {
	{ id = "projects", label = "Projects", icon = "layers", beta = true },
	{ id = "artifacts", label = "Artifacts", icon = "copy", beta = false },
	{ id = "customize", label = "Customize", icon = "palette", beta = false },
	{ id = "more", label = "More", icon = "ellipsis", beta = false },
}

-- Sample chats for chat mode.
local CHATS = {
	"Plan a three-day trip to Lisbon",
	"Explain CRDTs like I'm new to them",
	"Rewrite this cover letter, warmer",
	"Sourdough starter smells like acetone",
	"Compare two standing desks",
	"Name ideas for a ceramics studio",
	"What is a monad, practically?",
}

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
		[".em-beta"] = { border = true, border_color = "#5a5a5b", radius = 10, padding = { 1, 7 }, color = "muted-foreground", size = "small" },
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
		[".em-foot"] = { height = 52, padding = { 0, 14, 0, 18 }, gap = 8 },
		[".em-avatar"] = { background = "#3a3a3b", radius = 10, color = "#d4d4d5", size = "small" },
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

		-- New, Search, the places.
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
		for _, place in PLACES do
			table.insert(nav, ui.row {
				class = "em-row",
				align = "center",
				tooltip = "Not in this example",
				ui.icon { name = place.icon :: any, class = "em-row-icon" },
				ui.text { place.label },
				place.beta and ui.text { "Beta", class = "em-beta" },
			})
		end

		-- The sessions by repository, or the chats.
		local rows = {}
		if not code then
			table.insert(rows, ui.row {
				class = "em-group em-group-first",
				align = "center",
				ui.text { "Recents", class = "em-group-name" },
			})
			local shown = 0
			for _, chat in CHATS do
				if matches(chat, state.query) then
					shown += 1
					table.insert(rows, ui.row {
						class = "em-session",
						align = "center",
						tooltip = "Sample chat",
						ui.row { grow = true, ui.text { chat, truncate = true } },
					})
				end
			end
			if shown == 0 then
				table.insert(rows, ui.text { "No chat matches.", class = "em-empty" })
			end
		elseif not history then
			table.insert(rows, ui.text { "Looking for sessions…", class = "em-empty" })
		else
			local first = true
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
				table.insert(rows, ui.text { "Sample sessions: no CLI sessions found.", class = "em-sample" })
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
				ui.row { class = "em-avatar", width = 20, height = 20, align = "center", justify = "center", ui.text "A" },
				ui.row {
					align = "center",
					gap = "small",
					tooltip = "Sample account",
					ui.text { "Alex", weight = "medium" },
					ui.text { "· Ultra", class = "em-plan" },
					ui.icon { name = "chevron-down", size = "small", class = "em-plan" },
				},
				ui.spacer(),
				ui.row { class = "em-tool", tooltip = "Report a bug", ui.icon "bug" },
			},
		}
	end,
}
