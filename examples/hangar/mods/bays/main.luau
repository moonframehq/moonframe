-- The bays: every worktree of the repository, with what its agent is doing,
-- its changes and its pull request. Picking one puts it on the topic
-- `hangar/bay`, and the deck shows its terminal. New bays and removals go
-- through the git provider, which picks the folder, keeps the branch, and
-- refuses to throw away uncommitted work.

local ahead_behind = use "ahead-behind"
local forge_pr = use "forge-pr"
local icon_button = use "icon-button"
local status_dot = use "status-dot"

type Worktree = {
	path: string,
	branch: string,
	changed: number,
	ahead: number,
	behind: number,
	prunable: boolean?,
	bare: boolean?,
}

type State = {
	selected: string?,
	-- What the deck was last told, to tell it again only of a change.
	told: string?,
	-- The repository as git last named it: a write says which it is for.
	root: string?,
	worktrees: { Worktree },
	-- What each bay is doing, from the deck.
	activity: { [string]: string },
	adding: boolean,
	draft: string,
	-- The path whose removal waits for a second click.
	removing: string?,
	-- A bay added but not listed yet.
	awaited: string?,
	-- A write is running, and what the last one refused.
	busy: boolean,
	problem: string?,
}

-- The worktrees worth a bay: a bare repository has no files, and a prunable
-- entry's folder is gone.
local function usable(worktrees: { any }): { Worktree }
	local kept = {}
	for _, worktree in worktrees do
		if not worktree.bare and not worktree.prunable then
			table.insert(kept, worktree)
		end
	end
	return kept
end

local function find(worktrees: { Worktree }, path: string?): (Worktree?, number?)
	for index, worktree in worktrees do
		if worktree.path == path then
			return worktree, index
		end
	end
	return nil, nil
end

-- Tells the deck which bay is on show.
local function announce(state: State): { any }
	local bay = find(state.worktrees, state.selected)
	state.told = bay and `{bay.path}\0{bay.branch}`
	return { { to = "topic:hangar/bay", msg = bay and { path = bay.path, branch = bay.branch } or false } }
end

-- Whether the deck has not heard of the bay on show as it is now: it may be
-- new, restored from the last launch, or on another branch.
local function untold(state: State): boolean
	local bay = find(state.worktrees, state.selected)
	return state.told ~= (bay and `{bay.path}\0{bay.branch}`)
end

-- Keeps a selection that exists: the first bay, when the chosen one went.
local function settle(state: State)
	if not find(state.worktrees, state.selected) then
		local first = state.worktrees[1]
		state.selected = first and first.path
	end
end

local function write(state: State, id: string, fields: { [string]: any }): { any }
	state.busy = true
	state.problem = nil
	local msg = table.clone(fields)
	msg.root = state.root
	msg.id = id
	return { { to = "provider:git", msg = msg } }
end

-- A key from a path, which holds what keys cannot: the end of it is what
-- tells worktrees apart.
local function key_of(path: string): string
	local safe = string.gsub(path, "[^%w_-]", "-")
	return string.sub(safe, -64)
end

local function row_of(worktree: Worktree, state: State, app: any)
	local activity = state.activity[worktree.path]
	local name = string.match(worktree.path, "([^/\\]+)$") or worktree.path
	local confirming = state.removing == worktree.path
	return ui.column {
		key = key_of(worktree.path),
		class = "bay",
		selected = worktree.path == state.selected,
		msg = { pick = worktree.path },
		context = { remove = worktree.path },
		ui.row {
			align = "center",
			gap = "small",
			status_dot { status = activity or "idle", spin = true, size = "normal" },
			ui.row { grow = true, ui.text { if worktree.branch ~= "" then worktree.branch else name, class = "bay-branch", truncate = true } },
			forge_pr { forge = app.github, branch = worktree.branch },
		},
		ui.row {
			align = "center",
			gap = "small",
			class = "bay-meta",
			ui.row { grow = true, ui.text { name, class = "bay-folder", truncate = true } },
			worktree.changed > 0 and ui.text { `{worktree.changed} changed`, class = "bay-changed" } or nil,
			ahead_behind { ahead = worktree.ahead, behind = worktree.behind },
		},
		confirming and ui.column {
			gap = "small",
			class = "bay-confirm",
			ui.text { "Remove this bay? Its branch stays.", class = "bay-folder" },
			ui.row {
				gap = "small",
				ui.button { "Cancel", small = true, msg = "keep" },
				ui.button { "Remove", small = true, msg = { confirm = worktree.path } },
			},
		} or nil,
	}
