-- The worktrees: every repository the projects provider found, each with
-- its work trees under it (the main one first), their branch, changes and
-- age, and a dot while an agent chat works in one. Picking one makes it the
-- workspace: it opens in the projects provider, and the chats, console and
-- inspector hear it on the topic `switchyard/workspace`.

local icon_button = use "icon-button"
local search = use "search-field"
local diff_counts = use "diff-counts"
local relative_time = use "relative-time"
local status_dot = use "status-dot"

type State = {
	selected: string?,
	query: string,
	searching: boolean,
	folded: { [string]: boolean? },
	workspace: { path: string, name: string, branch: string }?,
}

local function workspace_of(project: any): any
	return { path = project.path, name = project.name, branch = project.branch }
end

-- What the agents chatting in `path` are doing: "working", or waiting on
-- the user's answer ("blocked"), or nil.
local function chat_activity(app: any, path: string): string?
	local found = nil
	for _, chat in (app.chat and app.chat.chats) or {} do
		local transcript = chat.transcript
		if chat.cwd == path and transcript then
			if transcript.permissions and #transcript.permissions > 0 then
				return "blocked"
			elseif transcript.status and transcript.status.state == "working" then
				found = "working"
			end
		end
	end
	return found
end

return mod {
	title = "Worktrees",
	subscribe = { "provider/projects" },

	styles = {
		[".ws-top"] = { padding = { 0, 10, 0, 10 }, gap = 2 },
		[".ws-list"] = { padding = { 0, 10, 4, 10 }, gap = 2 },
		[".ws-repo"] = { padding = { 12, 8, 4, 8 }, gap = 6, radius = 8 },
		[".ws-repo:hover"] = { background = "list-hover" },
		[".ws-repo-badge"] = { background = "accent/14", color = "accent", radius = 6, padding = { 1, 6 }, weight = "semibold" },
		[".ws-repo-name"] = { weight = "medium" },
		[".ws-row"] = { padding = { 7, 10 }, radius = 10, gap = 2 },
		[".ws-row:hover"] = { background = "list-hover" },
		[".ws-row:selected"] = { background = "card", shadow = "0 1px 3px #00000014" },
		[".ws-branch"] = { weight = "medium" },
		[".ws-branch-primary"] = { weight = "semibold" },
		[".ws-meta"] = { color = "muted-foreground", size = "small" },
		[".ws-icon"] = { color = "muted-foreground" },
		[".ws-bottom"] = { padding = { 8, 12 }, gap = 4 },
	},

	init = function(previous: State?): State
		local state: State = previous or { selected = nil, query = "", searching = false, folded = {} }
		state.searching = false
		state.query = ""
		state.workspace = nil
		return state
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "search" then
				state.searching = not state.searching
				state.query = ""
			end
			return
		end
		if msg.topic == "provider/projects" then
			-- The first projects: tell everyone where we are.
			local projects = msg.value
			local current = projects and projects.current
			if state.selected and current and current.path ~= state.selected then
				return state, { { to = "provider:projects", msg = { select = state.selected } } }
			elseif current then
				local previous = state.workspace
				state.selected = current.path
				state.workspace = workspace_of(current)
				if not previous or previous.path ~= current.path or previous.branch ~= current.branch or previous.name ~= current.name then
					return state, { { to = "topic:switchyard/workspace", msg = state.workspace } }
				end
			end
			return
		end
		if msg.select then
			state.selected = msg.select.path
			state.workspace = msg.select
			return state, {
				{ to = "provider:projects", msg = { select = msg.select.path } },
				{ to = "topic:switchyard/workspace", msg = msg.select },
			}
		elseif msg.fold then
			state.folded[msg.fold] = if state.folded[msg.fold] then nil else true
		elseif msg.msg == "query" then
			state.query = msg.value
		elseif msg.msg == "cancel" then
			state.query = ""
			state.searching = false
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local query = string.lower(state.query)
		local now = (projects and projects.now) or 0

		-- Each project by its path, and the repositories by their main
		-- work tree, in the order their newest work tree comes.
		local by_path, repos, order = {}, {}, {}
		for _, project in (projects and projects.projects) or {} do
			by_path[project.path] = project
		end
		for _, project in (projects and projects.projects) or {} do
			local main = (project.worktrees and project.worktrees[1] and project.worktrees[1].path) or project.path
			if not repos[main] then
				local head = by_path[main] or project
				repos[main] = { name = head.name, path = main, trees = head.worktrees or {} }
				table.insert(order, main)
			end
		end

		local list = {}
		for _, main in order do
			local repo = repos[main]
			local rows = {}
			local trees = if #repo.trees > 0 then repo.trees else { { path = repo.path, branch = (by_path[repo.path] or {}).branch, primary = true } }
			for _, tree in trees do
				local project = by_path[tree.path]
				local branch = tree.branch or (project and project.branch) or "detached"
				local matches = query == "" or string.find(string.lower(branch), query, 1, true)
					or string.find(string.lower(repo.name), query, 1, true)
				if matches then
					local activity = chat_activity(app, tree.path)
					local workspace = { path = tree.path, name = repo.name, branch = branch }
					table.insert(rows, ui.column {
						class = "ws-row",
						selected = tree.path == state.selected,
						msg = { select = workspace },
						tooltip = tree.path,
						ui.row { align = "center", gap = "small",
							ui.icon { name = "git-branch", size = "small", class = "ws-icon" },
							ui.text { branch, class = if tree.primary then "ws-branch ws-branch-primary" else "ws-branch", truncate = true },
							tree.primary and ui.text { "★", class = "ws-meta" },
							ui.spacer(),
							activity and status_dot { status = activity, size = "small",
								tooltip = if activity == "blocked" then "An agent is waiting on you" else "An agent is working" },
							project and project.files > 0 and diff_counts { added = project.added, removed = project.deleted },
						},
						project and ui.row { align = "center", gap = "small", style = { padding = { 0, 0, 0, 22 } },
							relative_time { seconds = math.max(0, now - project.time) },
							project.behind > 0 and ui.text { `↓{project.behind}`, class = "ws-meta" },
							project.ahead > 0 and ui.text { `↑{project.ahead}`, class = "ws-meta" },
						},
					})
				end
			end
			if #rows > 0 then
				local folded = state.folded[main] and query == ""
				table.insert(list, ui.row { class = "ws-repo", align = "center", msg = { fold = main }, tooltip = main,
					ui.text { string.upper(string.sub(repo.name, 1, 1)), class = "ws-repo-badge", size = "small" },
					ui.text { repo.name, class = "ws-repo-name", truncate = true },
					ui.icon { name = if folded then "chevron-right" else "chevron-down", size = "small", class = "ws-icon" },
					ui.spacer(),
					ui.text { tostring(#rows), class = "ws-meta" },
				})
				if not folded then
					for _, row in rows do
						table.insert(list, row)
					end
				end
			end
		end

		local body
		if not projects then
			body = ui.text { "Looking for repositories…", tone = "muted", style = { padding = { 12, 20 } } }
		elseif projects.available == false then
			body = ui.text { projects.reason or "No repositories", tone = "muted", style = { padding = { 12, 20 } } }
		elseif #list == 0 then
			body = ui.text {
				if query ~= "" then "No worktree matches" else `No git repositories in {table.concat(projects.roots or {}, ", ")}`,
				tone = "muted", wrap = true, style = { padding = { 12, 20 } },
			}
		else
			body = ui.column { class = "ws-list", shrink = false, table.unpack(list) }
		end

		return ui.column { grow = true, gap = "none",
			ui.row { class = "ws-top", drag = true, height = 52, align = "center",
				-- Room for the traffic lights.
				ui.row { width = 76 },
				ui.spacer(),
				icon_button { icon = "search", tooltip = "Filter worktrees", msg = "search", active = state.searching },
				icon_button { icon = "panel-left", tooltip = "Hide worktrees", msg = "toggle-left", to = "shell" },
			},
			state.searching and ui.row { style = { padding = { 0, 14, 6, 14 } },
				search { id = "worktrees", value = state.query, placeholder = "Filter worktrees…", change = "query", cancel = "cancel", focus = true },
			},
			ui.column { grow = true, scroll = true, gap = "none", body },
			ui.row { class = "ws-bottom", align = "center",
				icon_button { icon = "palette", tooltip = "Themes", command = "show-themes" },
				icon_button { icon = "refresh-cw", tooltip = "Look for repositories again", msg = { refresh = true }, to = "provider:projects" },
				ui.spacer(),
				icon_button { icon = "panel-bottom", tooltip = "Show or hide the console", msg = "toggle-bottom", to = "shell" },
				icon_button { icon = "panel-right", tooltip = "Show or hide the inspector", msg = "toggle-right", to = "shell" },
			},
		}
	end,
}
