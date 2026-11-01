-- The sessions: every repository the projects provider found, grouped by
-- where it lives, newest first. Each row has the agent running in it (from
-- `app.agents`), its age, its branch and its uncommitted changes. Picking
-- one opens it in the provider and tells the session and changes mods.

local icon_button = use "icon-button"
local search = use "search-field"
local diff_counts = use "diff-counts"
local relative_time = use "relative-time"
local status_dot = use "status-dot"

type State = {
	selected: string?,
	query: string,
	searching: boolean,
	changed_only: boolean,
}

-- The agent working in `path`, if any: the first whose folder is in it.
local function agent_in(app: any, path: string): string?
	local agents = app.agents and app.agents.agents
	if not agents then
		return nil
	end
	for _, agent in agents do
		local cwd = agent.cwd
		if cwd and (cwd == path or string.sub(cwd, 1, #path + 1) == path .. "/") then
			return agent.name
		end
	end
	return nil
end

local AGENT_ICONS = {
	claude = "agent-claude", codex = "agent-codex", gemini = "agent-gemini", cursor = "agent-cursor",
	copilot = "agent-copilot", opencode = "agent-opencode", amp = "agent-amp", droid = "agent-droid",
	kiro = "agent-kiro", qwen = "agent-qwen", grok = "agent-grok",
}

-- A group's heading: the folder's name, and the folder above when it says
-- more than the home folder does ("moonframe" under "worktrees").
local function heading(home: string): (string, string?)
	local parent, name = string.match(home, "^(.*)/([^/]+)$")
	if not name then
		return home, nil
	end
	local above = string.match(parent :: string, "([^/]+)$")
	if not above or above == "~" then
		return name, nil
	end
	return name, (string.gsub(above, "^%.", ""))
end

return mod {
	region = "left",
	title = "Sessions",

	styles = {
		[".sessions-top"] = { padding = { 0, 10, 0, 10 }, gap = 2 },
		[".sessions-list"] = { padding = { 0, 10, 4, 10 }, gap = 2 },
		[".sessions-group"] = { padding = { 14, 10, 6, 10 }, gap = 6 },
		[".sessions-group-name"] = { weight = "medium", color = "muted-foreground" },
		[".sessions-group-count"] = { color = "muted-foreground", size = "small" },
		[".sessions-group-tag"] = { background = "accent/12", color = "accent", radius = 6, padding = { 1, 6 } },
		[".session-row"] = { padding = { 9, 10 }, radius = 10, gap = 3 },
		[".session-row:hover"] = { background = "list-hover" },
		[".session-row:selected"] = { background = "card", shadow = "0 1px 3px #00000014" },
		[".session-name"] = { weight = "medium" },
		[".session-meta"] = { color = "muted-foreground", size = "small" },
		[".session-check"] = { color = "muted-foreground" },
		[".sessions-bottom"] = { padding = { 8, 12 }, gap = 4 },
	},

	init = function(previous: State?): State
		return previous or { selected = nil, query = "", searching = false, changed_only = false }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			if msg == "search" then
				state.searching = not state.searching
				state.query = ""
			elseif msg == "changed-only" then
				state.changed_only = not state.changed_only
			end
			return
		end
		if msg.select then
			state.selected = msg.select
			return state, {
				{ to = "provider:projects", msg = { select = msg.select } },
				{ to = "session", msg = { select = msg.select } },
				{ to = "changes", msg = { select = msg.select } },
			}
		elseif msg.msg == "query" then
			state.query = msg.value
		elseif msg.msg == "cancel" then
			state.query = ""
			state.searching = false
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local current = projects and projects.current
		local selected = state.selected or (current and current.path)
		local query = string.lower(state.query)

		-- Groups in the order their newest session comes.
		local groups, order = {}, {}
		for _, project in (projects and projects.projects) or {} do
			local matches = query == "" or string.find(string.lower(project.name), query, 1, true)
				or string.find(string.lower(project.branch), query, 1, true)
			if matches and (not state.changed_only or project.files > 0) then
				local group = groups[project.home]
				if not group then
					group = {}
					groups[project.home] = group
					table.insert(order, project.home)
				end
				table.insert(group, project)
			end
		end

		local list = {}
		for index, home in order do
			local name, tag = heading(home)
			table.insert(list, ui.row {
				class = "sessions-group",
				-- The first heading sits right under the top bar.
				style = if index == 1 then { padding = { 0, 10, 6, 10 } } else nil,
				align = "center",
				tooltip = home,
				ui.text { name, class = "sessions-group-name" },
				tag and ui.text { tag, class = "sessions-group-tag", size = "small" },
				ui.text { tostring(#groups[home]), class = "sessions-group-count" },
			})
			for _, project in groups[home] do
				local agent = agent_in(app, project.path)
				local seconds = math.max(0, (projects.now or 0) - project.time)
				local meta = project.branch
				if project.files > 0 then
					meta ..= ` · {project.files} {if project.files == 1 then "file" else "files"}`
				end
				table.insert(list, ui.column {
					class = "session-row",
					selected = project.path == selected,
					msg = { select = project.path },
					tooltip = project.subject ~= "" and project.subject or nil,
					ui.row {
						align = "center",
						gap = "small",
						agent and ui.icon { name = (AGENT_ICONS[agent] or "bot") :: any, size = "small" },
						ui.text { if agent then `{agent} - {project.name}` else project.name, class = "session-name", truncate = true },
						ui.spacer(),
						relative_time { seconds = seconds, compact = true },
						if agent
							then status_dot { status = "working", size = "small", tooltip = `{agent} is running here` }
							else ui.icon { name = "check", size = "small", class = "session-check" },
					},
					ui.row {
						align = "center",
						gap = "small",
						ui.text { meta, class = "session-meta", truncate = true },
						project.files > 0 and diff_counts { added = project.added, removed = project.deleted },
						project.behind > 0 and ui.text { `↓{project.behind}`, class = "session-meta" },
						project.ahead > 0 and ui.text { `↑{project.ahead}`, class = "session-meta" },
					},
				})
			end
		end

		local body
		if not projects then
			body = ui.text { "Looking for projects…", tone = "muted", style = { padding = { 12, 20 } } }
		elseif projects.available == false then
			body = ui.text { projects.reason or "No projects", tone = "muted", style = { padding = { 12, 20 } } }
		elseif #order == 0 then
			body = ui.text {
				if query ~= "" or state.changed_only then "No session matches" else `No git repositories in {table.concat(projects.roots or {}, ", ")}`,
				tone = "muted",
				wrap = true,
				style = { padding = { 12, 20 } },
			}
		else
			body = ui.column { class = "sessions-list", shrink = false, table.unpack(list) }
		end

		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				class = "sessions-top",
				drag = true,
				height = 52,
				align = "center",
				-- Room for the traffic lights.
				ui.row { width = 76 },
				ui.spacer(),
				icon_button { icon = "panel-left", tooltip = "Hide sessions", msg = "toggle-left", to = "shell" },
				icon_button { icon = "plus", tooltip = "Find a session", msg = "search", active = state.searching },
			},
			state.searching and ui.row {
				style = { padding = { 0, 14, 6, 14 } },
				search {
					id = "sessions",
					value = state.query,
					placeholder = "Filter sessions…",
					change = "query",
					cancel = "cancel",
					focus = true,
				},
			},
			ui.column { grow = true, scroll = true, gap = "none", body },
			ui.row {
				class = "sessions-bottom",
				align = "center",
				icon_button {
					icon = "folder-git-2",
					tooltip = "Only sessions with changes",
					msg = "changed-only",
					active = state.changed_only,
				},
				icon_button { icon = "search", tooltip = "Filter sessions", msg = "search", active = state.searching },
				icon_button { icon = "palette", tooltip = "Themes", command = "show-themes" },
			},
		}
	end,
}
