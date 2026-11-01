-- The projects: Home, then each repository the projects provider found,
-- its work trees nested under it when open (the main one tagged PRIMARY),
-- a row to add a work tree, and Remote at the foot. Agents running in a
-- work tree (from `app.agents`) spin beside it; when one ends there while
-- you are elsewhere, the work tree is marked done until you visit it.
-- Picking a work tree opens it in the provider and publishes it on
-- `trellis/worktree`, which the shell and the panels follow.

local icon_button = use "icon-button"
local avatar = use "avatar"

type State = {
	selected: string?,
	expanded: { [string]: boolean? },
	-- The last work tree picked in each repository.
	last: { [string]: string? },
	-- Agents by their folder, as last seen, and those that ended unseen.
	working: { [string]: string? },
	finished: { [string]: string? },
	adding: string?,
	draft: string,
	pending: string?,
	note: string?,
	made: number,
	remote: boolean,
}

type Tree = { path: string, name: string, branch: string, primary: boolean, summary: any }
type Repo = { path: string, name: string, trees: { Tree }, files: number }

local AGENT_ICONS = {
	claude = "agent-claude", codex = "agent-codex", gemini = "agent-gemini", cursor = "agent-cursor",
	copilot = "agent-copilot", opencode = "agent-opencode", amp = "agent-amp", droid = "agent-droid",
	kiro = "agent-kiro", qwen = "agent-qwen", grok = "agent-grok",
}

local function base(path: string): string
	return string.match(path, "([^/]+)$") or path
end

