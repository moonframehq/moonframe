-- The middle pane: the open repository's history as a graph (every branch,
-- tag and HEAD, newest first), or its branches, as the shell's History and
-- Branches say with `{ tab = ... }`. Searching lists the commits whose
-- message, author or id holds the words instead of the graph. The commit
-- picked is shown by the commit mod, from what the git provider publishes
-- as `commit` once this mod asks it with `show`.

local ahead_behind = use "ahead-behind"
local commit_graph = use "commit-graph"
local empty_state = use "empty-state"
local relative_time = use "relative-time"
local search_field = use "search-field"

-- Commits asked of git: the most it gives.
local COMMITS = 500
-- Search results listed.
local MAX_FOUND = 200

type State = {
	-- The repository the selection is in; another one starts over.
	root: string?,
	-- The commit picked, its full id.
	selected: string?,
	query: string,
	-- Focus the search field (View > Find a Commit).
	finding: boolean,
	-- The log git last published, for Up and Down to step through.
	log: { any }?,
	-- What it lists: "history" or "branches".
	tab: string,
}

local function show(state: State, sha: string): { any }
	state.selected = sha
	return { { to = "provider:git", msg = { show = sha } } }
end

-- The commits whose subject, author or id holds `query`.
local function found(log: { any }, query: string): { any }
	local hits = {}
	for _, commit in log do
		local subject = string.lower(commit.subject or "")
		local author = string.lower(commit.author or "")
		if
			string.find(subject, query, 1, true)
			or string.find(author, query, 1, true)
			or string.sub(commit.sha, 1, #query) == query
		then
			table.insert(hits, commit)
			if #hits >= MAX_FOUND then
				break
			end
		end
	end
	return hits
end

-- The index in `log` of the commit picked, if it is there.
local function index_of(log: { any }, sha: string?): number?
	for index, commit in log do
		if commit.sha == sha then
			return index
		end
	end
	return nil
end

local function ago(now: number?, time: number?): any
	if not now or not time then
		return nil
	end
	return relative_time { seconds = math.max(0, now - time), compact = true }
end

local function search_rows(state: State, hits: { any }, now: number?)
	local rows: { any } = {}
	for _, commit in hits do
		table.insert(
			rows,
			ui.row {
				class = "found",
				gap = "small",
				align = "center",
				selected = commit.sha == state.selected,
				msg = { pick = commit.sha },
				ui.column {
					grow = true,
					gap = "none",
					ui.text { commit.subject or "", truncate = true },
					ui.row {
						gap = "small",
						ui.text { commit.short or "", class = "found-sha" },
						ui.text { commit.author or "", tone = "muted", size = "small", truncate = true },
					},
				},
				ago(now, commit.time),
			}
		)
	end
	return rows
end

local function branch_rows(git: any, state: State, now: number?)
	local rows: { any } = {}
	for _, branch in git.branches or {} do
		local picked = state.selected ~= nil and branch.head ~= nil
			and string.sub(state.selected, 1, #branch.head) == branch.head
		table.insert(
			rows,
			ui.row {
				class = "branch",
				gap = "small",
				align = "center",
				selected = picked,
				msg = { branch = branch.head },
				tooltip = if branch.upstream then `{branch.name} → {branch.upstream}` else branch.name,
				ui.icon { if branch.current then "circle-dot" else "git-branch", tone = if branch.current then "accent" else "muted" },
				ui.column {
					grow = true,
					gap = "none",
					ui.text { branch.name, weight = if branch.current then "semibold" else "normal", truncate = true },
					ui.text { branch.head or "", class = "found-sha" },
				},
				branch.gone and ui.badge { "gone", tone = "warning" }
					or ahead_behind { ahead = branch.ahead or 0, behind = branch.behind or 0 },
				ago(now, branch.time),
			}
		)
	end
	return rows
end

return mod {
	title = "History",
	subscribe = { "provider/git" },

	-- Up and Down step through the history; a focused field keeps them.
	keys = { up = { step = -1 }, down = { step = 1 } } :: any,

	styles = {
		[".timeline-head"] = { padding = { 8, 10, 6, 10 } },
		[".found"] = { padding = { 6, 12 }, radius = 6, margin = { 0, 6 } },
		[".found:hover"] = { background = "list-hover" },
		[".found:selected"] = { background = "list-active" },
		[".found-sha"] = { size = "small", font = "mono", color = "muted-foreground" },
		[".branch"] = { padding = { 7, 12 }, radius = 6, margin = { 0, 6 } },
		[".branch:hover"] = { background = "list-hover" },
		[".branch:selected"] = { background = "list-active" },
	},

	-- Never the saved state: git starts with no log asked for.
	init = function(): (State, { any }?)
		local state: State = { query = "", finding = false, tab = "history" }
		return state, { { to = "provider:git", msg = { log = COMMITS } } }
	end,

	update = function(state: State, msg: any)
		if msg == "find" then
			state.finding = true
			return state, { { to = "shell", msg = { select = "history" } } }
		elseif type(msg) ~= "table" then
			return
		end
		if msg.topic == "provider/git" then
			local git = msg.value
			if type(git) ~= "table" or not git.repository then
				return
			end
			local log = git.log
			state.log = log
			if git.root ~= state.root then
				-- Another repository: start from its newest commit.
				state.root = git.root
				state.selected = nil
				state.query = ""
			end
			if not state.selected and log and log[1] then
				return state, show(state, log[1].sha)
			end
			return state
		elseif msg.tab then
			state.tab = msg.tab
			return state
		elseif msg.msg == "query" then
			state.query = string.lower(msg.value or "")
			state.finding = false
			return state
		elseif msg.msg == "cancel" then
			state.query = ""
			state.finding = false
			return state
		elseif msg.select == "pick" and msg.sha then
			return state, show(state, msg.sha)
		elseif msg.pick then
			return state, show(state, msg.pick)
		elseif msg.branch then
			-- A branch's tip, by its short id; the graph marks it once git
			-- says which commit that is.
			return state, show(state, msg.branch)
		elseif msg.step then
			local log = state.log
			if not log or #log == 0 then
				return
			end
			local list = if state.query ~= "" then found(log, state.query) else log
			local at = index_of(list, state.selected) or 0
			local next = list[math.clamp(at + msg.step, 1, #list)]
			if next and next.sha ~= state.selected then
				return state, show(state, next.sha)
			end
			return
		end
		return
	end,

	view = function(state: State, app)
		local git = app.git
		local now = app.projects and app.projects.now
		local tab = state.tab
		if not git then
			return ui.text { "Reading the repository…", tone = "muted", class = "timeline-head" }
		end
		-- A branch picked by its short id: the full one, once git shows it.
		local commit = git.commit
		local selected = state.selected
		if commit and selected and string.sub(commit.sha or "", 1, #selected) == selected then
			selected = commit.sha
		end
		if tab == "branches" then
			local rows = branch_rows(git, state, now)
			return ui.column {
				grow = true,
				gap = "none",
				ui.row {
					class = "timeline-head",
					ui.text { `{#rows} local branches, most recent first`, tone = "muted", size = "small" },
				},
				ui.column { grow = true, scroll = true, gap = "none", shrink = false, table.unpack(rows) },
			}
		end
		local log = git.log
		local body: any
		if not log then
			body = ui.text { "Reading commits…", tone = "muted", class = "timeline-head" }
		elseif #log == 0 then
			body = empty_state { icon = "git-commit-horizontal", title = "No commits", message = "Commits show here once the repository has some." }
		elseif state.query ~= "" then
			local hits = found(log, state.query)
			body = if #hits == 0
				then empty_state { icon = "search", title = "Nothing found", message = `No commit among the newest {#log} matches.` }
				else ui.column { grow = true, scroll = true, gap = "none", shrink = false, table.unpack(search_rows(state, hits, now)) }
		else
			body = ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				shrink = false,
				commit_graph { commits = log, compact = true, select = "pick", selected = selected },
			}
		end
		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "timeline-head",
				search_field {
					id = "timeline-find",
					icon = true,
					grow = true,
					placeholder = "Find by message, author or id",
					change = "query",
					cancel = "cancel",
					focus = state.finding,
					shortcut = "secondary-f",
				},
			},
			body,
		}
	end,
}
