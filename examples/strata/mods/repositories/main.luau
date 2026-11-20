-- The repositories: those opened lately, then every one the projects
-- provider finds under your project folders, by folder. A click opens it
-- through the git provider, which refuses a folder that is not in a git
-- repository. With none open (started from the Finder, in `/`), it reopens
-- the last one, or fills the window with a way to pick one.

local search_field = use "search-field"
local sidebar = use "sidebar"
local icon_button = use "icon-button"

-- Repositories remembered, most recent first.
local MAX_RECENT = 6

type State = {
	-- Top folders opened, most recent first; kept across launches.
	recent: { string },
	query: string,
	-- Sections folded shut, by id.
	folded: { [string]: boolean? },
	-- The folder being opened, and what the last attempt refused.
	opening: string?,
	problem: string?,
	-- The last repository was tried once this launch.
	reopened: boolean,
}

local function name_of(path: string): string
	return string.match(path, "([^/\\]+)[/\\]*$") or path
end

local function parent_of(path: string): string
	return string.match(path, "^(.*)[/\\][^/\\]+[/\\]*$") or ""
end

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

local function matches(query: string, ...: string?): boolean
	if query == "" then
		return true
	end
	for index = 1, select("#", ...) do
		local text = select(index, ...)
		if text and string.find(string.lower(text), query, 1, true) then
			return true
		end
	end
	return false
end

-- One row: the name over its branch, and how many files changed.
local function row(state: State, path: string, branch: string?, changed: number?, current: boolean)
	return ui.row {
		class = if state.opening == path then "repo repo-opening" else "repo",
		gap = "small",
		align = "center",
		selected = current,
		tooltip = path,
		msg = { reopen = path },
		context = { forget = path },
		ui.column {
			grow = true,
			gap = "none",
			ui.text { name_of(path), weight = "medium", truncate = true },
			branch and ui.text { branch, class = "repo-branch", truncate = true } or nil,
		},
		changed and changed > 0 and ui.badge { tostring(changed), tooltip = `{changed} files changed` } or nil,
	}
end

-- The whole window while no repository is open.
local function welcome(state: State)
	local recent: { any } = {}
	for _, path in state.recent do
		table.insert(recent, ui.item { name_of(path), detail = parent_of(path), msg = { reopen = path } })
	end
	return ui.column {
		grow = true,
		gap = "none",
		-- The title bar is hidden: this strip moves the window.
		ui.row { drag = true, height = 44 },
		ui.column {
			grow = true,
			align = "center",
			justify = "center",
			ui.column {
				width = 420,
				gap = "normal",
				align = "center",
				ui.icon { "layers", size = "title", tone = "muted" },
				ui.text { "Open a repository", size = "large", weight = "semibold" },
				ui.text { "Browse its history, branch by branch, commit by commit.", tone = "muted" },
				ui.button {
					"Open Repository…",
					icon = "folder-open",
					variant = "primary",
					disabled = state.opening ~= nil,
					msg = "open",
				},
				state.problem and ui.text { state.problem, class = "repos-problem" } or nil,
				#recent > 0 and ui.column { width = 420, gap = "none", table.unpack(recent) } or nil,
			},
		},
	}
end

return mod {
	title = "Repositories",
	subscribe = { "provider/git" },

	styles = {
		[".repos-problem"] = { size = "small", color = "danger" },
		[".repo"] = { padding = { 5, 10 }, radius = 6 },
		[".repo:hover"] = { background = "list-hover" },
		[".repo:selected"] = { background = "list-active" },
		[".repo-branch"] = { size = "small", color = "muted-foreground" },
		[".repo-opening"] = { opacity = 0.6 },
	},

	init = function(previous: any): State
		return {
			recent = previous and previous.recent or {},
			query = "",
			folded = previous and previous.folded or {},
			reopened = false,
		}
	end,

	update = function(state: State, msg: any)
		if msg == "open" then
			return state, pick()
		elseif type(msg) ~= "table" then
			return
		end
		if msg.topic == "provider/git" then
			local git = msg.value
			if type(git) ~= "table" then
				return
			end
			if git.repository then
				if git.root and remember(state, git.root) then
					return state
				end
				return
			end
			-- Started where there is none: back to the last one, once.
			local last = state.recent[1]
			if not state.reopened and last and not state.opening then
				state.reopened = true
				return state, open(state, last)
			end
			return
		elseif msg.msg == "query" then
			state.query = string.lower(msg.value or "")
			return state
		elseif msg.toggle then
			state.folded[msg.toggle] = not state.folded[msg.toggle] or nil
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
			return state, open(state, msg.reopen)
		elseif msg.forget then
			forget(state, msg.forget)
			return state
		elseif msg.reply == "open" then
			local asked = state.opening
			state.opening = nil
			if msg.ok then
				state.problem = nil
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
		local root = git and git.root
		local projects = app.projects
		local query = state.query
		-- Projects by their folder, in the order the provider lists them
		-- (most recently active first), and by path to find a recent one.
		local by_path: { [string]: any } = {}
		local groups: { { title: string, rows: { any } } } = {}
		local group_of: { [string]: number } = {}
		for _, project in (projects and projects.projects) or {} do
			by_path[project.path] = project
		end
		local recent_rows: { any } = {}
		for _, path in state.recent do
			local project = by_path[path]
			if matches(query, name_of(path), project and project.branch) then
				local branch = if project then project.branch elseif path == root and git then git.branch else nil
				table.insert(recent_rows, row(state, path, branch, project and project.files, path == root))
			end
		end
		for _, project in (projects and projects.projects) or {} do
			if matches(query, project.name, project.branch) then
				local folder = project.home or "Elsewhere"
				local index = group_of[folder]
				if not index then
					table.insert(groups, { title = folder, rows = {} })
					index = #groups
					group_of[folder] = index
				end
				local path = project.path
				table.insert(groups[index].rows, row(state, path, project.branch, project.files, path == root))
			end
		end
		local sections: { any } = {}
		if #recent_rows > 0 then
			table.insert(sections, { title = "Recent", id = "recent", collapsed = state.folded.recent, table.unpack(recent_rows) })
		end
		for _, group in groups do
			table.insert(sections, { title = group.title, id = group.title, collapsed = state.folded[group.title], table.unpack(group.rows) })
		end
		local note: any = nil
		if not projects then
			note = ui.text { "Looking for repositories…", tone = "muted", class = "repos-note" }
		elseif #sections == 0 then
			note = ui.text { if query ~= "" then "No repository matches." else "No repositories found.", tone = "muted" }
		end
		return sidebar {
			header = ui.row {
				gap = "small",
				align = "center",
				search_field { id = "repos-filter", icon = true, grow = true, placeholder = "Filter repositories", change = "query" },
				icon_button { icon = "folder-open", size = "small", tooltip = "Open Repository…", msg = "open" },
			},
			sections = sections,
			state.problem and ui.text { state.problem, class = "repos-problem" } or nil,
			note,
		}
	end,
}