local function inside(path: string, folder: string): boolean
	return path == folder or string.sub(path, 1, #folder + 1) == folder .. "/"
end

-- Repositories in the order their newest work tree comes, each with its
-- work trees as git lists them (the main one first).
local function repositories(projects: any): { Repo }
	local repos: { [string]: Repo } = {}
	local order: { Repo } = {}
	local by_path = {}
	for _, project in (projects and projects.projects) or {} do
		by_path[project.path] = project
	end
	for _, project in (projects and projects.projects) or {} do
		local trees = project.worktrees or {}
		local main = if #trees > 0 then trees[1].path else project.path
		if not repos[main] then
			local repo: Repo = { path = main, name = base(main), trees = {}, files = 0 }
			if #trees == 0 then
				table.insert(repo.trees, { path = project.path, name = project.name, branch = project.branch, primary = true, summary = project })
			end
			for _, tree in trees do
				table.insert(repo.trees, {
					path = tree.path,
					name = if tree.primary then base(main) else base(tree.path),
					branch = tree.branch,
					primary = tree.primary,
					summary = by_path[tree.path],
				})
			end
			for _, tree in repo.trees do
				if tree.summary then
					repo.files += tree.summary.files
				end
			end
			repos[main] = repo
			table.insert(order, repo)
		end
	end
	return order
end

local function publish(path: string, name: string, repo: string?, branch: string?): { any }
	return {
		{ to = "topic:trellis/worktree", msg = { path = path, name = name, repo = repo, branch = branch } },
		{ to = "provider:projects", msg = { select = path } },
	}
end

-- Agents by the work tree they run in: the deepest one holding their
-- folder, since work trees often live inside their main one.
local function by_tree(agents: { [string]: string? }, repos: { Repo }): { [string]: string }
	local out = {}
	for folder, name in agents do
		local best: string? = nil
		for _, repo in repos do
			for _, tree in repo.trees do
				if inside(folder, tree.path) and (not best or #tree.path > #best) then
					best = tree.path
				end
			end
		end
		if best and name then
			out[best] = name
		end
	end
	return out
end

return mod {
	region = "left",
	title = "Projects",
	subscribe = { "provider/agents", "provider/projects" },

	styles = {
		[".nav-top"] = { padding = { 0, 8, 0, 10 }, gap = 2 },
		[".nav-filter-row"] = { padding = { 0, 8 }, gap = 6 },
		[".nav-filter"] = { background = "secondary", radius = 7, padding = { 0, 8, 0, 10 }, gap = 6 },
		[".nav-filter-text"] = { size = "small", weight = "medium", color = "muted-foreground" },
		[".nav-square"] = { background = "secondary", radius = 7, color = "muted-foreground" },
		[".nav-list"] = { padding = { 8, 8 }, gap = 2 },
		[".nav-row"] = { padding = { 0, 6 }, radius = 8, gap = 10 },
		[".nav-row:hover"] = { background = "list-hover" },
		[".nav-row:selected"] = { background = "raised" },
		[".nav-name"] = { weight = "medium" },
		[".nav-sub"] = { size = "small", color = "muted-foreground" },
		[".nav-tile"] = { radius = 7, color = "#ffffff" },
		[".nav-home"] = { background = "#b06ab9" },
		[".nav-plus"] = { background = "secondary", color = "muted-foreground" },
		[".nav-tree"] = { padding = { 0, 8, 0, 48 }, radius = 8, gap = 8 },
		[".nav-tree:hover"] = { background = "list-hover" },
		[".nav-tree:selected"] = { background = "raised" },
		[".nav-primary"] = { background = "secondary", radius = 4, padding = { 0, 5 }, color = "muted-foreground", weight = "bold" },
		[".nav-count"] = { size = "small", color = "muted-foreground", font = "mono" },
		[".nav-muted"] = { color = "muted-foreground" },
		[".nav-new"] = { padding = { 0, 8, 0, 18 }, radius = 8, gap = 18, color = "muted-foreground" },
		[".nav-new:hover"] = { background = "list-hover", color = "foreground" },
		[".nav-input"] = { background = "background", border = true, border_color = "border", radius = 6, padding = { 4, 8 } },
		[".nav-note"] = { size = "small", color = "danger", padding = { 0, 8, 0, 48 } },
		[".nav-done"] = { color = "success" },
		[".nav-agent"] = { color = "warning" },
		[".nav-foot"] = { padding = { 8, 8, 10, 8 } },
		[".nav-remote"] = { background = "secondary", radius = 8, padding = { 0, 10 }, gap = 8, color = "muted-foreground" },
		[".nav-remote:hover"] = { color = "foreground" },
		[".nav-remote-body"] = { padding = { 6, 12 }, gap = 4 },
		[".avatar"] = { radius = 7 },
	},

	init = function(previous: State?)
		local state: State = previous
			or {
				selected = nil,
				expanded = {},
				last = {},
				working = {},
				finished = {},
				adding = nil,
				draft = "",
				pending = nil,
				note = nil,
				made = 0,
				remote = false,
			}
		state.adding, state.pending, state.note = nil, nil, nil
		state.finished = state.finished or {}
		state.working = state.working or {}
		-- The work tree open when the app last closed opens again.
		if state.selected then
			local path = state.selected :: string
			return state, publish(path, if path == "~" then "Home" else base(path), nil, nil)
		end
		return state
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "cancel-add" then
				state.adding = nil
				state.note = nil
			elseif msg == "remote" then
				state.remote = not state.remote
			elseif msg == "home" then
				state.selected = "~"
				return state, publish("~", "Home", nil, nil)
			end
			return
		end
		if msg.topic == "provider/agents" then
			-- Agents by folder; one gone since the last look has finished.
			local now: { [string]: string? } = {}
			for _, agent in (msg.value and msg.value.agents) or {} do
				if agent.cwd then
					now[agent.cwd] = agent.name
				end
			end
			for folder, name in state.working do
				if not now[folder] and not (state.selected and inside(folder, state.selected)) then
					state.finished[folder] = name
				end
			end
			state.working = now
			return
		elseif msg.topic == "provider/projects" then
			-- At the very first launch, the most recently active work tree opens.
			local first = msg.value and msg.value.projects and msg.value.projects[1]
			if not state.selected and first then
				local trees = first.worktrees or {}
				local repo = if #trees > 0 then trees[1].path else first.path
				state.selected = first.path
				state.expanded[repo] = true
				state.last[repo] = first.path
				return state, publish(first.path, base(first.path), base(repo), first.branch)
			end
			-- The answer to a work tree we asked for.
			local action = msg.value and msg.value.action
			if state.pending and action and action.id == state.pending then
				state.pending = nil
				if action.ok then
					state.adding = nil
					state.note = nil
					if action.path then
						state.selected = action.path
						return state, publish(action.path, base(action.path), nil, nil)
					end
				else
					state.note = action.error
				end
			end
			return
		end
		if msg.repo then
			-- A repository: open it, or fold it when it is open and shown.
			local repo = msg.repo
			local shown = state.selected and inside(state.selected, repo) or (state.last[repo] == state.selected)
			if state.expanded[repo] and shown then
				state.expanded[repo] = nil
				return
			end
			state.expanded[repo] = true
			local path = state.last[repo] or msg.primary or repo
			state.selected = path
			return state, publish(path, base(path), base(repo), nil)
		elseif msg.tree then
			state.selected = msg.tree
			state.last[msg.in_repo] = msg.tree
			for folder in state.finished do
				if inside(folder, msg.tree) then
					state.finished[folder] = nil
				end
			end
			return state, publish(msg.tree, msg.name, base(msg.in_repo), msg.branch)
		elseif msg.add then
			state.adding = if state.adding == msg.add then nil else msg.add
			state.draft = ""
			state.note = nil
		elseif msg.msg == "draft" then
			state.draft = msg.value
		elseif msg.msg == "create" and state.adding then
			local name = string.gsub(msg.value or state.draft, "^%s*(.-)%s*$", "%1")
			if name == "" then
				return
			end
			state.made += 1
			state.pending = `w{state.made}`
			state.note = nil
			return state, { { to = "provider:projects", msg = { worktree = name, repo = state.adding, id = state.pending } } }
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local repos = repositories(projects)
		local working, finished = by_tree(state.working, repos), by_tree(state.finished, repos)
		local window = app.window
		local mac = window.platform == "macos" and not window.fullscreen

		local rows = {}
		table.insert(rows, ui.row {
			class = "nav-row",
			selected = state.selected == "~",
			height = 40,
			align = "center",
			msg = "home",
			ui.row {
				class = "nav-tile nav-home",
				width = 32,
				height = 32,
				align = "center",
				justify = "center",
				ui.icon { name = "house" },
			},
			ui.text { "Home", class = "nav-name" },
		})

		for _, repo in repos do
			local open = state.expanded[repo.path] == true
			local chosen = nil
			for _, tree in repo.trees do
				if tree.path == state.selected then
					chosen = tree
				end
			end
			-- What the folded tile says: an agent at work, or one done.
			local status = nil
			for _, tree in repo.trees do
				if working[tree.path] then
					status = "working"
				elseif not status and finished[tree.path] then
					status = "done"
				end
			end
			local primary = repo.trees[1] and repo.trees[1].path
			table.insert(rows, ui.row {
				class = "nav-row",
				selected = chosen ~= nil and (not open or #repo.trees > 1),
				height = 40,
				align = "center",
				msg = { repo = repo.path, primary = primary },
				tooltip = repo.path,
				avatar { name = repo.name, size = "large", status = if open then nil else status :: any, tooltip = repo.path },
				ui.column {
					grow = true,
					gap = "none",
					ui.text { repo.name, class = "nav-name", truncate = true },
					chosen and not chosen.primary and ui.text { chosen.name, class = "nav-sub", truncate = true },
				},
				open and ui.icon { name = "chevron-down", class = "nav-muted", size = "small" },
			})
			if open then
				for _, tree in repo.trees do
					local agent = working[tree.path]
					local done = not agent and finished[tree.path]
					local files = tree.summary and tree.summary.files or 0
					table.insert(rows, ui.row {
						class = "nav-tree",
						selected = tree.path == state.selected,
						height = 32,
						align = "center",
						msg = { tree = tree.path, name = tree.name, branch = tree.branch, in_repo = repo.path },
						tooltip = `{tree.branch} · {tree.path}`,
						ui.text { tree.name, truncate = true },
						tree.primary and ui.text { "PRIMARY", class = "nav-primary", size = "small" },
						ui.spacer(),
						files > 0 and ui.text { tostring(files), class = "nav-count", tooltip = `{files} changed` },
						agent and ui.icon { name = (AGENT_ICONS[agent] or "bot") :: any, class = "nav-agent", size = "small", tooltip = `{agent} is running here` },
						done and ui.icon { name = "circle-check", class = "nav-done", size = "small", tooltip = `{done} finished here` },
					})
				end
				if state.adding == repo.path then
					table.insert(rows, ui.row {
						style = { padding = { 4, 8, 4, 44 } },
						ui.input {
							id = "new-worktree",
							class = "nav-input",
							grow = true,
							value = state.draft,
							placeholder = "Branch for the new work tree",
							change = "draft",
							submit = "create",
							keys = { escape = "cancel-add" },
							focus = true,
							disabled = state.pending ~= nil,
						},
					})
					table.insert(rows, if state.note
						then ui.text { state.note, class = "nav-note", wrap = true }
						else ui.text {
							if state.pending then "Adding…" else `↵ adds ~/.moonframe/worktrees/{repo.name}/<branch>`,
							class = "nav-sub",
							style = { padding = { 0, 8, 2, 48 } },
							truncate = true,
						})
				else
					table.insert(rows, ui.row {
						class = "nav-new",
						height = 32,
						align = "center",
						msg = { add = repo.path },
						ui.icon { name = "plus", size = "small" },
						ui.text { "New Worktree" },
					})
				end
			end
		end

		local roots = table.concat((projects and projects.roots) or {}, ", ")
		table.insert(rows, ui.row {
			class = "nav-row",
			height = 40,
			align = "center",
			msg = { refresh = true },
			to = "provider:projects",
			tooltip = `Trellis lists the git repositories in {if roots == "" then "your project folders" else roots}; set MOONFRAME_PROJECTS for others. Click to look again.`,
			ui.row {
				class = "nav-tile nav-plus",
				width = 32,
				height = 32,
				align = "center",
				justify = "center",
				ui.icon { name = "plus" },
			},
			ui.text { "Add Project", class = "nav-muted" },
		})

		local herdr = app.herdr
		local body = if not projects
			then ui.text { "Looking for projects…", tone = "muted", style = { padding = { 8, 14 } } }
			else ui.column { class = "nav-list", shrink = false, table.unpack(rows) }

		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "nav-top",
				drag = true,
				height = 32,
				shrink = false,
				align = "center",
				mac and ui.row { width = 72 },
				ui.spacer(),
				icon_button { icon = "panel-left", size = "small", tooltip = "Hide projects (⌘B)", msg = "toggle-left", to = "shell" },
			},
			ui.row {
				class = "nav-filter-row",
				height = 28,
				shrink = false,
				ui.row {
					class = "nav-filter",
					grow = true,
					align = "center",
					tooltip = "Every repository with its work trees",
					ui.text { "All Projects", class = "nav-filter-text" },
					ui.spacer(),
					ui.text { tostring(#repos), class = "nav-count" },
				},
				ui.row {
					class = "nav-square",
					width = 28,
					align = "center",
					justify = "center",
					msg = { refresh = true },
					to = "provider:projects",
					tooltip = "Look for projects again",
					ui.icon { name = "refresh-cw", size = "small" },
				},
			},
			ui.column { grow = true, scroll = true, gap = "none", body },
			ui.column {
				class = "nav-foot",
				gap = "small",
				shrink = false,
				state.remote and ui.column {
					class = "nav-remote-body",
					ui.text { "Terminals here", class = "nav-sub" },
					ui.row {
						align = "center",
						gap = "small",
						ui.icon { name = "server", size = "small", tone = "muted" },
						ui.text { if herdr and herdr.installed then "herdr-core" elseif herdr and herdr.tmux then "tmux" else "this window" },
						ui.spacer(),
						ui.text {
							if herdr and (herdr.installed or herdr.tmux) then "kept on quit" else "end on quit",
							class = "nav-sub",
						},
					},
					ui.text { "No remote hosts added", class = "nav-sub" },
				},
				ui.row {
					class = "nav-remote",
					height = 34,
					align = "center",
					msg = "remote",
					ui.icon { name = "globe", size = "small" },
					ui.text { "Remote" },
					ui.spacer(),
					ui.text { "0 hosts", class = "nav-sub" },
					ui.icon { name = if state.remote then "chevron-down" else "chevron-up", size = "small" },
				},
			},
		}
	end,
}
