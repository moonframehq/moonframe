-- The chosen session in a card: its name, folder, branch and changes over a
-- shell started in its folder, and a status line under it. Each session
-- keeps its own shell, named after its folder, so going back to one finds
-- it as it was (Moonframe keeps up to 8 shells a mod).

local icon_button = use "icon-button"
local diff_counts = use "diff-counts"

type State = {
	selected: string?,
	left: boolean,
	right: boolean,
}

-- A terminal id for a folder: the same folder always gets the same shell.
local function shell_id(path: string): string
	local hash = 5381
	for index = 1, #path do
		hash = (hash * 33 + string.byte(path, index)) % 2147483647
	end
	return `s{hash}`
end

local function tilde(path: string, home: string?): string
	-- The provider gives each project's home as ~/...; its path is absolute.
	if home then
		local name = string.match(path, "([^/]+)$")
		return `{home}/{name}`
	end
	return path
end

local AGENT_ICONS = {
	claude = "agent-claude", codex = "agent-codex", gemini = "agent-gemini", cursor = "agent-cursor",
	copilot = "agent-copilot", opencode = "agent-opencode", amp = "agent-amp",
}

return mod {
	region = "main",
	title = "Session",

	styles = {
		[".session-card"] = {
			background = "card",
			radius = 14,
			shadow = "0 1px 2px #0000000d, 0 4px 16px -4px #00000014",
			border = true,
			border_color = "border",
		},
		[".session-header"] = { padding = { 0, 14 }, gap = 10 },
		[".session-title"] = { weight = "bold" },
		[".session-folder"] = { color = "muted-foreground" },
		[".session-branch"] = { background = "secondary", radius = 7, padding = { 3, 8 }, gap = 6 },
		[".session-agent"] = { background = "secondary", radius = 7, padding = { 3, 9 }, color = "muted-foreground" },
		[".session-body"] = { padding = { 0, 4, 0, 14 } },
		[".session-footer"] = { padding = { 8, 18, 12, 18 }, gap = 2, border = "top", border_color = "border" },
		[".session-footer-text"] = { font = "mono", color = "muted-foreground", size = "small" },
	},

	init = function(previous: State?): State
		return previous or { selected = nil, left = true, right = true }
	end,

	update = function(state: State, msg)
		if type(msg) ~= "table" then
			return
		end
		if msg.select then
			state.selected = msg.select
		elseif msg.panes then
			state.left = msg.panes.left
			state.right = msg.panes.right
		end
	end,

	view = function(state: State, app)
		local projects = app.projects
		local current = projects and projects.current
		local path = state.selected or (current and current.path)
		local project
		for _, each in (projects and projects.projects) or {} do
			if each.path == path then
				project = each
			end
		end
		local agent
		local running = app.agents and app.agents.agents
		if running and path then
			for _, each in running do
				if each.cwd and (each.cwd == path or string.sub(each.cwd, 1, #path + 1) == path .. "/") then
					agent = each.name
				end
			end
		end

		local window = app.window
		local inset = not state.left and window.platform == "macos" and not window.fullscreen
		local header = ui.row {
			class = "session-header",
			drag = true,
			height = 50,
			align = "center",
			inset and ui.row { width = 64 },
			not state.left and icon_button { icon = "panel-left", tooltip = "Show sessions", msg = "toggle-left", to = "shell" },
			project and ui.text { project.name, class = "session-title", truncate = true },
			project and ui.row {
				width = 300,
				ui.text { tilde(project.path, project.home), class = "session-folder", truncate = true },
			},
			project and project.branch ~= "" and ui.row {
				class = "session-branch",
				align = "center",
				max_width = 340,
				ui.icon { name = "git-branch", size = "small", tone = "muted" },
				ui.text { project.branch, size = "small", truncate = true },
				diff_counts { added = project.added, removed = project.deleted },
			},
			ui.spacer(),
			agent and ui.row {
				class = "session-agent",
				align = "center",
				gap = "small",
				shrink = false,
				ui.icon { name = (AGENT_ICONS[agent] or "bot") :: any, size = "small" },
				ui.text { agent, size = "small" },
			},
			not state.right and icon_button { icon = "panel-right", tooltip = "Show changes", msg = "toggle-right", to = "shell" },
		}

		local body
		if not path then
			body = ui.column {
				grow = true,
				align = "center",
				justify = "center",
				ui.icon { name = "square-terminal", size = "title", tone = "muted" },
				ui.text { "Pick a session", tone = "muted" },
			}
		else
			body = ui.column {
				class = "session-body",
				grow = true,
				ui.terminal { id = shell_id(path), cwd = path },
			}
		end

		local stats = {}
		if project then
			table.insert(stats, `{project.files} {if project.files == 1 then "file" else "files"} +{project.added} −{project.deleted}`)
			if project.ahead > 0 or project.behind > 0 then
				table.insert(stats, `↑{project.ahead} ↓{project.behind}`)
			end
			if project.subject ~= "" then
				table.insert(stats, `last: {project.subject}`)
			end
		end

		local footer = project and ui.column {
			class = "session-footer",
			ui.text {
				`{tilde(project.path, project.home)} ({project.branch})`,
				class = "session-footer-text",
				truncate = true,
			},
			ui.text {
				table.concat(stats, "   "),
				class = "session-footer-text",
				truncate = true,
			},
		}

		return ui.column {
			class = "session-card",
			grow = true,
			gap = "none",
			header,
			body,
			footer,
		}
	end,
}
