-- Which repository Hangar shows. Started outside one (from the Finder, the
-- app runs in `/`), it reopens the last one; with none it fills the window
-- with a way to pick one and the repositories opened before. File > Switch
-- Repository lists them again later. The git provider does the opening and
-- refuses a folder that is not in a git repository; the main shell mounts
-- this mod in place of the bays while there is none.

local dialog = use "dialog"

-- Repositories remembered, most recent first.
local MAX_RECENT = 10

type State = {
	-- The main checkouts opened, most recent first; kept across launches.
	recent: { string },
	-- Whether git watches a repository, as it last said.
	repository: boolean?,
	-- The main checkout on show.
	current: string?,
	-- The folder being opened, and what the last attempt refused.
	opening: string?,
	problem: string?,
	-- The last repository was tried once this launch.
	reopened: boolean,
	-- The switch dialog is open.
	choosing: boolean,
}

local function name_of(path: string): string
	return string.match(path, "([^/\\]+)[/\\]*$") or path
end

local function parent_of(path: string): string
	return string.match(path, "^(.*)[/\\][^/\\]+[/\\]*$") or ""
end

-- The main checkout: the first worktree git lists, unless it is bare (a
-- bare repository has no files to open), else the folder on show.
local function main_checkout(git: any): string?
	for _, worktree in git.worktrees or {} do
		if not worktree.bare then
			return worktree.path
		end
	end
	return git.root
end

-- Puts `path` first in the recent list; whether anything moved.
local function remember(state: State, path: string): boolean
	if state.recent[1] == path then
		return false
	end
	local found = table.find(state.recent, path)
	if found then
		table.remove(state.recent, found)
	end
	table.insert(state.recent, 1, path)
	while #state.recent > MAX_RECENT do
		table.remove(state.recent)
	end
	return true
end

local function forget(state: State, path: string)
	local found = table.find(state.recent, path)
	if found then
		table.remove(state.recent, found)
	end
end

local function open(state: State, path: string): { any }
	state.opening = path
	state.problem = nil
	return { { to = "provider:git", msg = { open = path, id = "open" } } }
end

local function pick(): { any }
	return { { to = "app:pick", msg = { kind = "folder", prompt = "Open Repository", reply = { picked = true } } } }
end

local function recent_rows(state: State): { any }
	local rows: { any } = {}
	for index, path in state.recent do
		table.insert(
			rows,
			ui.row {
				key = `recent-{index}`,
				class = "recent",
				align = "center",
				gap = "small",
				selected = path == state.current,
				msg = { reopen = path },
				context = { forget = path },
				tooltip = path,
				ui.icon { "folder-git-2", tone = "muted" },
				ui.column {
					grow = true,
					gap = "none",
					ui.text { name_of(path), weight = "medium", truncate = true },
					ui.text { parent_of(path), class = "recent-path", truncate = true },
				},
				state.opening == path and ui.icon { "loader-circle", spin = true, tone = "muted" } or nil,
			}
		)
	end
	return rows
end

-- The whole window while no repository is open.
local function welcome(state: State)
	local recent = recent_rows(state)
	return ui.column {
		grow = true,
		gap = "none",
		class = "welcome",
		-- The title bar is hidden: this strip moves the window.
		ui.row { drag = true, height = 44 },
		ui.column {
			grow = true,
			align = "center",
			justify = "center",
			ui.column {
				width = 440,
				gap = "normal",
				align = "center",
				ui.icon { "folder-git-2", size = "title", tone = "muted" },
				ui.text { "Open a repository", size = "large", weight = "semibold" },
				ui.text {
					"Agents side by side on one git repository, a worktree each.",
					tone = "muted",
				},
				ui.button {
					"Open Repository…",
					icon = "folder-open",
					variant = "primary",
					disabled = state.opening ~= nil,
					msg = "open",
				},
				state.problem and ui.text { state.problem, class = "repositories-problem" } or nil,
				#recent > 0 and ui.column {
					gap = "none",
					class = "recent-list",
					ui.text { "Recent", class = "recent-title" },
					table.unpack(recent),
				} or nil,
			},
		},
	}
end

return mod {
	title = "Repositories",
	subscribe = { "provider/git" },

	styles = {
		[".welcome"] = { background = "background" },
		[".recent-list"] = { width = 440, padding = { 12, 0, 0, 0 } },
		[".recent-title"] = { size = "small", color = "muted-foreground", padding = { 0, 10, 6, 10 } },
		[".recent"] = { padding = { 7, 10 }, radius = 8 },
		[".recent:hover"] = { background = "list-hover" },
		[".recent:selected"] = { background = "card" },
		[".recent-path"] = { size = "small", color = "muted-foreground" },
		[".repositories-problem"] = { size = "small", color = "danger" },
	},

	init = function(previous: any): State
		return {
			recent = previous and previous.recent or {},
			reopened = false,
			choosing = false,
		}
	end,

	update = function(state: State, msg: any)
		if msg == "open" then
			return state, pick()
		elseif msg == "switch" then
			state.choosing = true
			state.problem = nil
			return state
		elseif msg == "dismiss" then
			state.choosing = false
			return state
		elseif type(msg) ~= "table" then
			return
		end
		if msg.topic == "provider/git" then
			local git = msg.value
			if type(git) ~= "table" then
				return
			end
			if git.repository then
				local main = main_checkout(git)
				local moved = state.repository ~= true or state.current ~= main
				state.repository = true
				state.current = main
				if main and remember(state, main) or moved then
					return state
				end
				return
			end
			state.repository = false
			state.current = nil
			-- Started where there is none: back to the last one, once.
			local last = state.recent[1]
			if not state.reopened and last and not state.opening then
				state.reopened = true
				return state, open(state, last)
			end
			return state
		elseif msg.picked then
			local path = msg.paths and msg.paths[1]
			if msg.cancelled or not path then
				return
			end
			return state, open(state, path)
		elseif msg.reopen then
			if state.opening then
				return
			end
			if msg.reopen == state.current then
				state.choosing = false
				return state
			end
			return state, open(state, msg.reopen)
		elseif msg.forget then
			forget(state, msg.forget)
			return state
		elseif msg.reply == "open" then
			local asked = state.opening
			state.opening = nil
			if msg.ok then
				state.choosing = false
				state.problem = nil
				-- It joins the list once git publishes it, by its main
				-- checkout: the folder picked may be a linked worktree.
			else
				state.problem = msg.error
				-- A folder gone or no longer a repository leaves the list.
				if asked and string.find(msg.error or "", " is not ", 1, true) then
					forget(state, asked)
				end
			end
			return state
		end
		return
	end,

	view = function(state: State, app)
		local git = app.git
		if git and not git.repository then
			return welcome(state)
		end
		local recent = recent_rows(state)
		return dialog {
			open = state.choosing,
			dismiss = "dismiss",
			title = "Switch Repository",
			icon = "folder-git-2",
			buttons = {
				{ label = "Open Other…", msg = "open", variant = "outline" },
			},
			state.problem and ui.text { state.problem, class = "repositories-problem" } or nil,
			#recent > 0 and ui.column { gap = "none", table.unpack(recent) }
				or ui.text { "No repositories yet.", tone = "muted" },
		}
	end,
}