end

return mod {
	region = "left",
	title = "Bays",
	subscribe = { "provider/git", "hangar/activity" },

	styles = {
		[".bays"] = { padding = { 44, 10, 10, 10 }, gap = 6 },
		[".bays-head"] = { padding = { 0, 6, 6, 6 } },
		[".bays-title"] = { weight = "semibold" },
		[".bay"] = { padding = { 9, 10 }, radius = 10, gap = 3 },
		[".bay:hover"] = { background = "list-hover" },
		[".bay:selected"] = { background = "card", shadow = "0 1px 3px #00000022" },
		[".bay-branch"] = { weight = "medium" },
		[".bay-folder"] = { size = "small", color = "muted-foreground" },
		[".bay-changed"] = { size = "small", color = "warning" },
		[".bay-confirm"] = { padding = { 6, 0, 0, 0 } },
		[".bays-problem"] = { size = "small", color = "danger", padding = { 2, 6 } },
	},

	init = function(previous: any): (State, { any })
		local state: State = {
			selected = previous and previous.selected,
			worktrees = {},
			activity = {},
			adding = false,
			draft = "",
			busy = false,
		}
		-- A topic outlives a launch: the bay it holds may be another
		-- repository's, so none is on show until git says what is here.
		return state, announce(state)
	end,

	update = function(state: State, msg: any)
		if msg == "add" then
			state.adding = not state.adding
			state.draft = ""
			state.problem = nil
			return state
		elseif msg == "keep" then
			state.removing = nil
			return state
		elseif type(msg) ~= "table" then
			return
		end
		if msg.topic == "provider/git" and type(msg.value) == "table" then
			state.root = msg.value.root
			state.worktrees = usable(msg.value.worktrees or {})
			-- A bay just added is not listed until now, and stays chosen.
			if not (state.awaited and not find(state.worktrees, state.awaited)) then
				state.awaited = nil
				settle(state)
			end
			if untold(state) then
				return state, announce(state)
			end
			return state
		elseif msg.topic == "hangar/activity" then
			state.activity = msg.value or {}
			return state
		elseif msg.pick then
			state.selected = msg.pick
			state.removing = nil
			return state, announce(state)
		elseif msg.step then
			local _, index = find(state.worktrees, state.selected)
			local count = #state.worktrees
			if index and count > 0 then
				state.selected = state.worktrees[(index - 1 + msg.step) % count + 1].path
				return state, announce(state)
			end
			return
		elseif msg.draft then
			state.draft = msg.value
			return state
		elseif msg.cancel then
			state.adding = false
			state.draft = ""
			state.problem = nil
			return state
		elseif msg.submit then
			if string.find(state.draft, "%S") and not state.busy then
				return state, write(state, "add", { worktree_add = { branch = state.draft } })
			end
			return
		elseif msg.remove then
			state.removing = msg.remove
			return state
		elseif msg.confirm then
			state.removing = nil
			return state, write(state, "remove", { worktree_remove = msg.confirm })
		elseif msg.reply == "add" or msg.reply == "remove" then
			state.busy = false
			if not msg.ok then
				state.problem = msg.error
				return state
			end
			if msg.reply == "add" then
				state.adding = false
				state.draft = ""
				-- Git lists it on its next look; the deck opens it then.
				state.selected = msg.message
				state.awaited = msg.message
			end
			return state
		end
		return
	end,

	view = function(state: State, app)
		local rows: { any } = {}
		for _, worktree in state.worktrees do
			table.insert(rows, row_of(worktree, state, app))
		end
		return ui.column {
			grow = true,
			gap = "none",
			class = "bays",
			ui.row {
				align = "center",
				class = "bays-head",
				ui.row { grow = true, ui.text { app.git and app.git.repository and "Bays" or "Not a repository", class = "bays-title" } },
				icon_button { icon = "plus", tooltip = "New bay", msg = "add" },
			},
			state.adding and ui.input {
				id = "branch",
				value = state.draft,
				placeholder = "Branch for the new bay",
				change = { draft = true },
				submit = { submit = true },
				keys = { escape = { cancel = true } },
			} or nil,
			state.problem and ui.text { state.problem, class = "bays-problem" } or nil,
			ui.column { grow = true, gap = "none", scroll = true, table.unpack(rows) },
		}
	end,
}
